//! Captured Pulse cases from `data/`: the inbound XML, the outbound HTTP the
//! adapter made, and the events or error it produced — loaded into doubles.
//!
//! Timestamps in a captured message are relative to the capture; `delay`
//! re-anchors them to now so the adapter's freshness window still applies.
//!
//! The request matcher that once lived here is `omnia_test::guest::MatchedHttp`
//! now; `Fetch` is only its serialised form. `TestDef` and `Case` stay local
//! because the capture format is this service's, not omnia's, and
//! `augentic/test` is retired for this repository.

#![allow(dead_code, reason = "each test binary uses a subset of the loader")]

use std::fs::File;
use std::path::Path;

use acme_common::{TIMEZONE, config};
use bytes::Bytes;
use chrono::{Timelike, Utc};
use http::{Method, Response};
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp};
use pulse_adapter::{MotionEvent, PulseMessage};
use serde::Deserialize;
use serde_json::Value;

omnia_test::provider! {
    /// The adapter's capability list, as doubles.
    pub struct TestProvider: Config + HttpRequest + Identity + Publish;
}

/// Every outbound URL in a fixture is relative to this base; both APIs the
/// adapter calls are seeded to it.
const BASE_URL: &str = "http://api.test";

/// A captured case as serialised under `data/`.
#[derive(Clone, Debug, Deserialize)]
pub struct TestDef {
    /// The Pulse XML received.
    pub input: Option<String>,
    params: Option<Params>,
    #[serde(default)]
    http_requests: Vec<Fetch>,
    /// What the adapter published, or how it refused.
    pub output: Option<Expected>,
}

/// Present when the capture's timestamps must be re-anchored to now.
#[derive(Clone, Debug, Deserialize)]
struct Params {
    delay: i32,
}

/// One outbound request the adapter made and the answer it got.
#[derive(Clone, Debug, Deserialize)]
struct Fetch {
    #[serde(default = "get")]
    method: String,
    path: String,
    /// The query string, so the fixture names the exact request it answers.
    request: Option<String>,
    #[serde(default)]
    response: FetchResponse,
}

fn get() -> String {
    Method::GET.to_string()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
struct FetchResponse {
    status: u16,
    body: Value,
}

impl Default for FetchResponse {
    fn default() -> Self {
        Self {
            status: 200,
            body: Value::String(String::new()),
        }
    }
}

/// The captured outcome.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expected {
    /// The Motion events published.
    Success(Vec<MotionEvent>),
    /// The handler error returned.
    Failure(omnia_guest::Error),
}

/// A fixture ready to drive: the re-anchored input and a seeded provider.
pub struct Case {
    /// The message to hand the handler.
    pub input: PulseMessage,
    /// Doubles seeded with the fixture's HTTP answers.
    pub provider: TestProvider,
    /// The captured outcome, if the fixture records one.
    pub expected: Option<Expected>,
}

impl Case {
    /// Every Motion event the handler published, in publish order.
    ///
    /// # Panics
    ///
    /// Panics when a published payload is not a Motion event.
    #[must_use]
    pub fn events(&self) -> Vec<MotionEvent> {
        self.provider
            .publish
            .sent()
            .iter()
            .map(|(_, message)| {
                serde_json::from_slice(&message.payload).expect("published payload is an event")
            })
            .collect()
    }
}

/// Read and prepare the fixture at `path`.
///
/// # Panics
///
/// Panics when the file is missing, malformed, or has no input.
pub fn load(path: impl AsRef<Path>) -> Case {
    let file = File::open(path.as_ref()).expect("should open fixture");
    let def: TestDef = serde_json::from_reader(file).expect("should deserialize fixture");
    prepare(def)
}

/// Prepare an already-deserialised fixture.
///
/// # Panics
///
/// Panics when the fixture has no input or the input is not a Pulse message.
#[must_use]
pub fn prepare(def: TestDef) -> Case {
    let xml = def.input.expect("fixture has an input");
    let message = quick_xml::de::from_str(&xml).expect("input is a Pulse message");
    let input = match def.params {
        Some(params) => shift_time(message, params.delay),
        None => message,
    };

    let http = def.http_requests.into_iter().fold(MatchedHttp::default(), |http, fetch| {
        let method = Method::from_bytes(fetch.method.as_bytes()).expect("fixture method");
        let query = fetch.request.map(|query| format!("?{query}")).unwrap_or_default();
        let response = Response::builder()
            .status(fetch.response.status)
            .body(Bytes::from(fetch.response.body.to_string()))
            .expect("fixture response");
        http.on(method, format!("{BASE_URL}{}{query}", fetch.path), response)
    });

    let provider = TestProvider::default()
        .config(MapConfig::default().with([
            (config::ENV, "dev"),
            (config::STATIC_API_URL, BASE_URL),
            (config::BLOCK_MGT_URL, BASE_URL),
            (config::API_IDENTITY, "block-mgt"),
        ]))
        .http(http)
        .identity(FixedIdentity::new("test-token"));

    Case {
        input,
        provider,
        expected: def.output,
    }
}

/// Re-anchor the first change's actual time to `delay` seconds before now.
fn shift_time(mut message: PulseMessage, delay: i32) -> PulseMessage {
    let Some(change) = message.train_update.changes.get_mut(0) else {
        return message;
    };

    let now = Utc::now().with_timezone(&TIMEZONE);
    message.train_update.created_date = now.date_naive();

    #[allow(clippy::cast_possible_wrap, reason = "seconds since midnight fit an i32")]
    let from_midnight = now.num_seconds_from_midnight() as i32;
    let adjusted_secs = from_midnight - delay;

    if change.has_departed {
        change.actual_departure_time = adjusted_secs;
    } else if change.has_arrived {
        change.actual_arrival_time = adjusted_secs;
    }
    message
}
