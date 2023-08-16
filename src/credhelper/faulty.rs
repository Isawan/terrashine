/// Implementation of the memory credential helper that always throws an error
///
/// This is useful for testing the error handling of the credential helper
use async_trait::async_trait;

use super::{Credential, CredentialHelper};

#[derive(Clone, Debug)]
pub(crate) struct FaultyCredentials;

impl FaultyCredentials {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CredentialHelper for FaultyCredentials {
    async fn get(&self, _hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error> {
        Err(anyhow::anyhow!("Error occurred"))
    }

    async fn store(&mut self, _hostname: String, _cred: String) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Error occurred"))
    }

    async fn forget(&mut self, _hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error> {
        Err(anyhow::anyhow!("Error occurred"))
    }
}
