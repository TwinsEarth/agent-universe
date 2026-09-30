use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

#[async_trait]
pub trait SecureKeyring: Send + Sync {
    async fn store(&self, key: &str, secret: &[u8]) -> Result<(), KeyringError>;
    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, KeyringError>;
    async fn delete(&self, key: &str) -> Result<(), KeyringError>;
}

#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("key not found")]
    NotFound,
    #[error("platform error: {0}")]
    PlatformError(String),
}

pub struct MemoryKeyring {
    data: Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for MemoryKeyring {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryKeyring {
    pub fn new() -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl SecureKeyring for MemoryKeyring {
    async fn store(&self, key: &str, secret: &[u8]) -> Result<(), KeyringError> {
        self.data
            .lock()
            .map_err(|e| KeyringError::PlatformError(e.to_string()))?
            .insert(key.to_string(), secret.to_vec());
        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, KeyringError> {
        Ok(self
            .data
            .lock()
            .map_err(|e| KeyringError::PlatformError(e.to_string()))?
            .get(key)
            .cloned())
    }

    async fn delete(&self, key: &str) -> Result<(), KeyringError> {
        self.data
            .lock()
            .map_err(|e| KeyringError::PlatformError(e.to_string()))?
            .remove(key);
        Ok(())
    }
}
