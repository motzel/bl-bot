#![allow(dead_code)]
mod beatleader;
mod bot;

use peak_alloc::PeakAlloc;
use std::sync::Arc;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

use log::{debug, info};

pub(crate) use poise::serenity_prelude as serenity;

use crate::bot::commands::{
    bl_add_auto_role, bl_link, bl_remove_auto_role, bl_replay, bl_show_auto_roles,
};
use serenity::model::id::GuildId;

use crate::beatleader::Client;
use crate::bot::beatleader::Player as BotPlayer;
use crate::bot::db::{
    fetch_and_update_all_players, get_guild_settings, get_linked_players, LinkedPlayers,
};
use crate::bot::{GuildSettings, UserRoleChanges};
use lazy_static::lazy_static;
use poise::serenity_prelude::{RoleId, UserId};
use shuttle_persist::PersistInstance;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;
use tokio::sync::Mutex;

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

    let options = poise::FrameworkOptions {
        commands: vec![
            bl_replay(),
            bl_link(),
            bl_show_auto_roles(),
            bl_add_auto_role(),
            bl_remove_auto_role(),
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

                let linked_players_arc = Arc::new(Mutex::new(linked_players));
                let guild_settings_arc = Arc::new(Mutex::new(guild_settings));

                info!("Setting bot status...");
                ctx.set_presence(
                    Some(serenity::model::gateway::Activity::playing("Beat Leader")),
                    serenity::model::user::OnlineStatus::Online,
                )
                .await;

                let _global_ctx = ctx.clone();
                let global_persist = persist.clone();

                let guild_settings_worker = Arc::clone(&guild_settings_arc);

                tokio::spawn(async move {
                    // let _channel = serenity::model::id::ChannelId(1131312515498901534_u64);
                    // let _ = _channel.say(_global_ctx, "test").await;

                    // let roles = _global_ctx
                    //     .http
                    //     .get_guild_roles(global_guild_id.into())
                    //     .await;
                    // println!("{:?}", roles);

                    let interval = std::time::Duration::from_secs(5 * 60);
                    info!("Run a task that updates profiles every {:?}", interval);

                    let mut timer = tokio::time::interval(interval);
                    loop {
                        timer.tick().await;

                        debug!("RAM usage: {} MB", PEAK_ALLOC.current_usage_as_mb());
                        debug!("Peak RAM usage: {} MB", PEAK_ALLOC.peak_usage_as_mb());

                        if let Ok(players) = fetch_and_update_all_players(&global_persist).await {
                            info!("Updating players roles ({})...", players.len());

                            let current_players_roles = players
                                .iter()
                                .map(|(player, user_id)| {
                                    // TODO: fetch current player roles

                                    (player, user_id, Vec::<RoleId>::new())
                                })
                                .collect::<Vec<(&BotPlayer, &UserId, Vec<RoleId>)>>();

                            let lock = guild_settings_worker.lock().await;
                            let roles_updates = current_players_roles
                                .iter()
                                .map(|(player, &user_id, roles)| {
                                    lock.get_role_updates(player, user_id, roles)
                                })
                                .collect::<Vec<UserRoleChanges>>();
                            drop(lock);

                            for _role_changes in roles_updates {
                                info!("{:?}", _role_changes);
                            }

                            info!("Players roles updated.");
                        }
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
