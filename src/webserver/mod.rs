use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{self, TraceLayer};
use tower_http::LatencyUnit;
use tracing::{info, warn};

use crate::config::Settings;
use crate::persist::CommonData;
use crate::webserver::routes::app_router;

mod routes;

pub struct WebServer {
    settings: Settings,
    token: CancellationToken,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct AppState {
    pub settings: Settings,
}

impl WebServer {
    pub fn new(data: CommonData, token: CancellationToken) -> Self {
        Self {
            settings: data.settings,
            token,
        }
    }

    pub async fn start(self) {
        let addr = SocketAddr::new(
            IpAddr::V4(self.settings.server.ip),
            self.settings.server.port,
        );

        info!("Starting web server on {}...", addr);

        let state = AppState {
            settings: self.settings,
        };

        let app_router = app_router(state).layer((
            TraceLayer::new_for_http()
                .make_span_with(crate::log::make_span_with)
                .on_response(
                    trace::DefaultOnResponse::new()
                        .latency_unit(LatencyUnit::Millis)
                        .level(tracing::Level::INFO),
                ),
            // Graceful shutdown will wait for outstanding requests to complete. Add a timeout so
            // requests don't hang forever.
            // TODO: add timeout to settings
            TimeoutLayer::new(std::time::Duration::from_secs(3)),
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
