//! Component rung: the shipped `guest.wasm` through the example host's own
//! wiring.
//!
//! `build.rs` compiles this package to a `wasm32-wasip2` component and
//! `gen.rs` names it (`COMPONENT_GUEST`). `examples/runtime.rs` is included
//! as `production`, so the `Hooks` its `runtime!` generates — the exact host
//! rows the binary links — assemble the runtime over `omnia_test::host::
//! Backends`, the in-memory defaults. A host the guest imports but the
//! example does not declare fails here at link time, which is the drift the
//! native rungs cannot see.
//!
//! Each scenario drives one trigger export in-process — [`HttpHandler`] and
//! [`MessagingHandler`] bypass the sockets and the broker loop — and asserts
//! the side effect on the backends: a publish reaching the broker, a state
//! write reaching the bucket. Payload semantics stay with the native rungs
//! (`tests/routes.rs`, `tests/messaging.rs`) and the crates' own tests.

use std::time::Duration;

use bytes::Bytes;
use common::topic;
use http::header::{CONTENT_TYPE, HOST};
use http::{Request, StatusCode};
use http_body_util::Full;
use omnia::Runtime;
use omnia::futures::StreamExt as _;
use omnia_test::host::{Backends, Deployment};
use omnia_wasi_http::HttpHandler;
use omnia_wasi_messaging::{Message, MessagingHandler};
use serde_json::Value;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

// The production `runtime!` as the example binary compiles it, untouched:
// `Hooks` is the wiring under test, `manifest()` the (empty) deployment it
// compiles in.
#[path = "../examples/runtime.rs"]
mod production;

/// A closed loopback port: every upstream call is refused immediately.
const CLOSED_UPSTREAM: &str = "http://127.0.0.1:9";

/// The `wasi:config` keys the guest reads, one `Settings` per part, so a
/// setting added to a part cannot be left out here. Host-side settings the
/// example host reads from its environment (identity credentials, the
/// websocket bind address) have no counterpart: `Backends::defaults()`
/// connects nothing.
const GUEST_KEYS: &[&[&str]] = &[
    gtfs_adapter::block_mgt::BlockMgtSettings::KEYS,
    gtfs_adapter::fleet::FleetSettings::KEYS,
    gtfs_adapter::trip::TripSettings::KEYS,
    pulse_adapter::handler::PulseSettings::KEYS,
    pulse_adapter::stops::StopsSettings::KEYS,
    pattern::decode::DecodeSettings::KEYS,
    pattern::identity::IdentitySettings::KEYS,
];

/// A dummy value for a guest setting: upstreams point at a closed port so
/// handlers that call out fail fast, everything else is a marker string.
fn guest_value(key: &str) -> &'static str {
    if key.ends_with("_URL") { CLOSED_UPSTREAM } else { "component-test" }
}

/// Assemble the shipped component over fresh in-memory backends through the
/// example host's `Hooks`, returning the bundle for reading state back.
async fn boot() -> (Backends, Runtime<Backends>) {
    let config = GUEST_KEYS.iter().flat_map(|keys| keys.iter().map(|key| (*key, guest_value(key))));
    let backends = Backends::defaults().await.config(config);
    let runtime = Deployment::from(production::manifest())
        .guest("guest", COMPONENT_GUEST)
        .boot(backends.clone(), <production::Hooks as omnia::Wiring<Backends>>::link)
        .await
        .expect("the shipped component links through the example host's wiring");
    (backends, runtime)
}

/// The bound on every wait for the broker: long enough for a cold
/// instantiation, short enough that a dropped publish fails fast.
const BROKER_WAIT: Duration = Duration::from_secs(5);

const TALLY_MESSAGE: &[u8] = include_bytes!("../crates/tally-connector/data/tally-message.json");
const PASSENGER_COUNT: &[u8] =
    include_bytes!("../crates/gtfs-adapter/data/realtime-passenger-count.v1.json");

/// The `value` of the fixture record at `index`, as the guest receives it.
fn fixture_value(raw: &[u8], index: usize) -> Vec<u8> {
    let records: Value = serde_json::from_slice(raw).expect("fixture parses");
    records[index]["value"].to_string().into_bytes()
}

/// The `wasi:http` export end to end: a request through the trigger's
/// handler reaches the guest's router, and the guest's publish reaches the
/// broker through the example's `wasi:messaging` host.
#[tokio::test]
async fn http_export() {
    let (backends, runtime) = boot().await;
    // The in-memory broker fans out only to live subscribers: subscribe
    // before the guest publishes, or the message is dropped.
    let mut broker = backends.messaging.subscribe();
    let handler = HttpHandler::new(&runtime)
        .expect("http routes consistent")
        .expect("the guest exports the http handler");

    // `Host` is required: the handler answers a request without an authority
    // with `400` before the guest sees it.
    let request = Request::post("/api/apc")
        .header(HOST, "guest.test")
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from_static(TALLY_MESSAGE)))
        .expect("request");
    let response = handler.handle(request).await.expect("handled");
    assert_eq!(response.status(), StatusCode::OK);

    // Exactly one publish, on the tally topic.
    let sent = tokio::time::timeout(BROKER_WAIT, broker.next())
        .await
        .expect("the publish reaches the broker")
        .expect("the broker stays open");
    assert_eq!(sent.topic, topic::TALLY_APC);
    assert!(
        tokio::time::timeout(Duration::from_millis(200), broker.next()).await.is_err(),
        "nothing else reached the broker"
    );

    runtime.shutdown();
}

/// The `wasi:messaging` export end to end: a delivery through the trigger's
/// handler reaches the guest's topic router, and the guest's state write
/// reaches the bucket through the example's `wasi:keyvalue` host.
#[tokio::test]
async fn messaging_export() {
    let (backends, runtime) = boot().await;
    let handler = MessagingHandler::new(&runtime)
        .expect("messaging routes consistent")
        .expect("the guest exports the messaging handler");

    let mut message = Message::new(fixture_value(PASSENGER_COUNT, 0));
    message.topic = topic::PASSENGER_COUNT.to_owned();
    handler.handle(message).await.expect("handled");

    // The guest's `StateStore` writes the occupancy under its composite key
    // into the bucket `Backends::state` reads.
    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";
    let entry = backends.state(key).await.expect("the occupancy status is stored");
    assert_eq!(entry, b"\"FEW_SEATS_AVAILABLE\"");

    runtime.shutdown();
}

// The pulse adapter stamps a service day and the GTFS adapter reads it back;
// the two zones must agree or a trip crosses midnight between them.
#[test]
fn pipeline_timezone() {
    assert_eq!(gtfs_adapter::trip::TIMEZONE, pulse_adapter::pulse::TIMEZONE);
}
