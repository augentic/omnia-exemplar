//! In-memory mock provider for pulse-connector handler tests.
#![allow(missing_docs)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use omnia_guest::{Config, Message, Publish};

/// A mock provider that records published messages and serves configuration
/// from an in-memory map.
#[derive(Default, Clone)]
pub struct MockProvider {
    config: Arc<Mutex<HashMap<String, String>>>,
    published: Arc<Mutex<Vec<(String, Message)>>>,
}

#[allow(clippy::missing_panics_doc)]
impl MockProvider {
    #[must_use]
    pub fn new() -> Self {
        let provider = Self::default();
        provider.config.lock().expect("lock").insert("ENV".to_string(), "dev".to_string());
        provider
    }

    #[must_use]
    pub fn published(&self) -> Vec<(String, Message)> {
        self.published.lock().expect("lock").clone()
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

impl Publish for MockProvider {
    fn send(&self, topic: &str, message: &Message) -> impl Future<Output = Result<()>> {
        let Ok(mut published) = self.published.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on published")));
        };
        published.push((topic.to_string(), message.clone()));
        std::future::ready(Ok(()))
    }
}
