use serde::Deserialize;
use url::Url;

/// Types in this module correspond to the API responses from hashicorp,
/// which can be found at
/// https://developer.hashicorp.com/terraform/internals/provider-registry-protocol

// Terraform registry provider API response for "List Available Versions"

#[derive(Deserialize, Debug)]
pub struct ProviderVersions {
    pub versions: Vec<ProviderVersionItem>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct ProviderVersionItem {
    pub version: String,
    pub protocols: Vec<String>,
    pub platforms: Vec<ProviderPlatform>,
}

#[derive(Deserialize, Debug)]
pub struct ProviderPlatform {
    pub os: String,
    pub arch: String,
}

// Terraform registry provider API response for "Find a provider package"

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ProviderResponse {
    pub protocols: Vec<String>,
    pub os: String,
    pub arch: String,
    pub filename: String,
    pub download_url: Url,
    pub shasums_url: Url,
    pub shasums_signature_url: Url,
    pub shasum: String,
    pub signing_keys: ProviderSigningKeys,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct ProviderSigningKeys {
    pub gpg_public_keys: Vec<ProviderGPGPublicKey>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct ProviderGPGPublicKey {
    pub key_id: String,
    pub ascii_armor: String,
}
