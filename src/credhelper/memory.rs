use std::{collections::HashMap, marker::Send};

use async_trait::async_trait;

use super::{types::Credential, CredentialHelper};

// Credential helper implementation by storing in the database
#[derive(Clone)]
pub struct MemoryCredentials {
    map: HashMap<String, Option<String>>,
}

impl MemoryCredentials {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

impl Default for MemoryCredentials {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialHelper for MemoryCredentials {
    async fn get(&self, hostname: impl AsRef<str> + Send) -> Result<Credential, anyhow::Error> {
        Ok(self
            .map
            .get(hostname.as_ref())
            .map_or(Credential::NotFound, |v| Credential::Entry(v.clone())))
    }

    async fn store(&mut self, hostname: String, cred: String) -> Result<(), anyhow::Error> {
        self.map.insert(hostname, Some(cred));
        Ok(())
    }

    async fn forget(&mut self, hostname: impl AsRef<str> + Send) -> Result<(), anyhow::Error> {
        self.map.remove(hostname.as_ref());
        Ok(())
    }
}
