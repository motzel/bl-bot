#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use lazy_static::lazy_static;
use log::{error, info, trace, warn};
use peak_alloc::PeakAlloc;
pub(crate) use poise::serenity_prelude as serenity;
use serenity::model::id::GuildId;

use crate::beatleader::oauth::OAuthAppCredentials;
use crate::beatleader::Client;
use crate::bot::commands::{
    cmd_add_auto_role, cmd_clan_invitation, cmd_export, cmd_import, cmd_link, cmd_profile,
    cmd_refresh_scores, cmd_register, cmd_remove_auto_role, cmd_replay, cmd_set_clan_invitation,
    cmd_set_clan_invitation_code, cmd_set_log_channel, cmd_set_profile_verification,
    cmd_show_settings, cmd_unlink,
};
use crate::bot::{GuildOAuthTokenRepository, GuildSettings, UserRoleChanges};
use crate::config::Settings;
use crate::file_storage::PersistInstance;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;

mod beatleader;
mod bot;
mod config;
mod embed;
mod file_storage;
mod storage;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

lazy_static! {
    static ref BL_CLIENT: Client = Client::default();
}

pub(crate) struct Data {
    guild_settings_repository: Arc<GuildSettingsRepository>,
    players_repository: Arc<PlayerRepository>,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    oauth_credentials: Option<OAuthAppCredentials>,
}
pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;
pub(crate) type Context<'a> = poise::Context<'a, Data, Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            info!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                info!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("bl_bot=info"))
        .target(env_logger::Target::Stdout)
        .init();

    let settings = Settings::new().unwrap();

    info!("Starting up...");

    let oauth_credentials = settings
        .oauth
        .as_ref()
        .map(|oauth_settings| OAuthAppCredentials {
            client_id: oauth_settings.client_id.clone(),
            client_secret: oauth_settings.client_secret.clone(),
            redirect_uri: oauth_settings.redirect_uri.clone(),
        });

    let persist = PersistInstance::new(PathBuf::from(&settings.storage_path)).unwrap();

    let options = poise::FrameworkOptions {
        commands: vec![
            cmd_replay(),
            cmd_profile(),
            cmd_link(),
            cmd_unlink(),
            cmd_show_settings(),
            cmd_add_auto_role(),
            cmd_remove_auto_role(),
            cmd_set_log_channel(),
            cmd_set_profile_verification(),
            cmd_set_clan_invitation(),
            cmd_set_clan_invitation_code(),
            cmd_clan_invitation(),
            // cmd_invite_player(),
            cmd_register(),
            cmd_export(),
            cmd_import(),
            cmd_refresh_scores(),
        ],
        pre_command: |ctx| {
            Box::pin(async move {
                info!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        /// This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                info!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        on_error: |error| Box::pin(on_error(error)),
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(options)
        .token(settings.discord_token.clone())
        .intents(serenity::GatewayIntents::non_privileged()) // | serenity::GatewayIntents::MESSAGE_CONTENT
        .setup(move |ctx, _ready, _framework| {
            Box::pin(async move {
                info!("Logged in as {}", _ready.user.name);

                let persist_arc = Arc::new(persist);
                let persist_arc2 = Arc::clone(&persist_arc);
                let persist_arc3 = Arc::clone(&persist_arc);

                info!("Initializing player OAuth tokens repository...");
                let player_oauth_token_repository =
                    Arc::new(PlayerOAuthTokenRepository::new(persist_arc).await.unwrap());
                info!(
                    "Player OAuth tokens repository initialized, length: {}.",
                    player_oauth_token_repository.len().await
                );

                info!("Initializing guild settings repository...");
                let guild_settings_repository =
                    Arc::new(GuildSettingsRepository::new(persist_arc2).await.unwrap());
                info!(
                    "Guild settings repository initialized, length: {}.",
                    guild_settings_repository.len().await
                );

                info!("Initializing players repository...");
                let players_repository =
                    Arc::new(PlayerRepository::new(persist_arc3).await.unwrap());
                info!(
                    "Players repository initialized, length: {}.",
                    players_repository.len().await
                );

                info!("Setting bot status...");
                ctx.set_presence(
                    Some(serenity::model::gateway::Activity::playing("Beat Leader")),
                    serenity::model::user::OnlineStatus::Online,
                )
                .await;

                let global_ctx = ctx.clone();

                let player_oauth_token_repository_worker = Arc::clone(&player_oauth_token_repository);
                let guild_settings_repository_worker = Arc::clone(&guild_settings_repository);
                let players_repository_worker = Arc::clone(&players_repository);

                let oauth_credentials_clone = oauth_credentials.clone();

                tokio::spawn(async move {
                    let interval = std::time::Duration::from_secs(settings.refresh_interval);
                    info!("Run a task that updates data every {:?}", interval);

                    loop {
                        info!("RAM usage: {} MB", PEAK_ALLOC.current_usage_as_mb());
                        info!("Peak RAM usage: {} MB", PEAK_ALLOC.peak_usage_as_mb());

                        info!("Refreshing expired OAuth tokens...");

                        if let Some(ref oauth_credentials) = oauth_credentials_clone {
                            for guild in guild_settings_repository_worker.all().await {
                                if let Some(clan_settings) = guild.get_clan_settings() {
                                    if clan_settings.is_oauth_token_set() {
                                        info!("Refreshing OAuth token for a clan {}...", clan_settings.get_clan());

                                        let clan_owner_id = clan_settings.get_owner();

                                        let oauth_token_option = player_oauth_token_repository_worker.get(&clan_owner_id).await;

                                        if let Some(oauth_token) = oauth_token_option {
                                            if !oauth_token.oauth_token.is_valid_for(chrono::Duration::seconds(settings.refresh_interval as i64 + 30)) {
                                                let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
                                                    clan_owner_id,
                                                    Arc::clone(&player_oauth_token_repository_worker),
                                                );
                                                let oauth_client = BL_CLIENT.with_oauth(
                                                    oauth_credentials.clone(),
                                                    guild_oauth_token_repository,
                                                );

                                                match oauth_client.refresh_token_if_needed().await {
                                                    Ok(oauth_token) => {
                                                        info!("OAuth token refreshed, expiration date: {}", oauth_token.get_expiration());
                                                    },
                                                    Err(err) => {
                                                        error!("OAuth token refreshing error: {}", err);
                                                    }
                                                }
                                            } else {
                                                info!("OAuth token is still valid, skip refreshing.");
                                            }
                                        } else {
                                            warn!("No OAuth token for a clan {} found.", clan_settings.get_clan());
                                        }
                                    }
                                }
                            }

                            info!("OAuth tokens refreshed.");
                        } else {
                            info!("No OAuth credentials, skipping.");
                        }

                        if let Ok(bot_players) =
                            players_repository_worker.update_all_players_stats(false).await
                        {
                            info!("Updating players roles ({})...", bot_players.len());

                            let mut current_players_roles = Vec::new();
                            for bot_player in bot_players {
                                trace!(
                                    "Fetching user {} ({}) roles...",
                                    &bot_player.user_id, &bot_player.name
                                );

                                for guild_id in &bot_player.linked_guilds {
                                    let Ok(member) = global_ctx
                                        .http
                                        .get_member(u64::from(*guild_id), bot_player.user_id.into())
                                        .await else {
                                        error!("Can not fetch user {} membership.", bot_player.user_id);
                                        continue;
                                    };

                                    current_players_roles.push((
                                        *guild_id,
                                        bot_player.clone(),
                                        member.roles,
                                    ));
                                }
                            }

                            let mut guild_ids = current_players_roles.iter().map(|(guild_id, _player, _roles)| *guild_id).collect::<Vec<GuildId>>();
                            guild_ids.sort_unstable();
                            guild_ids.dedup();

                            let mut guilds : HashMap<GuildId, GuildSettings> = HashMap::new();

                            for guild_id in &guild_ids {
                                if let Ok(guild_settings) = guild_settings_repository_worker.get(guild_id).await {
                                    guilds.insert(*guild_id, guild_settings);
                                }
                            }

                            let role_changes = current_players_roles
                                .iter()
                                .filter_map(|(guild_id, player, roles)| {
                                    guilds.get(guild_id).map(|guild_settings| guild_settings.get_role_updates(*guild_id, player, roles))
                                })
                                .collect::<Vec<UserRoleChanges>>();

                            for rc in role_changes {
                                match rc.apply(&global_ctx.http).await {
                                    Ok(rc) => {
                                        if rc.is_changed() {
                                            if let Some(bot_channel_id) = guilds.get(&rc.guild_id).map_or_else(|| None, |guild_settings| guild_settings.get_channel()) {
                                                info!(
                                                    "Logging changes to channel #{}",
                                                    bot_channel_id
                                                );

                                                match bot_channel_id
                                                    .send_message(global_ctx.clone(), |m| {
                                                        m.content(format!("{}", rc))
                                                            .allowed_mentions(|am| am.empty_parse())
                                                    })
                                                    .await {
                                                    Ok(_) => {}
                                                    Err(err) => {
                                                        info!("Can not post log update to channel #{}: {}", bot_channel_id, err);
                                                    }
                                                };
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to update roles for user {}: {}",
                                            rc.user_id, e
                                        );
                                    }
                                }
                            }

                            info!("Players roles updated.");
                        }

                        tokio::time::sleep(interval).await;
                    }
                });

                Ok(Data {
                    guild_settings_repository,
                    players_repository,
                    player_oauth_token_repository,
                    oauth_credentials,
                })
            })
        });

    framework.run().await.unwrap();
}
