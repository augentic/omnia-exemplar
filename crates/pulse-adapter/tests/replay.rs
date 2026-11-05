//! Tests for expected success and failure outputs from the Pulse adapter for a
//! set of inputs captured as snapshots from the live system.

mod fixture;

use std::fs;

use chrono::Utc;
use common::topic;
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, DeliveryError};
use pulse_adapter::pulse::TIMEZONE;

use self::fixture::{Case, Expected};

// Load each test case. For each, present the input to the adapter and compare
// the output expected.
#[tokio::test]
#[expect(
    clippy::disallowed_methods,
    reason = "integration test enumerates captured replay fixtures from disk"
)]
async fn run() {
    for entry in fs::read_dir("data/replay").expect("should read directory") {
        replay(fixture::load(entry.expect("should read entry").path())).await;
    }
}

async fn replay(case: Case) {
    let router = pulse_adapter::topics().with_state(Client::new(case.provider.clone()));
    let delivery = Delivery {
        topic: Some(topic::PULSE.to_string()),
        payload: case.input.clone(),
        ..Delivery::default()
    };
    let result = router.handle(delivery).await;
    let curr_events = case.events();

    match &case.expected {
        None => assert!(curr_events.is_empty()),
        Some(Expected::Success(expected_events)) if expected_events.is_empty() => {
            assert!(curr_events.is_empty());
        }
        Some(Expected::Success(expected_events)) => {
            assert_eq!(curr_events.len(), expected_events.len());
            expected_events.iter().zip(curr_events).for_each(|(published, mut actual)| {
                let now = Utc::now().with_timezone(&TIMEZONE);
                let diff = now.timestamp() - actual.message_data.timestamp.timestamp();
                assert!(diff.abs() < 3, "expected vs actual too great: {diff}");

                // compare original published message to pulse event
                actual.received_at = published.received_at;
                actual.message_data.timestamp = published.message_data.timestamp;

                let json_actual = serde_json::to_value(&actual).unwrap();
                let json_expected = serde_json::to_value(published).unwrap();
                assert_eq!(json_expected, json_actual);
            });
        }
        Some(Expected::Failure(expected_error)) => {
            let error = result.expect_err("should have error");
            assert!(
                matches!(&error, DeliveryError::Rejected(msg)
                    if msg.contains(&expected_error.code())
                        && msg.contains(&expected_error.description())),
                "expected {expected_error:?}, got {error:?}"
            );
        }
    }
}
