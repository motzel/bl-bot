#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use lazy_static::lazy_static;
use log::{debug, error, info};
use peak_alloc::PeakAlloc;
pub(crate) use poise::serenity_prelude as serenity;
use serenity::model::id::GuildId;
use shuttle_persist::PersistInstance;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

use crate::beatleader::Client;
use crate::bot::commands::{
    cmd_add_auto_role, cmd_link, cmd_profile, cmd_register, cmd_remove_auto_role, cmd_replay,
    cmd_set_log_channel, cmd_show_settings, cmd_unlink,
};
use crate::bot::{GuildSettings, UserRoleChanges};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;

mod beatleader;
mod bot;
mod storage;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

lazy_static! {
    static ref BL_CLIENT: Client = Client::default();
}

pub(crate) struct Data {
    guild_settings_repository: Arc<GuildSettingsRepository>,
    players_repository: Arc<PlayerRepository>,
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

#[shuttle_runtime::main]
async fn poise(
    #[shuttle_secrets::Secrets] secret_store: SecretStore,
    #[shuttle_persist::Persist] persist: PersistInstance,
) -> ShuttlePoise<Data, Error> {
    info!("Starting up...");

    // Get config set in `Secrets.toml`
    let discord_token = secret_store
        .get("DISCORD_TOKEN")
        .expect("'DISCORD_TOKEN' was not found");

    let refresh_interval: u64 = secret_store
        .get("REFRESH_INTERVAL")
        .expect("'REFRESH_INTERVAL' was not found")
        .parse()
        .expect("'REFRESH_INTERVAL' should be an integer");
    if refresh_interval < 30 {
        panic!("REFRESH_INTERVAL should be greater than 30 seconds");
    }

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
            cmd_register(),
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
        .token(discord_token)
        .intents(serenity::GatewayIntents::non_privileged()) // | serenity::GatewayIntents::MESSAGE_CONTEN
        .setup(move |ctx, _ready, _framework| {
            Box::pin(async move {
                info!("Logged in as {}", _ready.user.name);

                let persist_arc = Arc::new(persist);
                let persist_arc2 = Arc::clone(&persist_arc);

                info!("Initializing guild settings repository...");
                let guild_settings_repository =
                    Arc::new(GuildSettingsRepository::new(persist_arc).await.unwrap());
                info!(
                    "Guild settings repository initialized, length: {}.",
                    guild_settings_repository.len().await
                );

                info!("Initializing players repository...");
                let players_repository =
                    Arc::new(PlayerRepository::new(persist_arc2).await.unwrap());
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

                let guild_settings_repository_worker = Arc::clone(&guild_settings_repository);
                let players_repository_worker = Arc::clone(&players_repository);

                tokio::spawn(async move {
                    let interval = std::time::Duration::from_secs(refresh_interval);
                    info!("Run a task that updates profiles every {:?}", interval);

                    loop {
                        debug!("RAM usage: {} MB", PEAK_ALLOC.current_usage_as_mb());
                        debug!("Peak RAM usage: {} MB", PEAK_ALLOC.peak_usage_as_mb());

                        if let Ok(bot_players) =
                            players_repository_worker.update_all_players_stats().await
                        {
                            info!("Updating players roles ({})...", bot_players.len());

                            let mut current_players_roles = Vec::new();
                            for bot_player in bot_players {
                                debug!(
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

                            let guild_ids = current_players_roles.iter().map(|(guild_id, _player, _roles)| *guild_id).collect::<Vec<GuildId>>();
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
                })
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}
