use http::StatusCode;

pub(crate) async fn healthcheck_handler() -> StatusCode {
    StatusCode::OK
}
