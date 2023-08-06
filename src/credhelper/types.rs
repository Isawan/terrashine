use async_trait::async_trait;
use std::marker::Send;

#[async_trait]
pub trait CredentialHelper {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Option<String>, anyhow::Error>;
    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error>;

    /// Forget a credential.
    /// NOTE: Deleting a non-existing credential is not an error
    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error>;
}
