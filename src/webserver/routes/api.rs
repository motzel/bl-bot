use axum::Router;

use crate::webserver::AppState;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
}
