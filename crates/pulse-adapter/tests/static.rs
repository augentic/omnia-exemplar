//! Static fixture tests for the Pulse position adapter.
#![cfg(not(miri))]

#[expect(dead_code, reason = "these scenarios read only failure outcomes; replay reads successes")]
mod fixture;

use common::topic;
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::DeliveryError;
use omnia_test::guest::deliver;
use pulse_adapter::stops::StopInfo;

use self::fixture::{Case, Expected};

#[tokio::test]
async fn stops_cache() {
    let case = fixture::load("data/static/0001.json");
    let stops_requests =
        || case.provider.http.requests().iter().filter(|r| r.uri.path() == "/gtfs/stops").count();

    // empty cache
    run(&case).await.expect("should process");

    let events = case.events();
    assert_eq!(events.len(), 2);
    // an arrival carries the stop's own location, not its departure location
    assert!(events[0].location_data.latitude.eq(&-36.12345));
    assert!(events[0].location_data.longitude.eq(&174.12345));
    assert!(case.provider.storage.state("pulse:stops").is_some(), "stops table cached");

    // warm cache
    run(&case).await.expect("should process from cache");

    assert_eq!(stops_requests(), 1);

    // corrupt cache
    case.provider.storage.insert_state("pulse:stops", b"not valid json");
    run(&case).await.expect("should refetch and process");

    assert_eq!(stops_requests(), 2);
    let cached = case.provider.storage.state("pulse:stops").expect("stops table rewritten");
    let stops: Vec<StopInfo> = serde_json::from_slice(&cached).expect("valid cached stops");
    assert!(!stops.is_empty());
}

// station 5 is not one of the adapter's active stations
#[tokio::test]
async fn unmapped_station() {
    let case = fixture::load("data/static/0003.json");

    run(&case).await.expect("should process");

    assert!(case.events().is_empty());
}

// Block Management allocates no vehicle to train 445
#[tokio::test]
async fn no_matching_vehicle() {
    let case = fixture::load("data/static/0004.json");

    run(&case).await.expect("should process");

    assert!(case.events().is_empty());
}

// station 0 is active, but the stops table lacks its stop code
#[tokio::test]
async fn no_matching_stop() {
    expect_error("data/static/0005.json").await;
}

// the train update names the train but carries no changes
#[tokio::test]
async fn no_changes() {
    expect_error("data/static/0007.json").await;
}

// the first change has neither arrived nor departed
#[tokio::test]
async fn no_actual_changes() {
    expect_error("data/static/0008.json").await;
}

// the change happened 61 seconds ago, past the 60-second window
#[tokio::test]
async fn too_late() {
    expect_error("data/static/0009.json").await;
}

// the change is 32 seconds in the future, past the 30-second window
#[tokio::test]
async fn too_early() {
    expect_error("data/static/0010.json").await;
}

// Run a fixture that is expected to fail and compare the error with the
// fixture's expected output.
async fn expect_error(path: &str) {
    let case = fixture::load(path);

    let Some(Expected::Failure(expected_error)) = &case.expected else {
        panic!("fixture should expect an error");
    };
    let error = run(&case).await.expect_err("should have error");
    assert!(
        fixture::delivery_error_matches(expected_error, &error),
        "expected {expected_error:?}, got {error:?}"
    );
}

async fn run(case: &Case) -> Result<(), DeliveryError> {
    let router = pulse_adapter::handlers().with_state(Client::new(case.provider.clone()));
    deliver(&router, topic::PULSE, case.input.clone()).await
}
