//! In-memory mock provider for pulse-connector operation tests.
#![allow(missing_docs, clippy::missing_panics_doc)]

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
    async fn get(&self, key: &str) -> Result<String> {
        self.config
            .lock()
            .expect("lock")
            .get(key)
            .cloned()
            .ok_or_else(|| anyhow!("no config value for `{key}`"))
    }
}

impl Publish for MockProvider {
    async fn send(&self, topic: &str, message: &Message) -> Result<()> {
        self.published.lock().expect("lock").push((topic.to_string(), message.clone()));
        Ok(())
    }
}
