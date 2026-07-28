//! Configuration keys and helpers.
//!
//! Every configuration key read by the exemplar is declared here so there is
//! a single catalog of what a deployment must provide. The guest
//! `.env.example` files carry sample values for each key.

use omnia_guest::Config;

/// Deployment environment (e.g. `dev`, `prod`). Prefixes every messaging
/// topic — see [`topic`].
pub const ENV: &str = "ENV";

/// Base URL of the Block Management API.
pub const BLOCK_MGT_URL: &str = "BLOCK_MGT_URL";

/// Identity used to acquire access tokens for the operator APIs.
pub const API_IDENTITY: &str = "API_IDENTITY";

/// Base URL of the static GTFS API.
pub const STATIC_API_URL: &str = "STATIC_API_URL";

/// Base URL of the Fleet API.
pub const FLEET_URL: &str = "FLEET_URL";

/// Base URL of the Trip Management API.
pub const TRIP_MANAGEMENT_URL: &str = "TRIP_MANAGEMENT_URL";

/// Enables the god-mode trip override operation. Off unless set to a truthy
/// value (`1`, `true`, `yes`, `on`).
pub const GOD_MODE_ENABLED: &str = "GOD_MODE_ENABLED";

/// Resolve the deployment environment.
///
/// Falls back to `dev` — with a warning — when [`ENV`] is not configured, so
/// the exemplar remains runnable out of the box. Real deployments should
/// always set `ENV` explicitly; the warning makes a missing key visible
/// instead of silently mis-prefixing every topic.
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
/// suffix (see [`crate::routes::topic`]), e.g. `realtime-pulse.v1` becomes
/// `dev-realtime-pulse.v1`.
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
