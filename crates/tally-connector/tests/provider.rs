#![allow(missing_docs)]

use std::sync::{Arc, Mutex};

use anyhow::Result;
use omnia_guest::{Config, Message, Publish};

#[derive(Default, Clone)]
pub struct MockProvider {
    published: Arc<Mutex<Vec<(String, Message)>>>,
}

impl MockProvider {
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn published(&self) -> Vec<(String, Message)> {
        self.published.lock().expect("lock").clone()
    }
}

impl Publish for MockProvider {
    fn send(
        &self, topic: &str, message: &Message,
    ) -> impl Future<Output = anyhow::Result<()>> + Send {
        let topic = topic.to_string();
        let message = message.clone();
        let published = Arc::clone(&self.published);

        async move {
            published.lock().expect("lock").push((topic, message));
            Ok(())
        }
    }
}

impl Config for MockProvider {
    fn get(&self, _key: &str) -> impl Future<Output = Result<String>> {
        std::future::ready(Ok("dev".to_string()))
    }
}
