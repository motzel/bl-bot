use std::sync::Arc;

pub(crate) use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ActivityData, ClientBuilder};
use poise::Framework;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

use worker::oauth::BlOauthTokenRefreshWorker;

use crate::beatleader::oauth::OAuthAppCredentials;
use crate::config::Settings;
use crate::discord::worker::clan_contribution::BlClanContributionWorker;
use crate::discord::worker::clan_peak::BlClanPeakWorker;
use crate::discord::worker::clan_wars::BlClanWarsMapsWorker;
use crate::discord::worker::player_stats::BlPlayersStatsWorker;
use crate::discord::worker::user_roles::UserRolesWorker;
use crate::persist::CommonData;
use crate::storage::bsmaps::BsMapsRepository;
use crate::storage::clan_peak::ClanPeakRepository;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::storage::playlist::PlaylistRepository;

pub mod bot;
mod worker;

pub(crate) struct BotData {
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub players_repository: Arc<PlayerRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub playlists_repository: Arc<PlaylistRepository>,
    pub maps_repository: Arc<BsMapsRepository>,
    pub clan_peak_repository: Arc<ClanPeakRepository>,
    pub settings: Settings,
}

impl BotData {
    fn oauth_credentials(&self) -> Option<OAuthAppCredentials> {
        self.settings
            .oauth
            .as_ref()
            .map(|oauth_settings| OAuthAppCredentials {
                client_id: oauth_settings.client_id.clone(),
                client_secret: oauth_settings.client_secret.clone(),
                redirect_uri: oauth_settings.redirect_uri.clone(),
            })
    }
}

impl From<CommonData> for BotData {
    fn from(value: CommonData) -> Self {
        Self {
            guild_settings_repository: value.guild_settings_repository,
            players_repository: value.players_repository,
            player_scores_repository: value.player_scores_repository,
            player_oauth_token_repository: value.player_oauth_token_repository,
            playlists_repository: value.playlists_repository,
            maps_repository: value.maps_repository,
            clan_peak_repository: value.clan_peak_repository,
            settings: value.settings,
        }
    }
}

pub(crate) type Context<'a> = poise::Context<'a, BotData, crate::Error>;

pub struct DiscordClient {
    client: serenity::Client,
    tracker: TaskTracker,
    token: CancellationToken,
}

impl DiscordClient {
    pub async fn new(
        data: CommonData,
        tracker: TaskTracker,
        token: CancellationToken,
    ) -> DiscordClient {
        let settings = data.settings.clone();

        let options = poise::FrameworkOptions {
            commands: bot::commands(),
            pre_command: |ctx| {
                Box::pin(async move {
                    info!("Executing command {}...", ctx.command().qualified_name);
                })
            },
            // This code is run after a command if it was successful (returned Ok)
            post_command: |ctx| {
                Box::pin(async move {
                    info!("Executed command {}!", ctx.command().qualified_name);
                })
            },
            on_error: |error| {
                Box::pin(async move {
                    match error {
                        poise::FrameworkError::Setup { error, .. } => {
                            panic!("Failed to start bot: {error:?}")
                        }
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            info!("Error in command `{}`: {:?}", ctx.command().name, error,);
                        }
                        error => {
                            if let Err(e) = poise::builtins::on_error(error).await {
                                info!("Error while handling error: {}", e)
                            }
                        }
                    };
                })
            },
            ..Default::default()
        };

        let tracker_clone = tracker.clone();
        let token_clone = token.clone();

        Self {
            client: ClientBuilder::new(
                settings.discord_token.clone(),
                serenity::GatewayIntents::non_privileged(), // | serenity::GatewayIntents::MESSAGE_CONTENT
            )
            .framework(
                Framework::builder()
                    .options(options)
                    .setup(move |ctx, _ready, _framework| {
                        Box::pin(async move {
                            info!("Bot logged in as {}", _ready.user.name);

                            info!("Setting bot status...");
                            ctx.set_presence(
                                Some(ActivityData::playing("Beat Leader")),
                                serenity::model::user::OnlineStatus::Online,
                            );

                            let bl_oauth_token_refresh_worker = BlOauthTokenRefreshWorker::new(
                                data.clone().into(),
                                chrono::Duration::seconds(settings.refresh_interval as i64 + 30),
                                token_clone.clone(),
                            );
                            let bl_clan_wars_maps_worker = BlClanWarsMapsWorker::new(
                                ctx.clone(),
                                data.clone().into(),
                                chrono::Duration::minutes(settings.clan_wars_interval as i64),
                                token_clone.clone(),
                            );
                            let bl_players_stats_worker =
                                BlPlayersStatsWorker::new(data.clone().into(), token_clone.clone());
                            let discord_user_roles_worker = UserRolesWorker::new(
                                ctx.clone(),
                                data.clone().into(),
                                token_clone.clone(),
                            );
                            let bl_clan_contribution_maps_worker = BlClanContributionWorker::new(
                                ctx.clone(),
                                data.clone().into(),
                                chrono::Duration::minutes(
                                    settings.clan_wars_contribution_interval as i64,
                                ),
                                token_clone.clone(),
                            );

                            let bl_clan_peak_worker = BlClanPeakWorker::new(
                                ctx.clone(),
                                data.clone().into(),
                                token_clone.clone(),
                            );

                            let data: BotData = data.into();

                            tracker_clone.spawn(async move {
                                let interval =
                                    std::time::Duration::from_secs(settings.refresh_interval);
                                info!("Run tasks that update data every {:?}", interval);

                                'outer: loop {
                                    bl_oauth_token_refresh_worker.run().await;

                                    bl_clan_peak_worker.run().await;

                                    if let Ok(bot_players) = bl_players_stats_worker.run().await {
                                        discord_user_roles_worker.run(bot_players).await;
                                    }

                                    bl_clan_contribution_maps_worker.run().await;

                                    bl_clan_wars_maps_worker.run().await;

                                    tokio::select! {
                                        _ = token_clone.cancelled() => {
                                            warn!("BL update tasks are shutting down...");
                                            break 'outer;
                                        }
                                        _ = tokio::time::sleep(interval) => {}
                                    }
                                }

                                warn!("BL update tasks shut down.");
                            });

                            Ok(data)
                        })
                    })
                    .build(),
            )
            .await
            .expect("Can not build discord client"),
            tracker,
            token,
        }
    }

    pub async fn start(mut self) {
        #[cfg(windows)]
        let shard_manager_clone_win = self.client.shard_manager.clone();

        #[cfg(unix)]
        let shard_manager_clone_win = self.client.shard_manager.clone();

        #[cfg(windows)]
        self.tracker.spawn(async move {
            let _ = signal::ctrl_c().await;
            warn!("CTRL+C pressed, shutting down...");
            self.token.cancel();

            warn!("Discord client is shutting down...");
            shard_manager_clone_win.shutdown_all().await;
            warn!("Discord client shut down.");
        });

        #[cfg(unix)]
        self.tracker.spawn(async move {
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt()).unwrap();
            let mut sighup = signal::unix::signal(signal::unix::SignalKind::hangup()).unwrap();
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = sigint.recv() => {
                    warn!("SIGINT received, shutting down...");
                    self.token.cancel();
                    warn!("Discord client is shutting down...");
                    shard_manager_clone_win
                        .shutdown_all()
                        .await;
                    warn!("Discord client shut down.");
                }
                _ = sighup.recv() => {
                    warn!("SIGHUP received, shutting down...");
                    self.token.cancel();
                    warn!("Discord client is shutting down...");
                    shard_manager_clone_win
                        .shutdown_all()
                        .await;
                    warn!("Discord client shut down.");
                }
                _ = sigterm.recv() => {
                    warn!("SIGTERM received, shutting down...");
                    self.token.cancel();
                    warn!("Discord client is shutting down...");
                    shard_manager_clone_win
                        .shutdown_all()
                        .await;
                    warn!("Discord client shut down.");
                }
            }
        });

        self.client
            .start()
            .await
            .expect("Can not start discord client");
    }
}
