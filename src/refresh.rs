use std::{
    collections::{
        hash_map::{Entry, RandomState},
        HashMap,
    },
    time::{Duration, Instant},
};

use sqlx::{
    postgres::{types::PgInterval, PgListener},
    PgPool,
};
use tokio::sync::{self, oneshot};

use crate::{
    error::TerrashineError,
    index::{self, refresh_versions, ProviderVersions},
    registry_client::RegistryClient,
};

const SCHEDULER_SPREAD_DENOMINATOR: u128 = 16;

#[derive(Clone, Hash, PartialEq, Eq)]
struct TerraformProvider {
    hostname: String,
    namespace: String,
    provider_type: String,
}

struct RefreshRequest {
    provider: TerraformProvider,
    response_channel: Option<oneshot::Sender<RefreshResponse>>,
}

struct RefreshEventRecord {
    timestamp: u32,
    refresh_time: Instant,
}

enum RefreshResponse {
    /// Returned when the request was acted on
    RefreshPerformed(Result<ProviderVersions, TerrashineError>),

    /// This response occurs when a refresh has been sent but the time since
    /// last refresh has not passed.
    /// This response can occur when multiple requests to the refresher has
    /// been sent concurrently, only the first received request will receive
    /// respond with RefreshPerformed.
    ProviderVersionNotStale,
}

async fn refresher(
    db: &PgPool,
    registry: &RegistryClient,
    refresh_interval: Duration,
    mut rx: sync::mpsc::Receiver<RefreshRequest>,
) {
    let mut last_refresh = HashMap::new();
    while let Some(message) = rx.recv().await {
        let provider = message.provider;
        let response_channel = message.response_channel;
        let last_refreshed_entry = last_refresh.entry(provider);
        match last_refreshed_entry {
            Entry::Vacant(v) => {
                tracing::debug!("Provider not known to local instance");
                let key = v.key();
                let result = refresh_versions(
                    db,
                    registry,
                    key.hostname.as_str(),
                    key.namespace.as_str(),
                    key.provider_type.as_str(),
                )
                .await;
                if let Some(sender) = response_channel {
                    sender.send(RefreshResponse::RefreshPerformed(result));
                }
                v.insert(Instant::now());
            }
            Entry::Occupied(mut o) if Instant::now() - *o.get() > refresh_interval => {
                let key = o.key();
                let result = refresh_versions(
                    db,
                    registry,
                    key.hostname.as_str(),
                    key.namespace.as_str(),
                    key.provider_type.as_str(),
                )
                .await;
                o.insert(Instant::now());
                if let Some(sender) = response_channel {
                    sender.send(RefreshResponse::RefreshPerformed(result));
                }
            }
            // Do nothing if interval has not passed.
            Entry::Occupied(o) => {
                if let Some(sender) = response_channel {
                    sender.send(RefreshResponse::ProviderVersionNotStale);
                }
            }
        };
    }
}
