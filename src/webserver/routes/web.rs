use crate::webserver::routes::fallback;
use crate::webserver::AppState;
use axum::handler::HandlerWithoutStateExt;
use axum::Router;
use tower_http::services::ServeDir;

pub(super) fn router() -> Router<AppState> {
    let serve_dir = ServeDir::new("./static").not_found_service(fallback::fallback.into_service());

    Router::new().nest_service("/", serve_dir)
}
