//! Configuration keys and helpers.
//!
//! Every configuration key the guest reads is declared here, so there is one
//! catalog of what a deployment must provide, and every topic is qualified
//! for the deployment's environment through [`topic`] or [`topic_for`].

use omnia_sdk::Config;

/// Deployment environment (`dev`, `prod`). Prefixes every messaging topic;
/// see [`topic`]. One constant per key after this one, each added at the end
/// of the list, before [`env`].
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
/// suffix (see [`crate::routes::topic`]): `realtime-orders.v1` becomes
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
