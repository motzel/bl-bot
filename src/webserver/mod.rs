use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tower_http::LatencyUnit;
use tracing::{info, warn};

use crate::config::Settings;
use crate::persist::CommonData;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::storage::playlist::PlaylistRepository;
use crate::webserver::routes::app_router;

mod routes;

pub struct WebServer {
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub playlists_repository: Arc<PlaylistRepository>,
    pub settings: Settings,
    tracker: TaskTracker,
    token: CancellationToken,
}

#[derive(Debug, Clone)]
pub(crate) struct AppState {
    pub playlists_repository: Arc<PlaylistRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub settings: Settings,
}

impl WebServer {
    pub fn new(data: CommonData, tracker: TaskTracker, token: CancellationToken) -> Self {
        Self {
            guild_settings_repository: data.guild_settings_repository,
            player_oauth_token_repository: data.player_oauth_token_repository,
            player_scores_repository: data.player_scores_repository,
            playlists_repository: data.playlists_repository,
            settings: data.settings,
            tracker,
            token,
        }
    }

    pub async fn start(self) {
        let addr = SocketAddr::new(
            IpAddr::V4(self.settings.server.ip),
            self.settings.server.port,
        );

        info!("Starting web server on {}...", addr);

        let timeout = self.settings.server.timeout;

        let state = AppState {
            guild_settings_repository: self.guild_settings_repository,
            player_oauth_token_repository: self.player_oauth_token_repository,
            player_scores_repository: self.player_scores_repository,
            playlists_repository: self.playlists_repository,
            settings: self.settings,
        };

        let app_router = app_router(self.tracker.clone(), self.token.clone(), state).layer((
            TraceLayer::new_for_http()
                .make_span_with(crate::log::make_span_with)
                .on_response(
                    trace::DefaultOnResponse::new()
                        .latency_unit(LatencyUnit::Millis)
                        .level(tracing::Level::INFO),
                ),
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            TimeoutLayer::new(std::time::Duration::from_secs(timeout as u64)),
        ));

        axum::serve(
            tokio::net::TcpListener::bind(&addr).await.unwrap(),
            app_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            self.token.cancelled().await;

            warn!("Web server is shutting down...");
        })
        .await
        .unwrap();

        warn!("Web server shut down.");
    }
}
