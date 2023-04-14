use std::{backtrace::Backtrace, time::Duration};

use axum::response::IntoResponse;
use http::StatusCode;
use tower_http::classify::ServerErrorsFailureClass;
use tracing::Span;

#[derive(Debug, thiserror::Error)]
pub enum TerrashineError {
    #[error("Error making request to database")]
    DatabaseError {
        #[from]
        source: sqlx::Error,
    },
    #[error("Response received upstream was too large, limit of {limit} bytes reached")]
    ProviderResponseTooLarge { limit: usize },
    #[error("HTTP Error from upstream")]
    ProviderResponseFailure {
        #[from]
        source: reqwest::Error,
    },
    #[error("Error deserializing json response from registry")]
    ProviderDeserializationError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Terraform {service_type} service not supported by {hostname}")]
    TerraformServiceNotSupported {
        service_type: &'static str,
        hostname: String,
    },
}

impl IntoResponse for TerrashineError {
    fn into_response(self) -> axum::response::Response {
        match self {
            TerrashineError::DatabaseError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            TerrashineError::ProviderResponseTooLarge { .. } => StatusCode::BAD_GATEWAY,
            TerrashineError::ProviderResponseFailure { .. } => StatusCode::BAD_GATEWAY,
            TerrashineError::ProviderDeserializationError { .. } => StatusCode::BAD_GATEWAY,
            TerrashineError::TerraformServiceNotSupported { .. } => StatusCode::BAD_GATEWAY,
        }
        .into_response()
    }
}
