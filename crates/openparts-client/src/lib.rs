//! openparts-client: abstracts communication with an `openparts-server`
//! instance (search, retrieval, cache, hash validation, lock resolution,
//! offline fallback — Architecture Specification section 7.4).
//!
//! TODO: not implemented for the first vertical slice. The slice runs
//! entirely against local `openparts-data` files (Architecture
//! Specification section 4.8, "Server is Optional") — there is no server
//! to talk to yet. Wire this up once `openparts-server` exists.

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("openparts-client is not implemented yet (no server exists to talk to)")]
    NotImplemented,
}

pub struct Client {
    pub registry_url: String,
}

impl Client {
    pub fn new(registry_url: impl Into<String>) -> Self {
        Self {
            registry_url: registry_url.into(),
        }
    }

    pub fn search(&self, _query: &str) -> Result<Vec<String>, ClientError> {
        Err(ClientError::NotImplemented)
    }
}
