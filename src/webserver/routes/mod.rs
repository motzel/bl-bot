use std::{fmt, str::FromStr};

use axum::http::StatusCode;
use axum::{extract::Query, extract::State, routing::get, Router};
use serde::{de, Deserialize, Deserializer};

use crate::webserver::AppState;

mod api;
mod fallback;
mod web;

pub(crate) fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/health_check", get(health_check))
        .route("/bl-oauth/", get(bl_oauth))
        .route("/bl-oauth", get(bl_oauth))
        .nest("/api", api::router())
        .nest("/", web::router())
        .with_state(state)
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
