#![allow(missing_docs)]

use core::panic;
use std::any::Any;
use std::error::Error;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use augentic_test::{Fetcher, Fixture, PreparedTestCase, TestDef, TestResult};
use bytes::Bytes;
use chrono::{Timelike, Utc};
use common::TIMEZONE;
use http::{Request, Response};
use omnia_guest::{Config, HttpRequest, Identity, Message, Publish};
use pulse_adapter::{MotionEvent, PulseMessage};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Replay {
    pub input: Option<PulseMessage>,
    pub params: Option<ReplayTransform>,
    pub output: Option<ReplayOutput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ReplayOutput {
    Events(Vec<MotionEvent>),
    Error(omnia_guest::Error),
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct ReplayTransform {
    pub delay: i32,
}

impl Fixture for Replay {
    type Error = omnia_guest::Error;
    type Input = PulseMessage;
    type Output = Vec<MotionEvent>;
    type TransformParams = ReplayTransform;

    fn from_data(data_def: &TestDef<Self::Error>) -> Self {
        let input_str: Option<String> = data_def.input.as_ref().and_then(|v| {
            serde_json::from_value(v.clone()).expect("should deserialize input as XML String")
        });
        let input = input_str.map(|s| {
            let msg: PulseMessage =
                quick_xml::de::from_str(&s).expect("should deserialize PulseMessage");
            msg
        });
        let params: Option<Self::TransformParams> = data_def.params.as_ref().and_then(|v| {
            serde_json::from_value(v.clone()).expect("should deserialize transform parameters")
        });
        let Some(output_def) = &data_def.output else {
            return Self {
                input,
                params,
                output: None,
            };
        };
        let output = match output_def {
            TestResult::Success(value) => serde_json::from_value(value.clone()).map_or_else(
                |_| panic!("should deserialize output as Motion events"),
                |events| Some(ReplayOutput::Events(events)),
            ),
            TestResult::Failure(err) => Some(ReplayOutput::Error(err.clone())),
        };
        Self {
            input,
            params,
            output,
        }
    }

    fn input(&self) -> Option<Self::Input> {
        self.input.clone()
    }

    fn params(&self) -> Option<Self::TransformParams> {
        self.params.clone()
    }

    fn output(&self) -> Option<Result<Self::Output, Self::Error>> {
        let output = self.output.as_ref()?;
        match output {
            ReplayOutput::Error(error) => Some(Err(error.clone())),
            ReplayOutput::Events(events) => {
                if events.is_empty() {
                    return None;
                }
                Some(Ok(events.clone()))
            }
        }
    }
}

#[derive(Clone)]
pub struct MockProvider {
    test_case: PreparedTestCase<Replay>,
    events: Arc<Mutex<Vec<MotionEvent>>>,
}

impl MockProvider {
    #[allow(clippy::missing_panics_doc)]
    #[allow(dead_code)]
    #[must_use]
    pub fn events(&self) -> Vec<MotionEvent> {
        self.events.lock().expect("should lock").clone()
    }

    #[allow(dead_code)]
    #[must_use]
    pub fn new(test_case: PreparedTestCase<Replay>) -> Self {
        Self {
            test_case,
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Config for MockProvider {
    async fn get(&self, _key: &str) -> Result<String> {
        // BLOCK_MGT_URL, STATIC_API_URL
        Ok("http://localhost:8080".to_string())
    }
}

impl HttpRequest for MockProvider {
    async fn fetch<T>(&self, request: Request<T>) -> Result<Response<Bytes>>
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn Error + Send + Sync + 'static>>,
    {
        let Some(http_requests) = &self.test_case.http_requests else {
            return Err(anyhow!("no http requests defined in replay session"));
        };
        let fetcher = Fetcher::new(http_requests);
        fetcher.fetch(&request)
    }
}

impl Publish for MockProvider {
    async fn send(&self, _topic: &str, message: &Message) -> Result<()> {
        let event: MotionEvent =
            serde_json::from_slice(&message.payload).context("deserializing event")?;
        self.events.lock().map_err(|e| anyhow!("{e}"))?.push(event);
        Ok(())
    }
}

impl Identity for MockProvider {
    async fn access_token(&self, _identity: String) -> Result<String> {
        Ok("mock_access_token".to_string())
    }
}

/// Input transformation function that shifts the timestamps in the
/// `PulseMessage` by the given delay in seconds.
#[must_use]
pub fn shift_time(input: &PulseMessage, params: Option<&ReplayTransform>) -> PulseMessage {
    if params.is_none() {
        return input.clone();
    }
    let delay = params.as_ref().map_or(0, |p| p.delay);
    let mut request = input.clone();
    let Some(change) = request.train_update.changes.get_mut(0) else {
        return request;
    };

    let now = Utc::now().with_timezone(&TIMEZONE);
    request.train_update.created_date = now.date_naive();

    #[allow(clippy::cast_possible_wrap)]
    let from_midnight = now.num_seconds_from_midnight() as i32;
    let adjusted_secs = from_midnight - delay;

    if change.has_departed {
        change.actual_departure_time = adjusted_secs;
    } else if change.has_arrived {
        change.actual_arrival_time = adjusted_secs;
    }
    request
}
