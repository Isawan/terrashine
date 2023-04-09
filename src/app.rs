use axum::{routing::get, Router};
use sqlx::{Pool, Postgres};
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::{artifacts::artifacts_handler, index::index_handler, version::version_handler};

#[derive(Clone)]
pub struct AppState {
    pub s3_client: aws_sdk_s3::Client,
    pub http_client: reqwest::Client,
    pub db_client: Pool<Postgres>,
}

impl AppState {
    pub fn new(
        s3: aws_sdk_s3::Client,
        db: Pool<Postgres>,
        http: reqwest::Client,
    ) -> AppState {
        AppState {
            s3_client: s3,
            http_client: http,
            db_client: db,
        }
    }
}

pub fn provider_mirror_app(state: AppState) -> Router {
    Router::new()
        .route(
            "/:hostname/:namespace/:provider_type/index.json",
            get(index_handler),
        )
        .route(
            "/:hostname/:namespace/:provider_type/:version",
            get(version_handler),
        )
        .route("/artifacts/:version_id", get(artifacts_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .with_state(state)
}
