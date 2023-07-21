mod beatleader;
mod bot;

use anyhow::Context as _;
use log::info;

pub(crate) use poise::serenity_prelude as serenity;

use crate::bot::commands::bl_link;
use crate::bot::commands::bl_replay;
use serenity::model::id::GuildId;

use crate::beatleader::Client;
use crate::bot::db::LinkedPlayers;
use shuttle_persist::PersistInstance;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

pub(crate) struct Data {
    guild_id: GuildId,
    bl_client: Client,
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
        .context("'DISCORD_TOKEN' was not found")?;

    let guild_id = secret_store
        .get("GUILD_ID")
        .context("'GUILD_ID' was not found")?;

    info!("Loading player link data...");
    if persist.load::<LinkedPlayers>("linked-players-v1").is_err() {
        let _ = persist.save::<LinkedPlayers>("linked-players-v1", LinkedPlayers::new());
    }

    let bl_client = Client::default();

    let options = poise::FrameworkOptions {
        commands: vec![
            bl_replay(),
            bl_link(),
            // bl_display_auto_roles(),
            // bl_add_auto_role(),
            // bl_remove_auto_roles(),
            // bl_test(),
            // bl_test2(),
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
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                info!("Logged in as {}", _ready.user.name);

                ctx.set_presence(
                    Some(serenity::model::gateway::Activity::playing("Beat Leader")),
                    serenity::model::user::OnlineStatus::Online,
                )
                .await;

                let guild_id = guild_id.parse().unwrap();

                let _global_ctx = ctx.clone();
                // channel.say(&global_ctx.http, "BOT started").await?;

                tokio::spawn(async move {
                    let _channel = serenity::model::id::ChannelId(1131312515498901534_u64);
                    // info!("{:#?}", global_ctx.http.get_channels(guild_id).await?);

                    let mut counter = 1;

                    let mut timer = tokio::time::interval(std::time::Duration::from_secs(60));
                    loop {
                        timer.tick().await;

                        // let _ = channel
                        //     .say(&global_ctx.http, format!("BOT message no. {}", counter))
                        //     .await;

                        // let _ = channel
                        //     .say(
                        //         &global_ctx.http,
                        //         "https://replay.beatleader.xyz/?scoreId=7919269",
                        //     )
                        //     .await;

                        info!("Counter: {}", counter);

                        counter += 1;
                    }
                });

                let guild_id = serenity::model::id::GuildId(guild_id);
                poise::builtins::register_in_guild(ctx, &framework.options().commands, guild_id)
                    .await?;

                Ok(Data {
                    guild_id,
                    persist,
                    bl_client,
                })
            })
        })
        .build()
        .await
        .map_err(shuttle_runtime::CustomError::new)?;

    Ok(framework.into())
}
