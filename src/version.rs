use http::HeaderMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Pool};
use std::collections::HashMap;
use tokio_stream::{self as stream, StreamExt};
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

#[derive(Clone, Debug, sqlx::FromRow)]
struct DownloadTuple {
    os: String,
    arch: String,
    url: Option<String>,
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

type Checksum = [u8; 32];

struct DatabaseDownloadResult {
    os: String,
    arch: String,
    sha256sum: Option<Checksum>,
}

async fn list_downloads(
    http: &Client,
    db: &PgPool,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
    version: &str,
) -> Result<Vec<DatabaseDownloadResult>, anyhow::Error> {
    let query = sqlx::query!(
        r#"
        select "os", "arch", "sha256sum"
        from "terraform_provider_version", "terraform_provider"
        where
            "terraform_provider_version"."provider_id" = "terraform_provider"."id"
            and "terraform_provider_version"."version" = $1
            and "terraform_provider"."hostname" = $2
            and "terraform_provider"."namespace" = $3
            and "terraform_provider"."type" = $4;
        "#,
        version,
        hostname,
        namespace,
        provider_type,
    );
    let mut rows = query.fetch(db);
    let mut result = vec![];
    while let Some(row) = rows.try_next().await? {
        if let Some(x) = row.sha256sum {
            let mut buffer = [0u8; 32];

            // Enforced by database schema, these error cases should never occur
            if (x.len() != 64) {
                tracing::error!(
                    expected = 64,
                    found = x.len(),
                    hash = x,
                    "sha256 hexadecimal hash expected, incorrect length."
                );
                panic!();
            }
            if let Err(error) = hex::decode_to_slice(&x, &mut buffer) {
                tracing::error!(
                    reason = ?error,
                    "sha256 hexadecimal hash expected, could not parse."
                );
                panic!();
            }

            result.push(DatabaseDownloadResult {
                os: row.os,
                arch: row.arch,
                sha256sum: Some(buffer),
            });
        } else {
            result.push(DatabaseDownloadResult {
                os: row.os,
                arch: row.arch,
                sha256sum: None,
            });
        }
    }
    Ok(result)
}
