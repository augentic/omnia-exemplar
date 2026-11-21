//! Captured Pulse cases from `data/`: the inbound XML, the outbound HTTP the
//! adapter made, and the events or error it produced, loaded into the
//! backends a run boots the stem's component over.
//!
//! Timestamps in a captured message are relative to the capture; `delay`
//! re-anchors them to now so the adapter's freshness window still applies.
//!
//! `Fetch` is the serialised form of a `MatchedHttp` route. `TestDef` and
//! `Case` are local because the capture format is this service's, not
//! omnia's.

use std::path::Path;

use bytes::Bytes;
use chrono::{Duration, Timelike, Utc};
use common::topic;
use http::{Method, Response};
use omnia::{Provides, Runtime};
use omnia_sdk::ErrorKind;
use omnia_test::host::{
    self, Backends, Deployment, FixedIdentity, HandleError, MatchedHttp, Sink, precompiled,
};
use omnia_wasi_config::WasiConfig;
use omnia_wasi_http::WasiHttp;
use omnia_wasi_identity::WasiIdentity;
use omnia_wasi_keyvalue::WasiKeyValue;
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;
use pulse_adapter::handler::PulseMessage;
use pulse_adapter::motion::MotionEvent;
use pulse_adapter::pulse::TIMEZONE;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

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
    Failure(#[serde(deserialize_with = "refusal")] omnia_sdk::Error),
}

// a capture records a refusal as `{"<kind>": {"code": .., "description": ..}}`
fn refusal<'de, D: Deserializer<'de>>(deserializer: D) -> Result<omnia_sdk::Error, D::Error> {
    #[derive(Deserialize)]
    struct Detail {
        code: String,
        description: String,
    }

    #[derive(Deserialize)]
    enum Refusal {
        BadRequest(Detail),
        NotFound(Detail),
        ServerError(Detail),
        BadGateway(Detail),
    }

    let (kind, detail) = match Refusal::deserialize(deserializer)? {
        Refusal::BadRequest(detail) => (ErrorKind::BadRequest, detail),
        Refusal::NotFound(detail) => (ErrorKind::NotFound, detail),
        Refusal::ServerError(detail) => (ErrorKind::ServerError, detail),
        Refusal::BadGateway(detail) => (ErrorKind::BadGateway, detail),
    };
    Ok(omnia_sdk::Error::new(kind, detail.code, detail.description))
}

/// A fixture ready to drive: the re-anchored input XML and the backends a
/// run boots over.
pub struct Case {
    /// The XML payload to deliver on the Pulse topic.
    pub input: Vec<u8>,
    /// The default backends with the adapter's configuration, whose stores
    /// every run shares.
    pub backends: Backends,
    /// The fixture's HTTP answers, recording every request a run sends.
    pub http: MatchedHttp,
    /// Every message a run publishes.
    pub sink: Sink,
    /// The captured outcome, if the fixture records one.
    pub expected: Option<Expected>,
}

impl Case {
    /// Delivers the input on the Pulse topic to a freshly booted component.
    ///
    /// # Errors
    ///
    /// Returns how the component settled a delivery it did not handle.
    pub async fn run(&self) -> Result<(), HandleError> {
        let backends = self
            .backends
            .clone()
            .http(self.http.clone())
            .identity(FixedIdentity::new("test-token"))
            .messaging(self.sink.clone());
        let runtime = boot(backends).await;
        let settled = host::deliver(&runtime, topic::PULSE, self.input.clone()).await;
        runtime.shutdown();
        settled
    }

    /// Every Motion event the handler published, in publish order.
    ///
    /// # Panics
    ///
    /// Panics when a published payload is not a Motion event.
    #[must_use]
    pub fn events(&self) -> Vec<MotionEvent> {
        self.sink
            .sent()
            .iter()
            .map(|message| {
                serde_json::from_slice(&message.payload).expect("published payload is an event")
            })
            .collect()
    }
}

async fn boot<B>(backends: B) -> Runtime<B>
where
    B: Provides<WasiConfig>
        + Provides<WasiHttp>
        + Provides<WasiIdentity>
        + Provides<WasiKeyValue>
        + Provides<WasiMessaging>
        + Provides<WasiOtel>,
{
    let guest = precompiled(PULSE_ADAPTER_GUEST).expect("guest precompiles");
    Deployment::new()
        .guest("pulse-adapter", guest)
        .boot(backends, |deployment| {
            deployment.host::<WasiConfig>()?;
            deployment.host::<WasiHttp>()?;
            deployment.host::<WasiIdentity>()?;
            deployment.host::<WasiKeyValue>()?;
            deployment.host::<WasiMessaging>()?;
            deployment.host::<WasiOtel>()?;
            Ok(())
        })
        .await
        .expect("runtime boots")
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
pub async fn load(path: impl AsRef<Path>) -> Case {
    let file = std::fs::File::open(path.as_ref()).expect("should open fixture");
    let def: TestDef = serde_json::from_reader(file).expect("should deserialize fixture");
    prepare(def).await
}

/// Prepare an already-deserialised fixture.
///
/// # Panics
///
/// Panics when the fixture has no input or the input is not a Pulse message.
pub async fn prepare(def: TestDef) -> Case {
    let xml = def.input.expect("fixture has an input");
    let input = match def.params {
        Some(params) => shift_time(&xml, params.delay),
        None => xml.into_bytes(),
    };

    let http = def.http_requests.into_iter().fold(MatchedHttp::default(), |http, fetch| {
        let method = Method::from_bytes(fetch.method.as_bytes()).expect("fixture method");
        let query = fetch.request.map_or_default(|query| format!("?{query}"));
        let response = Response::builder()
            .status(fetch.response.status)
            .body(Bytes::from(fetch.response.body.to_string()))
            .expect("fixture response");
        http.on(method, format!("{BASE_URL}{}{query}", fetch.path), response)
    });

    let backends = Backends::defaults().await.config([
        ("STATIC_API_URL", BASE_URL),
        ("BLOCK_MGT_URL", BASE_URL),
        ("API_IDENTITY", "block-mgt"),
    ]);

    Case {
        input,
        backends,
        http,
        sink: Sink::default(),
        expected: def.output,
    }
}

/// Whether the component settled a delivery the way `expected` says, with a
/// few seconds' slack when the description embeds a freshness delay
/// computed from `Utc::now()`.
///
/// A bad request or a conflict is rejected and any other failure released,
/// each with the guest error's `code: …, description: …` text as its reason.
pub fn delivery_error_matches(expected: &omnia_sdk::Error, error: &HandleError) -> bool {
    let rejects = matches!(expected.kind(), ErrorKind::BadRequest | ErrorKind::Conflict);
    let reason = match error {
        HandleError::Rejected(reason) if rejects => reason,
        HandleError::Released { reason, .. } if !rejects => reason,
        _ => return false,
    };
    let Some((code, actual)) =
        reason.strip_prefix("code: ").and_then(|rest| rest.split_once(", description: "))
    else {
        return false;
    };
    if code != expected.code() {
        return false;
    }
    let desc = expected.description();
    delay_seconds(desc).map_or_else(
        || actual.contains(desc),
        |expected_secs| delay_seconds(actual).is_some_and(|secs| (secs - expected_secs).abs() <= 3),
    )
}

fn delay_seconds(text: &str) -> Option<i64> {
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
