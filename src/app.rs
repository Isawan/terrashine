use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use http::{HeaderMap, StatusCode};
use tower_http::{
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::Level;

use crate::index;
use crate::version::Version;

#[derive(Clone)]
struct AppState {
    http_client: reqwest::Client,
}

impl AppState {
    fn new() -> AppState {
        AppState {
            http_client: reqwest::Client::new(),
        }
    }
}

pub fn provider_mirror_app() -> Router {
    Router::new()
        .route(
            "/:hostname/:namespace/:provider_type/index.json",
            get(index_handler),
        )
        .route(
            "/:hostname/:namespace/:provider_type/:version",
            get(version_handler),
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
        .with_state(AppState::new())
}

async fn index_handler(
    State(state): State<AppState>,
    Path((hostname, namespace, provider_type)): Path<(String, String, String)>,
) -> Result<(HeaderMap, String), StatusCode> {
    index::index(state.http_client, &hostname, &namespace, &provider_type).await
}

async fn version_handler<'a>(
    State(state): State<AppState>,
    Path((hostname, namespace, provider_type, version_json)): Path<(
        String,
        String,
        String,
        Version,
    )>,
) -> Result<(HeaderMap, String), StatusCode> {
    todo!();
}
