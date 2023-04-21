use crate::{
    error::TerrashineError,
    http::index::refresh_versions,
    registry::{ProviderVersions, RegistryClient},
};
use sqlx::PgPool;
use std::{
    collections::{hash_map::Entry, HashMap},
    time::{Duration, Instant},
};
use tokio::sync::{self, oneshot};
use tracing::{info_span, Instrument, Span};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct TerraformProvider {
    pub hostname: String,
    pub namespace: String,
    pub provider_type: String,
}

#[derive(Debug)]
pub(crate) struct RefreshRequest {
    pub(crate) provider: TerraformProvider,
    pub(crate) response_channel: Option<oneshot::Sender<RefreshResponse>>,
    pub(crate) span: Span,
}

#[derive(Debug)]
pub(crate) enum RefreshResponse {
    /// Returned when the request was acted on
    RefreshPerformed(Result<ProviderVersions, TerrashineError>),

    /// This response occurs when a refresh has been sent but the time since
    /// last refresh has not passed.
    /// This response can occur when multiple requests to the refresher has
    /// been sent concurrently, only the first received request will receive a
    /// response with RefreshPerformed.
    ProviderVersionNotStale,
}

pub(crate) async fn refresher(
    db: &PgPool,
    registry: &RegistryClient,
    mut rx: sync::mpsc::Receiver<RefreshRequest>,
    refresh_interval: Duration,
) {
    let mut last_refresh = HashMap::new();
    while let Some(message) = rx.recv().await {
        // Set up tracing span relationship based one whether a response is expected.
        let span;
        if message.response_channel.is_some() {
            span =
                info_span!(parent: message.span, "refresh_request", provider = ?message.provider );
        } else {
            span = info_span!("refresh_request", provider = ?message.provider );
            span.follows_from(message.span);
        }

        async {
            tracing::debug!("Received refresh request");
            let provider = message.provider;
            let response_channel = message.response_channel;
            let last_refreshed_entry = last_refresh.entry(provider);

            match last_refreshed_entry {
                Entry::Vacant(v) => {
                    tracing::info!(
                        "Terraform provider not known to local instance, requesting upstream"
                    );
                    let key = v.key();
                    let result = refresh_versions(
                        db,
                        registry,
                        key.hostname.as_str(),
                        key.namespace.as_str(),
                        key.provider_type.as_str(),
                    )
                    .await;
                    if result.is_ok() {
                        v.insert(Instant::now());
                    }
                    if let Some(sender) = response_channel {
                        let result = sender.send(RefreshResponse::RefreshPerformed(result));
                        if let Err(e) = result {
                            tracing::error!(reason=?e, "Error responding to refresh request");
                        }
                    }
                }
                Entry::Occupied(mut o) if Instant::now() - *o.get() > refresh_interval => {
                    tracing::info!("Provider is stale, updating provider");
                    let key = o.key();
                    let result = refresh_versions(
                        db,
                        registry,
                        key.hostname.as_str(),
                        key.namespace.as_str(),
                        key.provider_type.as_str(),
                    )
                    .await;
                    // If an error occurs here, it isn't critical, just leave it for a bit
                    o.insert(Instant::now());
                    if let Some(sender) = response_channel {
                        let result = sender.send(RefreshResponse::RefreshPerformed(result));
                        if let Err(e) = result {
                            tracing::error!(reason=?e, "Error responding to refresh request");
                        }
                    }
                }
                // Do nothing if interval has not passed.
                Entry::Occupied(_) => {
                    tracing::trace!("Provider is not stale, ignoring request to refresh");
                    if let Some(sender) = response_channel {
                        let result = sender.send(RefreshResponse::ProviderVersionNotStale);
                        if let Err(e) = result {
                            tracing::error!(reason=?e, "Error responding to refresh request");
                        }
                    }
                }
            };
        }
        .instrument(span)
        .await
    }
}
