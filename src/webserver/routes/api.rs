use crate::webserver::AppState;
use axum::extract::{Path, State};
use axum::headers::HeaderMap;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/json/:param", get(json_handler))
        .route("/test-error", get(test_error))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Test {
    id: u32,
    first: String,
    second: i32,
    accept_header: Option<String>,
}

#[tracing::instrument(skip(_settings, headers), level=tracing::Level::INFO, name="webserver:api:some_handler")]
async fn json_handler(
    State(_settings): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<u32>,
) -> (StatusCode, impl IntoResponse) {
    let accept_header = match headers.get("Accept").map(|h| h.as_bytes()) {
        None => "None",
        Some(h) => std::str::from_utf8(h).unwrap(),
    };

    tracing::debug!(?accept_header, "accept header fetched");

    let obj = Test {
        id,
        first: "test string".to_string(),
        second: 42,
        accept_header: Some(accept_header.to_owned()),
    };

    tracing::debug!(?obj, "obj created");

    (StatusCode::OK, Json(obj))
}

#[derive(thiserror::Error)]
pub(crate) enum TestError {
    #[error("Something bad happened")]
    SomethingBad,
}

impl IntoResponse for TestError {
    #[tracing::instrument(skip_all, level=tracing::Level::INFO, name = "webserver:api:test_error")]
    fn into_response(self) -> Response {
        tracing::error!("TestError has occurred: {:?}", self);

        match self {
            Self::SomethingBad => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": {"status": StatusCode::SERVICE_UNAVAILABLE.as_u16(), "message": format!("{:?}", self).trim()}})),
            )
                .into_response(),
        }
    }
}

impl std::fmt::Debug for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self)?;
        Ok(())
    }
}

#[tracing::instrument(skip_all, level=tracing::Level::INFO, name = "api:test_error")]
async fn test_error() -> Result<impl IntoResponse, TestError> {
    Err::<&'static str, TestError>(TestError::SomethingBad)
}
