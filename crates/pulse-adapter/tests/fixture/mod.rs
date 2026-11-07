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

use std::path::Path;

use bytes::Bytes;
use chrono::{Duration, Timelike, Utc};
use http::{Method, Response};
use omnia_sdk::api::messaging::DeliveryError;
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp, Provider};
use pulse_adapter::handler::PulseMessage;
use pulse_adapter::motion::MotionEvent;
use pulse_adapter::pulse::TIMEZONE;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::Deserialize;
use serde_json::Value;

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
    Failure(omnia_sdk::Error),
}

/// A fixture ready to drive: the re-anchored input XML and a seeded provider.
pub struct Case {
    /// The XML payload to deliver on the Pulse topic.
    pub input: Vec<u8>,
    /// Doubles seeded with the fixture's HTTP answers.
    pub provider: Provider,
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
#[expect(
    clippy::disallowed_types,
    reason = "integration test helper reads captured JSON fixtures from disk"
)]
pub fn load(path: impl AsRef<Path>) -> Case {
    let file = std::fs::File::open(path.as_ref()).expect("should open fixture");
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
    let input = match def.params {
        Some(params) => shift_time(&xml, params.delay),
        None => xml.into_bytes(),
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

    let provider = Provider::default()
        .config(MapConfig::default().with([
            ("STATIC_API_URL", BASE_URL),
            ("BLOCK_MGT_URL", BASE_URL),
            ("API_IDENTITY", "block-mgt"),
        ]))
        .http(http)
        .identity(FixedIdentity::new("test-token"));

    Case {
        input,
        provider,
        expected: def.output,
    }
}

/// Whether `error` matches `expected`, with a few seconds' slack when the
/// description embeds a freshness delay computed from `Utc::now()`.
pub fn delivery_error_matches(expected: &omnia_sdk::Error, error: &DeliveryError) -> bool {
    let DeliveryError::Rejected(msg) = error else {
        return false;
    };
    if !msg.contains(&expected.code()) {
        return false;
    }
    let desc = expected.description();
    extract_delay_seconds(&desc).map_or_else(
        || msg.contains(&desc),
        |expected_secs| {
            extract_delay_seconds(msg).is_some_and(|actual| (actual - expected_secs).abs() <= 3)
        },
    )
}

fn extract_delay_seconds(text: &str) -> Option<i64> {
    for prefix in ["outdated by ", "too early by "] {
        let Some(rest) = text.split_once(prefix).map(|(_, rest)| rest) else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(secs) = digits.parse() {
            return Some(secs);
        }
    }
    None
}

/// Re-anchor the first change's actual time in `xml` to `delay` seconds before now.
///
/// The date and seconds-from-midnight are both derived from the shifted
/// instant, so a `delay` that reaches back across midnight lands on the
/// previous date with a positive time-of-day rather than a negative one the
/// adapter would reject as `no_update`.
fn shift_time(xml: &str, delay: i32) -> Vec<u8> {
    let message: PulseMessage = quick_xml::de::from_str(xml).expect("input is a Pulse message");
    let Some(change) = message.train_update.changes.first() else {
        return xml.as_bytes().to_vec();
    };
    let time_tag: Option<&str> = if change.has_departed {
        Some("horaSalidaReal")
    } else if change.has_arrived {
        Some("horaEntradaReal")
    } else {
        None
    };

    let event = Utc::now().with_timezone(&TIMEZONE) - Duration::seconds(i64::from(delay));
    let created_date = event.format("%d/%m/%Y").to_string();
    #[expect(clippy::cast_possible_wrap, reason = "seconds since midnight fit an i32")]
    let adjusted_secs = (event.num_seconds_from_midnight() as i32).to_string();

    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Vec::with_capacity(xml.len()));
    let mut current_tag: Option<String> = None;
    let mut paso_index = 0usize;

    loop {
        match reader.read_event().expect("valid XML") {
            Event::Eof => break,
            Event::Start(elem) => {
                let name = elem.name().into_inner().to_owned();
                if name == "pasoTren" {
                    paso_index += 1;
                }
                current_tag = Some(name);
                writer.write_event(Event::Start(elem)).expect("write");
            }
            Event::End(elem) => {
                current_tag = None;
                writer.write_event(Event::End(elem)).expect("write");
            }
            Event::Text(text) => {
                let replacement = if current_tag.as_deref() == Some("fechaCreacion") {
                    Some(created_date.as_str())
                } else if paso_index == 1 && current_tag.as_deref() == time_tag {
                    Some(adjusted_secs.as_str())
                } else {
                    None
                };
                match replacement {
                    Some(value) => {
                        writer.write_event(Event::Text(BytesText::new(value))).expect("write");
                    }
                    None => writer.write_event(Event::Text(text)).expect("write"),
                }
            }
            other => writer.write_event(other).expect("write"),
        }
    }

    writer.into_inner()
}
