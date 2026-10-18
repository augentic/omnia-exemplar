//! The deployment environment and the helpers that qualify a topic for it.
//!
//! Every other configuration key is a stem's own, declared beside the
//! handler that reads it through [`Config::get`].

use omnia_sdk::Config;

/// Deployment environment (`dev`, `prod`). Prefixes every messaging topic;
/// see [`topic`].
pub const ENV: &str = "ENV";

/// Resolve the deployment environment.
///
/// Falls back to `dev`, with a warning, when [`ENV`] is not configured, so
/// the guest runs out of the box; a deployment sets `ENV` explicitly, and the
/// warning makes a missing key visible instead of silently mis-prefixing
/// every topic.
pub async fn env(provider: &impl Config) -> String {
    match Config::get(provider, ENV).await {
        Ok(env) => env,
        Err(error) => {
            tracing::warn!("`ENV` is not configured, defaulting to `dev`: {error}");
            "dev".to_string()
        }
    }
}

/// Build a fully-qualified messaging topic from an environment-agnostic
/// suffix (see [`crate::topic`]): `realtime-orders.v1` becomes
/// `dev-realtime-orders.v1`.
pub async fn topic(provider: &impl Config, suffix: &str) -> String {
    topic_for(&env(provider).await, suffix)
}

/// Build a fully-qualified messaging topic from an already-resolved
/// environment. Prefer this over [`topic`] when qualifying several topics so
/// the environment is only resolved once.
#[must_use]
pub fn topic_for(env: &str, suffix: &str) -> String {
    format!("{env}-{suffix}")
}
