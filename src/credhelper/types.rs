use async_trait::async_trait;
use reqwest::RequestBuilder;
use std::marker::Send;

#[derive(PartialEq, Eq)]
pub enum Credential {
    NotFound,
    Entry(Option<String>),
}

#[async_trait]
pub trait CredentialHelper: Sync {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error>;
    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error>;

    /// Forget a credential.
    /// NOTE: Deleting a non-existing credential is not an error
    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error>;

    async fn transform(
        &self,
        request: RequestBuilder,
        hostname: impl AsRef<str> + Send,
    ) -> Result<RequestBuilder, anyhow::Error> {
        match self.get(hostname).await? {
            Credential::NotFound => Ok(request),
            Credential::Entry(None) => Ok(request),
            Credential::Entry(Some(token)) => Ok(request.bearer_auth(token)),
        }
    }
}
