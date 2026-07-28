//! In-memory mock provider for gtfs-adapter operation tests.
#![allow(missing_docs, clippy::missing_panics_doc)]
// Shared by several test binaries; not every binary uses every helper.
#![allow(dead_code)]

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use omnia_guest::{Config, HttpRequest, Identity, Message, Publish, StateStore};

/// Canned HTTP response bodies keyed by URL prefix.
type CannedResponses = Vec<(String, Vec<u8>)>;

/// A mock provider backed by in-memory maps: configuration keys, state store
/// entries, published messages, and canned HTTP responses keyed by URL
/// prefix.
#[derive(Default, Clone)]
pub struct MockProvider {
    config: Arc<Mutex<HashMap<String, String>>>,
    state: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    published: Arc<Mutex<Vec<(String, Message)>>>,
    responses: Arc<Mutex<CannedResponses>>,
}

impl MockProvider {
    /// A provider pre-populated with the configuration keys the operations
    /// read (see `acme_common::config`).
    #[must_use]
    pub fn new() -> Self {
        let provider = Self::default();
        provider.set_config("ENV", "dev");
        provider.set_config("BLOCK_MGT_URL", "http://block-mgt.test");
        provider.set_config("FLEET_URL", "http://fleet.test");
        provider.set_config("TRIP_MANAGEMENT_URL", "http://trip-mgt.test");
        provider.set_config("STATIC_API_URL", "http://static.test");
        provider.set_config("API_IDENTITY", "test-identity");
        provider
    }

    pub fn set_config(&self, key: &str, value: &str) {
        self.config.lock().expect("lock").insert(key.to_string(), value.to_string());
    }

    /// Register a canned response body for any request whose URI starts with
    /// `url_prefix`.
    pub fn respond_with(&self, url_prefix: &str, body: impl Into<Vec<u8>>) {
        self.responses.lock().expect("lock").push((url_prefix.to_string(), body.into()));
    }

    #[must_use]
    pub fn published(&self) -> Vec<(String, Message)> {
        self.published.lock().expect("lock").clone()
    }

    #[must_use]
    pub fn state(&self, key: &str) -> Option<Vec<u8>> {
        self.state.lock().expect("lock").get(key).cloned()
    }

    pub fn seed_state(&self, key: &str, value: impl Into<Vec<u8>>) {
        self.state.lock().expect("lock").insert(key.to_string(), value.into());
    }
}

impl Config for MockProvider {
    async fn get(&self, key: &str) -> Result<String> {
        self.config
            .lock()
            .expect("lock")
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!("no config value for `{key}`"))
    }
}

impl StateStore for MockProvider {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().expect("lock").get(key).cloned())
    }

    async fn set(
        &self, key: &str, value: &[u8], _ttl_secs: Option<u64>,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().expect("lock").insert(key.to_string(), value.to_vec()))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.state.lock().expect("lock").remove(key);
        Ok(())
    }
}

impl Publish for MockProvider {
    async fn send(&self, topic: &str, message: &Message) -> Result<()> {
        self.published.lock().expect("lock").push((topic.to_string(), message.clone()));
        Ok(())
    }
}

impl Identity for MockProvider {
    async fn access_token(&self, _identity: String) -> Result<String> {
        Ok("mock_access_token".to_string())
    }
}

impl HttpRequest for MockProvider {
    async fn fetch<T>(&self, request: Request<T>) -> Result<Response<Bytes>>
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn Error + Send + Sync + 'static>>,
    {
        let uri = request.uri().to_string();
        let body = self
            .responses
            .lock()
            .expect("lock")
            .iter()
            .find(|(prefix, _)| uri.starts_with(prefix.as_str()))
            .map(|(_, body)| body.clone());

        let response = match body {
            Some(body) => Response::builder().status(StatusCode::OK).body(Bytes::from(body))?,
            None => Response::builder().status(StatusCode::NOT_FOUND).body(Bytes::new())?,
        };
        Ok(response)
    }
}
