use std::str;

use reqwest::{Client, Response};
use serde::Deserialize;

const DISCOVERY_RESPONSE_SIZE_MAX_BYTES: usize = 16384; // 16KB
const REGISTRY_METADATA_SIZE_MAX_BYTES: usize = 8388608; // 8MB

#[derive(Clone)]
pub struct RegistryClient {
    http: Client,
}

impl RegistryClient {
    pub fn new(http: Client) -> Self {
        RegistryClient { http }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RegistryError {
    #[error("Response received upstream was too large, limit of {limit} reached")]
    ResponseTooLarge { limit: usize },
    #[error("HTTP Error from upstream")]
    UpstreamResponseFailure {
        #[from]
        source: reqwest::Error,
    },
    #[error("Error deserializing json response from registry")]
    DeserializationError {
        #[from]
        source: serde_json::Error,
    },
    #[error("Terraform {service_type} service not supported by {hostname}")]
    TerraformServiceNotSupported {
        service_type: &'static str,
        hostname: String,
    },
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DiscoveredServices {
    #[serde(rename = "providers.v1")]
    providers_v1: Option<String>,
    #[serde(rename = "modules.v1")]
    modules_v1: Option<String>,
    #[serde(rename = "login.v1")]
    login_v1: Option<String>,
}

async fn read_body_limit(
    buffer: &mut Vec<u8>,
    mut response: Response,
    limit: usize,
) -> Result<(), RegistryError> {
    while let Some(chunk) = response.chunk().await? {
        if buffer.len() + chunk.len() > limit {
            return Err(RegistryError::ResponseTooLarge { limit });
        }
        buffer.extend(chunk);
    }
    Ok(())
}

impl RegistryClient {
    /// Performs request upstream to handle terraform service discovery protocol
    async fn discover_services(&self, hostname: &str) -> Result<DiscoveredServices, RegistryError> {
        let url = format!("https://{}/.well-known/terraform.json", hostname);
        let mut response_buffer = Vec::with_capacity(DISCOVERY_RESPONSE_SIZE_MAX_BYTES);
        let response = self.http.get(url).send().await?.error_for_status()?;
        read_body_limit(
            &mut response_buffer,
            response,
            DISCOVERY_RESPONSE_SIZE_MAX_BYTES,
        )
        .await?;
        let services = serde_json::from_slice(&response_buffer[..])?;
        Ok(services)
    }

    pub async fn provider_get<A: for<'a> Deserialize<'a>>(
        &self,
        hostname: impl AsRef<str>,
        path: impl AsRef<str>,
    ) -> Result<A, RegistryError> {
        let hostname = hostname.as_ref();
        let services = self.discover_services(hostname).await?;
        if let Some(base_url) = services.providers_v1 {
            let path = path.as_ref();
            let url = format!("https://{hostname}{base_url}{path}");
            let mut response_buffer = Vec::with_capacity(REGISTRY_METADATA_SIZE_MAX_BYTES);
            tracing::debug!(%url, "GET registry provider");
            let response = self.http.get(&url).send().await?.error_for_status()?;
            read_body_limit(
                &mut response_buffer,
                response,
                REGISTRY_METADATA_SIZE_MAX_BYTES,
            )
            .await?;
            let result = serde_json::from_slice(&response_buffer[..])?;
            Ok(result)
        } else {
            Err(RegistryError::TerraformServiceNotSupported {
                service_type: "provider",
                hostname: hostname.to_string(),
            })
        }
    }
}
