use std::{
    collections::HashMap,
    marker::Send,
    sync::{Arc, RwLock},
};

use super::{types::Credential, CredentialHelper};

// Credential helper implementation by storing in the database
#[derive(Clone, Debug)]
pub struct MemoryCredentials {
    map: Arc<RwLock<HashMap<String, Option<String>>>>,
}

impl MemoryCredentials {
    pub fn new() -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryCredentials {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialHelper for MemoryCredentials {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error> {
        let map = self.map.try_read().map_err(|_| {
            anyhow::anyhow!("Could not acquire read lock on in memory credential store")
        })?;
        Ok(map
            .get(hostname.as_ref())
            .map_or(Credential::NotFound, |v| Credential::Entry(v.clone())))
    }

    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error> {
        self.map
            .try_write()
            .map_err(|_| {
                anyhow::anyhow!("Could not acquire write lock on in memory credential store")
            })?
            .insert(hostname, Some(cred));
        Ok(())
    }

    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error> {
        self.map
            .try_write()
            .map_err(|_| {
                anyhow::anyhow!("Could not acquire write lock on in memory credential store")
            })?
            .remove(hostname.as_ref());
        Ok(())
    }
}
