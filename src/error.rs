use axum::response::IntoResponse;
use http::StatusCode;
use crate::refresh::TerraformProvider;

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
    #[error("Terrashine concurrent upstream fetch occurred when fetching new terraform provider")]
    ConcurrentUpstreamProviderFetch { provider: TerraformProvider },
    #[error("Too many requests in channel ({channel_name}) caused a timeout")]
    TooManyRequestsInChannel { channel_name: &'static str },
    #[error("Broken refresher channel as the receiver has been dropped")]
    BrokenRefresherChannel,
    #[error(transparent)]
    Anyhow {
        #[from]
        source: anyhow::Error,
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
            TerrashineError::Anyhow { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            TerrashineError::ConcurrentUpstreamProviderFetch { .. } => {
                StatusCode::TOO_MANY_REQUESTS
            }
            TerrashineError::TooManyRequestsInChannel { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            TerrashineError::BrokenRefresherChannel => StatusCode::INTERNAL_SERVER_ERROR,
        }
        .into_response()
    }
}
