use std::time::Duration;
use std::{fmt, str::FromStr};

use axum::extract::Path;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, extract::State, http::header, routing::get, Json, Router};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::{governor::GovernorConfigBuilder, GovernorError, GovernorLayer};

use crate::discord::bot::commands::playlist::Playlist;
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

#[tracing::instrument(skip(state), level=tracing::Level::INFO, name="webserver:playlist")]
async fn playlist(
    State(state): State<AppState>,
    Path((player_id, playlist_id)): Path<(String, String)>,
) -> (StatusCode, impl IntoResponse) {
    match state.playlists_repository.get(&playlist_id).await {
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": {"code": "not_found", "message": "Playlist not found"}})).into_response(),
        ),
        Some(ref repository_playlist) => match repository_playlist.custom_data {
            None => (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"error": {"code": "not_sync", "message": "Playlist cannot be synchronized"}}),
                ).into_response(),
            ),
            Some(ref custom_data) => {
                if custom_data.player_id != player_id {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(
                            json!({"error": {"code": "forbidden", "message": "Playlist belongs to another player"}}),
                        ).into_response(),
                    );
                }

                match Playlist::for_clan_player(
                    &state.player_scores_repository,
                    state.settings.server.url.as_str(),
                    custom_data.clan_tag.clone(),
                    custom_data.player_id.clone(),
                    custom_data.playlist_type.clone(),
                    custom_data.last_played.clone(),
                    custom_data.count,
                )
                .await
                {
                    Ok(mut refreshed_playlist) => {
                        refreshed_playlist.set_id(repository_playlist.get_id().clone());

                        let _ = &state
                            .playlists_repository
                            .save(refreshed_playlist.clone())
                            .await;

                        let mut response = Json(json!(
                            refreshed_playlist.set_image(Playlist::default_image())
                        ))
                        .into_response();

                        response.headers_mut().insert(
                            header::CONTENT_DISPOSITION,
                            format!(
                                "attachment; filename=\"{}.json",
                                refreshed_playlist.get_title().replace([' ', '-'], "_")
                            ).parse().unwrap(),
                        );

                        (StatusCode::OK, response)
                    }
                    Err(err) => (
                        StatusCode::BAD_GATEWAY,
                        Json(
                            json!({"error": {"code": "bl_error", "message": format!("Playlist generating error: {}", err)}}),
                        ).into_response(),
                    ),
                }
            }
        },
    }
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
