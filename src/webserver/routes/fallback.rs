use axum::headers::HeaderMap;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

#[tracing::instrument(level=tracing::Level::INFO, name="webserver:fallback")]
pub(crate) async fn fallback(uri: Uri) -> (StatusCode, impl IntoResponse) {
    (
        StatusCode::NOT_FOUND,
        if uri.path().starts_with("/api") {
            Json(json!({"error": {"status": 404, "message": "Not found"}})).into_response()
        } else {
            let mut headers = HeaderMap::new();
            headers.insert("Content-type", "text/html; charset=utf-8".parse().unwrap());

            (headers, include_str!("../../../static/404.html")).into_response()
        },
    )
}
