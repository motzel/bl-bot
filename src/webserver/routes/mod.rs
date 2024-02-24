use std::sync::Arc;
use std::time::Duration;
use std::{fmt, str::FromStr};

use axum::extract::Path;
use axum::http::{Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::{extract::Query, extract::State, http::header, routing::get, Json, Router};
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tower_governor::key_extractor::KeyExtractor;
use tower_governor::{governor::GovernorConfigBuilder, GovernorError, GovernorLayer};

use crate::beatleader::oauth::OAuthAppCredentials;
use crate::discord::bot::commands::playlist::Playlist;
use crate::discord::bot::GuildOAuthTokenRepository;
use crate::webserver::AppState;
use crate::BL_CLIENT;
use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use poise::serenity_prelude::GuildId;

mod api;
mod fallback;
mod web;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct PlaylistUserExtractor;

impl KeyExtractor for PlaylistUserExtractor {
    type Key = String;

    fn name(&self) -> &'static str {
        "PlaylistUserExtractor"
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
            .key_extractor(PlaylistUserExtractor)
            .period(Duration::from_secs(180))
            .burst_size(3)
            .use_headers()
            .error_handler(|err| match err {
                GovernorError::TooManyRequests { wait_time, headers } => {
                    let (mut parts, body) = Json(json!({"error": {"code": "rate_limit", "message": format!("Too Many Requests! Wait for {}s", wait_time), "retry_after": wait_time}}))
                        .into_response()
                        .into_parts();

                    parts.status = StatusCode::TOO_MANY_REQUESTS;
                    if let Some(headers) = headers {
                        headers.into_iter().for_each(|(name_option, value)| if let Some(name) = name_option {parts.headers.insert(name, value);});
                    }

                    Response::from_parts(parts, body)
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": {"code": "internal_rate_limit_error", "message": "Unknown error"}})).into_response(),
                ).into_response(),
            })
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

#[tracing::instrument(skip(app_state), level=tracing::Level::INFO, name="webserver:playlist")]
async fn playlist(
    State(app_state): State<AppState>,
    Path((player_id, playlist_id)): Path<(String, String)>,
) -> (StatusCode, impl IntoResponse) {
    match app_state.playlists_repository.get(&playlist_id).await {
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
                    &app_state.player_scores_repository,
                    app_state.settings.server.url.as_str(),
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

                        let _ = &app_state
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
    #[serde(default, deserialize_with = "empty_string_as_none")]
    code: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    iss: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
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

#[tracing::instrument(skip(app_state), level=tracing::Level::INFO, name="webserver:bl-oauth")]
async fn bl_oauth(
    Query(params): Query<Params>,
    State(app_state): State<AppState>,
) -> (StatusCode, String) {
    let oauth_settings = match app_state.settings.oauth {
        None => return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured."
                .to_string(),
        ),
        Some(oauth_settings) => oauth_settings.clone()
    };

    let error_response = (
        StatusCode::BAD_GATEWAY,
        "Something went wrong.\n\nNo authorization code or oauth state in response, can not continue."
            .to_string(),
    );

    let auth_code = match params.code {
        None => return error_response.clone(),
        Some(code) => {
            if code.is_empty() {
                return error_response.clone();
            }

            code
        }
    };

    match params.state {
        Some(state) => {
            let mc = new_magic_crypt!(oauth_settings.client_secret.as_str(), 256);

            match mc.decrypt_base64_to_string(state.as_str()) {
                Err(err) => {
                    let err_string = format!("Can not decode oauth state: {}", err);
                    tracing::error!("{}", err_string.as_str());

                    return (StatusCode::BAD_REQUEST, err_string);
                }
                Ok(decoded_state) => match decoded_state.parse::<GuildId>() {
                    Err(err) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            format!("Invalid oauth state: {}", err),
                        )
                    }
                    Ok(guild_id) => {
                        match app_state.guild_settings_repository.get(&guild_id).await {
                            Err(_) => {
                                return (StatusCode::BAD_REQUEST, "Invalid guild ID".to_string())
                            }
                            Ok(guild_settings) => {
                                let Some(mut clan_settings) = guild_settings.get_clan_settings()
                                else {
                                    return (StatusCode::BAD_REQUEST, "Clan settings not found, use ``/bl-set-clan-invitation`` command first".to_string());
                                };

                                let oauth_client = BL_CLIENT.with_oauth(
                                    OAuthAppCredentials {
                                        client_id: oauth_settings.client_id.clone(),
                                        client_secret: oauth_settings.client_secret.clone(),
                                        redirect_uri: oauth_settings.redirect_uri.clone(),
                                    },
                                    GuildOAuthTokenRepository::new(
                                        clan_settings.get_owner().clone(),
                                        Arc::clone(&app_state.player_oauth_token_repository),
                                    ),
                                );

                                match oauth_client
                                    .oauth()
                                    .access_token_and_store(auth_code.as_str())
                                    .await {
                                    Err(err) => {
                                        return (StatusCode::BAD_GATEWAY, format!(
                                            "An error has occurred: {}\n\nUse the /bl-set-clan-invitation command again.", err
                                        ))
                                    },
                                    Ok(_) => {
                                        clan_settings.set_oauth_token(true);

                                        let self_invite = clan_settings.supports_self_invitation();

                                        if app_state
                                            .guild_settings_repository
                                            .set_clan_settings(
                                                &guild_settings.get_key(),
                                                Some(clan_settings),
                                            )
                                            .await
                                            .is_err()
                                        {
                                            return (
                                                StatusCode::INTERNAL_SERVER_ERROR,
                                                "An error occurred while saving clan settings".to_string(),
                                            );
                                        }

                                        (
                                            StatusCode::OK,
                                            format!(
                                                "Clan invitation service has been set up.\n\n{}",
                                                if self_invite {
                                                    "Players can use the ``/bl-clan-invitation`` command to send themselves an invitation to join the clan."
                                                } else {
                                                    "You can use the ``/bl-invite-player`` command to send a player an invitation to join the clan."
                                                }
                                            ),
                                        )
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
        None => error_response,
    }
}
