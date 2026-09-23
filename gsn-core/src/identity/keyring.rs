use std::collections::HashMap;
use std::sync::Mutex;
use async_trait::async_trait;

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
        self.data.lock().unwrap().insert(key.to_string(), secret.to_vec());
        Ok(())
    }

    async fn load(&self, key: &str) -> Result<Option<Vec<u8>>, KeyringError> {
        Ok(self.data.lock().unwrap().get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<(), KeyringError> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }
}
