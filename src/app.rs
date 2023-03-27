use std::collections::hash_map::{DefaultHasher, RandomState};

use axum::{extract::Path, routing::get, Router};
use moka::future::Cache;
use sqlx::{Connection, Pool, Postgres};
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::index;
use crate::version;

#[derive(Clone)]
pub struct AppState {
    pub http_client: reqwest::Client,
    pub db_client: Pool<Postgres>,
    pub meta_cache: Cache<(String, String, String), String>,
}

impl AppState {
    pub fn new(
        db: Pool<Postgres>,
        http: reqwest::Client,
        read_cache: Cache<(String, String, String), String>,
    ) -> AppState {
        AppState {
            http_client: http,
            db_client: db,
            meta_cache: read_cache,
        }
    }
}

pub fn provider_mirror_app(state: AppState) -> Router {
    Router::new()
        .route(
            "/:hostname/:namespace/:provider_type/index.json",
            get(index::index_handler),
        )
        .route(
            "/:hostname/:namespace/:provider_type/:version",
            get(version::version_handler),
        )
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
