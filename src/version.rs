use http::HeaderMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

use axum::extract::{Path, State};
use hyper::StatusCode;

use crate::app::AppState;

#[derive(Serialize)]
struct MirrorVersions {
    archives: HashMap<String, MirrorDownloadDetail>,
}

#[derive(Serialize)]
struct MirrorDownloadDetail {
    url: Url,
    hashes: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderResponse {
    protocols: Vec<String>,
    os: String,
    arch: String,
    filename: String,
    download_url: Url,
    shasums_url: Url,
    shasums_signature_url: Url,
    shasum: String,
    signing_keys: ProviderSigningKeys,
}

#[derive(Deserialize, Debug)]
struct ProviderSigningKeys {
    gpg_public_keys: ProviderGPGPublicKeys,
}

#[derive(Deserialize, Debug)]
struct ProviderGPGPublicKeys {
    keys: Vec<ProviderGPGPublicKey>,
}

#[derive(Deserialize, Debug)]
struct ProviderGPGPublicKey {
    key_id: String,
    ascii_armor: String,
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct Version {
    full: String,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("Could not parse {0}")]
pub struct VersionParseError(String);

impl Version {
    fn prefix(&self) -> &str {
        // This is safe as we verified this when creating the type.
        self.full.strip_suffix(".json").unwrap()
    }
}

impl TryFrom<String> for Version {
    type Error = VersionParseError;
    fn try_from(value: String) -> Result<Version, VersionParseError> {
        match value.strip_suffix(".json") {
            Some(_) => Ok(Version { full: value }),
            None => Err(VersionParseError(value)),
        }
    }
}

fn archive_name(os: &str, arch: &str) -> String {
    let mut s = String::with_capacity(os.len() + 1 + arch.len());
    s.push_str(os);
    s.push('_');
    s.push_str(arch);
    s
}

pub async fn version_handler<'a>(
    State(AppState {
        http_client: http, ..
    }): State<AppState>,
    Path((hostname, namespace, provider_type, version_json)): Path<(
        String,
        String,
        String,
        Version,
    )>,
) -> Result<(HeaderMap, String), StatusCode> {
    todo!();
}
