//! In-memory mock provider for gtfs-adapter operation tests.
#![allow(missing_docs)]
// Shared by several test binaries; not every binary uses every helper.
#![allow(dead_code)]

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use omnia_guest::{CasError, Config, HttpRequest, Identity, Message, Publish, StateStore};

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

#[allow(clippy::missing_panics_doc)]
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
    fn get(&self, key: &str) -> impl Future<Output = Result<String>> {
        let Ok(config) = self.config.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on config")));
        };
        std::future::ready(
            config.get(key).cloned().ok_or_else(|| anyhow!("no config value for `{key}`")),
        )
    }
}

impl StateStore for MockProvider {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        let Ok(state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        std::future::ready(Ok(state.get(key).cloned()))
    }

    fn set(
        &self, key: &str, value: &[u8], _ttl_secs: Option<u64>,
    ) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        std::future::ready(Ok(state.insert(key.to_string(), value.to_vec())))
    }

    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        state.remove(key);
        std::future::ready(Ok(()))
    }

    fn cas(
        &self, key: &str, expected: Option<&[u8]>, value: &[u8],
    ) -> impl Future<Output = Result<(), CasError>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(CasError::Store(
                "failed to obtain lock on state".into(),
            )));
        };
        let observed = state.get(key).cloned();
        if observed.as_deref() != expected {
            return std::future::ready(Err(CasError::Conflict(observed)));
        }
        state.insert(key.to_string(), value.to_vec());
        std::future::ready(Ok(()))
    }

    fn increment(&self, key: &str, delta: i64) -> impl Future<Output = Result<i64>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        let current = match state.get(key) {
            None => 0,
            Some(value) => {
                let Ok(bytes) = <[u8; 8]>::try_from(value.as_slice()) else {
                    return std::future::ready(Err(anyhow!(
                        "value is {} bytes, not an 8-byte big-endian integer",
                        value.len()
                    )));
                };
                i64::from_be_bytes(bytes)
            }
        };
        let Some(incremented) = current.checked_add(delta) else {
            return std::future::ready(Err(anyhow!("adding delta overflows i64")));
        };
        state.insert(key.to_string(), incremented.to_be_bytes().to_vec());
        std::future::ready(Ok(incremented))
    }
}

impl Publish for MockProvider {
    fn send(&self, topic: &str, message: &Message) -> impl Future<Output = Result<()>> {
        let Ok(mut published) = self.published.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on published")));
        };
        published.push((topic.to_string(), message.clone()));
        std::future::ready(Ok(()))
    }
}

impl Identity for MockProvider {
    fn access_token(&self, _identity: String) -> impl Future<Output = Result<String>> {
        std::future::ready(Ok("mock_access_token".to_string()))
    }
}

impl HttpRequest for MockProvider {
    fn fetch<T>(&self, request: Request<T>) -> impl Future<Output = Result<Response<Bytes>>>
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn Error + Send + Sync + 'static>>,
    {
        let uri = request.uri().to_string();
        let Ok(responses) = self.responses.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on responses")));
        };
        let body = responses
            .iter()
            .find(|(prefix, _)| uri.starts_with(prefix.as_str()))
            .map(|(_, body)| body.clone());

        let response = body.map_or_else(
            || Response::builder().status(StatusCode::NOT_FOUND).body(Bytes::new()),
            |body| Response::builder().status(StatusCode::OK).body(Bytes::from(body)),
        );
        match response {
            Ok(response) => std::future::ready(Ok(response)),
            Err(error) => std::future::ready(Err(error.into())),
        }
    }
}
