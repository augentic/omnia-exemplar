//! Tests for expected success and failure outputs from the Pulse adapter for a
//! set of inputs captured as snapshots from the live system.

mod fixture;

use std::fs;

use acme_common::TIMEZONE;
use chrono::Utc;
use omnia_guest::api::{Client, Metadata};

use self::fixture::{Case, Expected};

// Load each test case. For each, present the input to the adapter and compare
// the output expected.
#[tokio::test]
async fn run() {
    for entry in fs::read_dir("data/replay").expect("should read directory") {
        replay(fixture::load(entry.expect("should read entry").path())).await;
    }
}

async fn replay(case: Case) {
    let result = Client::new("acme", case.provider.clone())
        .call(case.input.clone(), &Metadata::default())
        .await;
    let curr_events = case.events();

    match &case.expected {
        None => assert!(curr_events.is_empty()),
        Some(Expected::Success(expected_events)) if expected_events.is_empty() => {
            assert!(curr_events.is_empty());
        }
        Some(Expected::Success(expected_events)) => {
            assert_eq!(curr_events.len(), expected_events.len());
            expected_events.iter().zip(curr_events).for_each(|(published, mut actual)| {
                // add 5 seconds to the actual message timestamp the adapter sleeps 5 seconds
                // before output the first round
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
            // Was the error the one defined in the fixture?
            let actual_error = result.expect_err("should have error");
            assert_eq!(actual_error.code(), expected_error.code());
            assert_eq!(actual_error.description(), expected_error.description());
        }
    }
}
