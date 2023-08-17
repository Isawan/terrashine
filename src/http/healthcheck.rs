use crate::http::response_types::Empty;
use axum::Json;
use http::StatusCode;

pub(crate) async fn healthcheck_handler() -> (StatusCode, Json<Empty>) {
    (StatusCode::OK, Json(Empty {}))
}
