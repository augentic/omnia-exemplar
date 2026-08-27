//! Tests for expected success and failure outputs from the Pulse adapter for a
//! set of inputs captured as snapshots from the live system.

mod provider;

use std::fs::{self, File};

use acme_common::TIMEZONE;
use acme_test::{TestCase, TestDef};
use chrono::Utc;
use omnia_guest::Error;
use omnia_guest::api::{Client, Metadata};

use crate::provider::{Replay, shift_time};

// Load each test case. For each, present the input to the adapter and compare
// the output expected.
#[tokio::test]
async fn run() {
    for entry in fs::read_dir("data/replay").expect("should read directory") {
        let file = File::open(entry.expect("should read entry").path()).expect("should open file");
        let test_def: TestDef<Error> =
            serde_json::from_reader(&file).expect("should deserialize session");
        replay(test_def).await;
    }
}

async fn replay(test_def: TestDef<Error>) {
    let test_case = TestCase::<Replay>::new(test_def).prepare(shift_time);
    let provider = provider::MockProvider::new(test_case.clone());
    let client = Client::new("acme", provider.clone());

    let input = test_case.input.expect("replay test input expected");
    let result = client.call(input, &Metadata::default()).await;
    let curr_events = provider.events();

    let Some(expected_result) = &test_case.output else {
        assert!(curr_events.is_empty());
        return;
    };

    match expected_result {
        Ok(expected_events) => {
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
        Err(expected_error) => {
            // Was the error the one defined in the fixture?
            let actual_error = result.expect_err("should have error");
            assert_eq!(actual_error.code(), expected_error.code());
            assert_eq!(actual_error.description(), expected_error.description());
        }
    }
}
