//! Test definition types shared by adapter fixtures.

use serde::Deserialize;
use serde_json::Value;

use crate::fetch::Fetch;

/// Standard test definition.
#[derive(Clone, Debug, Deserialize)]
pub struct TestDef<E: std::error::Error> {
    /// Input data.
    ///
    /// The `Value` is expected to be deserialized into the input
    /// type needed by the test case handler.
    pub input: Option<Value>,

    /// Transform parameters.
    ///
    /// Optional parameters that can be used to transform the input data
    /// before passing it to the test case handler. The type of this field
    /// depends on the specific test case handler so we use generic JSON here.
    pub params: Option<Value>,

    /// Outgoing HTTP requests that need to be mocked.
    pub http_requests: Option<Vec<Fetch>>,

    /// Output data.
    ///
    /// The expected output from the test case handler. This can either be an
    /// error or a successful output, depending on the test case. The type of
    /// this field depends on the specific test case handler so we use generic
    /// JSON here.
    ///
    /// Note: The "output" need not be the return type of the underlying handler
    /// under test. It could be a database query or published message that is
    /// sent out from the handler.
    pub output: Option<TestResult<E>>,
}

/// Overlay for a standard rust `Result` type that has tidier deserialization.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all(deserialize = "snake_case"))]
pub enum TestResult<E: std::error::Error> {
    /// Successful result.
    Success(Value),
    /// Error result.
    Failure(E),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::Method;

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
    enum SampleError {
        BadRequest { code: String, description: String },
    }

    impl std::fmt::Display for SampleError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::BadRequest { code, description } => write!(f, "{code}: {description}"),
            }
        }
    }

    impl std::error::Error for SampleError {}

    #[test]
    fn test_deserialize_testdef_with_output() {
        let json = r#"{
            "input": "<CCO>payload</CCO>",
            "params": { "delay": 9 },
            "http_requests": [
                {
                    "path": "/gtfs/stops",
                    "response": { "body": [{"stop_code":"133"}] }
                },
                {
                    "path": "/allocations/trips",
                    "method": "GET",
                    "response": { "body": ["vehicle 1"] }
                }
            ],
            "output": {
                "success": [{"eventType":"Location"}]
            }
        }"#;

        let test_def: TestDef<SampleError> = serde_json::from_str(json).unwrap();

        let Value::String(input) = test_def.input.expect("input exists") else {
            panic!("Expected input to be a string");
        };
        assert!(input.starts_with("<CCO"));
        assert_eq!(test_def.params.expect("params exist")["delay"], serde_json::json!(9));
        let http_requests = test_def.http_requests.expect("http requests exist");
        assert_eq!(http_requests.len(), 2);
        assert_eq!(http_requests[0].method, Method::Get);
        assert_eq!(http_requests[1].path, "/allocations/trips");
        match test_def.output.expect("output exists") {
            TestResult::Success(events) => {
                let events_array = events.as_array().expect("output is an array");
                assert_eq!(events_array.len(), 1);
            }
            TestResult::Failure(_) => panic!("Expected success output"),
        }
    }

    #[test]
    fn test_deserialize_testdef_with_error() {
        let json = r#"{
            "input": "<CCO>payload</CCO>",
            "params": { "delay": 506 },
            "output": {
                "failure": {
                    "BadRequest": {
                        "code": "bad_time",
                        "description": "outdated by 506 seconds"
                    }
                }
            }
        }"#;

        let test_def: TestDef<SampleError> = serde_json::from_str(json).unwrap();

        let Value::String(input) = test_def.input.expect("input exists") else {
            panic!("Expected input to be a string");
        };
        assert!(input.starts_with("<CCO"));
        assert_eq!(test_def.params.expect("params exist")["delay"], serde_json::json!(506));
        assert!(test_def.http_requests.is_none());
        match test_def.output.expect("output exists") {
            TestResult::Failure(SampleError::BadRequest { code, description }) => {
                assert_eq!(code, "bad_time");
                assert_eq!(description, "outdated by 506 seconds");
            }
            TestResult::Success(_) => panic!("Expected failure output"),
        }
    }
}
