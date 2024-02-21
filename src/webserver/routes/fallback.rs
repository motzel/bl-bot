use axum::http::{header, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::json;

#[tracing::instrument(level=tracing::Level::INFO, name="webserver:fallback")]
pub(crate) async fn fallback(uri: Uri) -> (StatusCode, impl IntoResponse) {
    (
        StatusCode::NOT_FOUND,
        if uri.path().starts_with("/api") {
            Response::new(json!({"error": {"status": 404, "message": "Not found"}}).to_string())
        } else {
            let mut response = Response::new(include_str!("../../../static/404.html").to_string());
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                "text/html; charset=utf-8".parse().unwrap(),
            );

            response
        },
    )
}
