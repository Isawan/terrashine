use http::{header::CONTENT_TYPE, HeaderValue};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, hash::Hash};
use tracing::{span, Level};
use url::ParseError;

use axum::{response::Response, Json};
use hyper::{header::HeaderName, HeaderMap, StatusCode};

const MAX_INDEX_RESPONSE: u64 = 4_000_000;

#[derive(Serialize)]
struct MirrorIndex<'a> {
    // TODO: the nested hahsmap value is always empty, we should implement
    // custom serialize to avoid unneeded work.
    versions: HashMap<&'a str, HashMap<&'a str, &'a str>>,
}

#[derive(Deserialize)]
struct ProviderVersions<'a> {
    #[serde(borrow)]
    versions: Vec<ProviderVersionItem<'a>>,
}

#[derive(Deserialize)]
struct ProviderVersionItem<'a> {
    version: &'a str,
    protocols: Vec<&'a str>,
    platforms: Vec<ProviderPlatform<'a>>,
}

#[derive(Deserialize)]
struct ProviderPlatform<'a> {
    os: &'a str,
    arch: &'a str,
}

pub async fn index<'a>(
    http: reqwest::Client,
    hostname: &str,
    namespace: &str,
    provider_type: &str,
) -> Result<(HeaderMap, String), StatusCode> {
    let url = build_url(hostname, namespace, provider_type).map_err(|_| StatusCode::BAD_GATEWAY)?;

    let response = http
        .get(url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let body = response
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    let provider_versions: ProviderVersions =
        serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_GATEWAY)?;
    let mirror_index = MirrorIndex::from(provider_versions);
    let response_body = serde_json::to_string(&mirror_index).expect("Failed to serialize");
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    Result::Ok((headers, response_body))
}

fn build_url(hostname: &str, namespace: &str, provider_type: &str) -> Result<Url, ParseError> {
    let mut url_builder = String::new();
    url_builder.push_str("https://");
    url_builder.push_str(hostname);
    url_builder.push_str("/v1/providers/");
    url_builder.push_str(namespace);
    url_builder.push_str("/");
    url_builder.push_str(provider_type);
    url_builder.push_str("/versions");

    Url::parse(&url_builder)
}

impl<'a> From<ProviderVersions<'a>> for MirrorIndex<'a> {
    fn from(provider_versions: ProviderVersions<'a>) -> MirrorIndex {
        let mut versions = HashMap::new();
        for version in provider_versions.versions.iter() {
            versions.insert(version.version, HashMap::new());
        }
        MirrorIndex { versions }
    }
}
