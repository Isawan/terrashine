use serde::{de, Deserialize, Deserializer, Serialize};
use std::{collections::HashMap, error::Error, fmt::Display, string::ParseError};
use url::{form_urlencoded::Parse, Url};

use axum::Json;
use hyper::StatusCode;

#[derive(Serialize)]
struct MirrorVersions<'a> {
    archives: HashMap<&'a str, &'a str>,
}

#[derive(Debug, Deserialize)]
struct ProviderResponse<'a> {
    protocols: Vec<&'a str>,
    os: &'a str,
    arch: &'a str,
    filename: &'a str,
    download_url: Url,
    shasums_url: Url,
    shasums_signature_url: Url,
    shasum: &'a str,
    signing_keys: ProviderSigningKeys<'a>,
}

#[derive(Deserialize, Debug)]
struct ProviderSigningKeys<'a> {
    #[serde(borrow)]
    gpg_public_keys: ProviderGPGPublicKeys<'a>,
}

#[derive(Deserialize, Debug)]
struct ProviderGPGPublicKeys<'a> {
    #[serde(borrow)]
    keys: Vec<ProviderGPGPublicKey<'a>>,
}

#[derive(Deserialize, Debug)]
struct ProviderGPGPublicKey<'a> {
    key_id: &'a str,
    ascii_armor: &'a str,
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

fn version<'a>(
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<Json<MirrorVersions<'a>>, StatusCode> {
    todo!()
}
