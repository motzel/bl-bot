#![allow(dead_code)]

use std::sync::Arc;

use lazy_static::lazy_static;
use log::{debug, error, info};
use peak_alloc::PeakAlloc;
pub(crate) use poise::serenity_prelude as serenity;
use serenity::model::id::GuildId;
use shuttle_persist::PersistInstance;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;

use crate::beatleader::Client;
use crate::bot::commands::{
    cmd_add_auto_role, cmd_link, cmd_register, cmd_remove_auto_role, cmd_replay,
    cmd_set_log_channel, cmd_show_settings, cmd_unlink,
};
use crate::bot::db::{
    fetch_and_update_all_players, get_guild_settings, get_linked_players, LinkedPlayers,
};
use crate::bot::{GuildSettings, UserRoleChanges};

mod beatleader;
mod bot;
mod storage;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

lazy_static! {
    static ref BL_CLIENT: Client = Client::default();
}

pub(crate) struct Data {
    guild_id: GuildId,
    guild_settings: Arc<Mutex<GuildSettings>>,
    linked_players: Arc<Mutex<LinkedPlayers>>,
    persist: PersistInstance,
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

    let guild_id = serenity::model::id::GuildId(
        secret_store
            .get("GUILD_ID")
            .expect("'GUILD_ID' was not found")
            .parse()
            .expect("'GUILD_ID' should be an integer"),
    );

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

    let global_guild_id = guild_id;

    let framework = poise::Framework::builder()
        .options(options)
        .token(discord_token)
        .intents(serenity::GatewayIntents::non_privileged()) // | serenity::GatewayIntents::MESSAGE_CONTEN
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                info!(
                    "Logged in as {} in guild {}",
                    _ready.user.name, global_guild_id
                );

                info!("Loading guild settings...");
                let guild_settings = match get_guild_settings(&persist, global_guild_id).await {
                    Ok(gs) => gs,
                    Err(e) => {
                        panic!("Error fetching guild settings: {}", e);
                    }
                };
                info!("Guild settings loaded");

                info!("Loading linked players...");
                let linked_players = match get_linked_players(&persist).await {
                    Ok(gs) => gs,
                    Err(e) => {
                        panic!("Error fetching linked players: {}", e);
                    }
                };
                info!("Linked players loaded");

                let bot_channel_id_opt = guild_settings.get_channel();

                let linked_players_arc = Arc::new(Mutex::new(linked_players));
                let guild_settings_arc = Arc::new(Mutex::new(guild_settings));

                info!("Setting bot status...");
                ctx.set_presence(
                    Some(serenity::model::gateway::Activity::playing("Beat Leader")),
                    serenity::model::user::OnlineStatus::Online,
                )
                .await;

                let global_ctx = ctx.clone();
                let global_persist = persist.clone();

                let guild_settings_worker = Arc::clone(&guild_settings_arc);

                tokio::spawn(async move {
                    let interval = std::time::Duration::from_secs(refresh_interval);
                    info!("Run a task that updates profiles every {:?}", interval);

                    loop {
                        debug!("RAM usage: {} MB", PEAK_ALLOC.current_usage_as_mb());
                        debug!("Peak RAM usage: {} MB", PEAK_ALLOC.peak_usage_as_mb());

                        if let Ok(players) = fetch_and_update_all_players(&global_persist).await {
                            info!("Updating players roles ({})...", players.len());

                            let mut current_players_roles = Vec::new();
                            for (player, user_id) in players {
                                debug!("Fetching user {} ({}) roles...", user_id, player.name);

                                let Ok(member) = global_ctx
                                    .http
                                    .get_member(guild_id.into(), user_id.into())
                                    .await else {
                                    error!("Can not fetch user {} membership.", user_id);
                                    continue;
                                };

                                current_players_roles.push((player, user_id, member.roles));
                            }

                            let lock = guild_settings_worker.lock().await;
                            let role_changes = current_players_roles
                                .iter()
                                .map(|(player, user_id, roles)| {
                                    lock.get_role_updates(player, *user_id, roles)
                                })
                                .collect::<Vec<UserRoleChanges>>();
                            drop(lock);

                            for rc in role_changes {
                                match rc.apply(global_guild_id, &global_ctx.http).await {
                                    Ok(rc) => {
                                        if rc.is_changed() {
                                            if let Some(bot_channel_id) = bot_channel_id_opt {
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
                                            rc.discord_user_id, e
                                        );
                                    }
                                }
                            }

                            info!("Players roles updated.");
                        }

                        tokio::time::sleep(interval).await;
                    }
                });

                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id)
                    .await?;

                Ok(Data {
                    guild_id,
                    linked_players: linked_players_arc,
                    persist,
                    guild_settings: guild_settings_arc,
                })
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}
