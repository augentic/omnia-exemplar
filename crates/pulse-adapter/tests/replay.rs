//! Tests for expected success and failure outputs from the Pulse adapter for a
//! set of inputs captured as snapshots from the live system.

mod fixture;

use std::fs;

use jiff::Timestamp;

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
        replay(fixture::load(entry.expect("should read entry").path()).await).await;
    }
}

async fn replay(case: Case) {
    let result = case.run().await;
    let events = case.events();

    match &case.expected {
        None => {
            assert!(result.is_ok(), "{result:?}");
            assert!(events.is_empty());
        }
        Some(Expected::Success(expected_events)) => {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(events.len(), expected_events.len());
            expected_events.iter().zip(events).for_each(|(published, mut actual)| {
                let diff = Timestamp::now().as_second() - actual.message_data.timestamp.as_second();
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
                fixture::delivery_error_matches(expected_error, &error),
                "expected {expected_error:?}, got {error:?}"
            );
        }
    }
}
