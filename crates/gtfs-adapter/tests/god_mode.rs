//! Tests for the feature-gated god-mode trip override.
#![cfg(feature = "god-mode")]

mod provider;

use gtfs_adapter::SetTripRequest;
use omnia_guest::api::{Client, Metadata};
use serde_json::Value;

use self::provider::MockProvider;

const OWNER: &str = "acme";

#[tokio::test]
async fn set_trip_rejected_when_disabled() {
    // `GOD_MODE_ENABLED` is not configured, so the handler must refuse
    let provider = MockProvider::new();

    let request = SetTripRequest {
        vehicle_id: "EM580".to_string(),
        trip_id: "TRIP-9".to_string(),
    };
    let error = Client::new(OWNER, provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect_err("should reject when god mode is disabled");

    assert!(error.to_string().contains("God mode not enabled"));
    assert!(provider.state("god_mode:overrides").is_none());
}

#[tokio::test]
async fn set_trip_stores_override_when_enabled() {
    let provider = MockProvider::new();
    provider.set_config("GOD_MODE_ENABLED", "true");

    let request = SetTripRequest {
        vehicle_id: "EM580".to_string(),
        trip_id: "TRIP-9".to_string(),
    };
    let reply = Client::new(OWNER, provider.clone())
        .call(request, &Metadata::default())
        .await
        .expect("should succeed");

    assert_eq!(reply.message, "Ok");
    assert_eq!(reply.process, 0);

    let state = provider.state("god_mode:overrides").expect("override should be stored");
    let state: Value = serde_json::from_slice(&state).expect("should parse state");
    assert_eq!(state["overrides"]["EM580"], "TRIP-9");
}
