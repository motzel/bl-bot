use std::time::Duration;
use std::{fmt, str::FromStr};

use axum::extract::Path;
use axum::http::{header, Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, extract::State, routing::get, Router};
use serde::{de, Deserialize, Deserializer, Serialize};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use tower_governor::key_extractor::KeyExtractor;
use tower_governor::{governor::GovernorConfigBuilder, GovernorError, GovernorLayer};

use crate::webserver::AppState;

mod api;
mod fallback;
mod web;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct PlaylistUser;

impl KeyExtractor for PlaylistUser {
    type Key = String;

    fn name(&self) -> &'static str {
        "PlaylistUser"
    }
    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        Ok(req
            .uri()
            .path()
            .split('/')
            .collect::<Vec<_>>()
            .get(2)
            .unwrap_or(&"playlist")
            .to_string())
    }
    fn key_name(&self, key: &Self::Key) -> Option<String> {
        Some(key.to_string())
    }
}

pub(crate) fn app_router(
    tracker: TaskTracker,
    token: CancellationToken,
    state: AppState,
) -> Router {
    let playlist_governor_conf = Box::new(
        GovernorConfigBuilder::default()
            .key_extractor(PlaylistUser)
            .period(Duration::from_secs(180))
            .burst_size(3)
            .finish()
            .unwrap(),
    );

    let playlist_governor_limiter = playlist_governor_conf.limiter().clone();

    tracker.spawn(async move {
        let interval = std::time::Duration::from_secs(60);

        'outer: loop {
            tokio::select! {
                _ = token.cancelled() => {
                    tracing::warn!("Playlist rate limiting is shutting down...");
                    break 'outer;
                }
                _ = tokio::time::sleep(interval) => {}
            }

            tracing::debug!(
                "Playlist rate limiting storage size: {}",
                playlist_governor_limiter.len()
            );

            playlist_governor_limiter.retain_recent();
        }

        tracing::warn!("Playlist rate limiting shut down.");
    });

    Router::new()
        .route("/playlist/:user/:id", get(playlist))
        .layer(GovernorLayer {
            config: Box::leak(playlist_governor_conf),
        })
        .route("/health_check", get(health_check))
        .route("/bl-oauth/", get(bl_oauth))
        .route("/bl-oauth", get(bl_oauth))
        .nest("/api", api::router())
        .nest("/", web::router())
        .with_state(state)
}

#[tracing::instrument(skip(_state), level=tracing::Level::INFO, name="webserver:playlist")]
async fn playlist(
    State(_state): State<AppState>,
    Path((user, id)): Path<(String, String)>,
) -> (StatusCode, impl IntoResponse) {
    (StatusCode::OK, {
        let mut response = Response::new("this is a playlist contents".to_string());

        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, "application/json".parse().unwrap());

        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"playlist.bplist\"".parse().unwrap(),
        );

        response
    })

    // (StatusCode::OK, format!("user: {}, id: {}", user, id))
}

#[tracing::instrument(level=tracing::Level::INFO, name="webserver:health_check")]
async fn health_check() -> StatusCode {
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Params {
    #[serde(deserialize_with = "empty_string_as_none")]
    code: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    iss: Option<String>,
    #[serde(deserialize_with = "empty_string_as_none")]
    state: Option<String>,
}

/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
    }
}

#[tracing::instrument(level=tracing::Level::INFO, name="webserver:bl-oauth")]
async fn bl_oauth(
    Query(params): Query<Params>,
    State(_settings): State<AppState>,
) -> (StatusCode, String) {
    (StatusCode::OK, format!("{params:?}"))
}
