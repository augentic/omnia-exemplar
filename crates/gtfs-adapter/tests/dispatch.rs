//! The adapter's routes and topics driven through its registrations, as the
//! guest serves them: one request per route, one delivery per topic.

mod support;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use http::Method;
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, DeliveryError, Router};
use omnia_test::guest::{MatchedHttp, Provider};
use serde_json::Value;
use tower::ServiceExt as _;

use self::support::{allocation_query, fleet_query, ok, provider};

const ENV: &str = "dev";
const FLEET_QUERY: &[u8] = include_bytes!("../data/fleet-query.json");

fn topics(provider: &Provider) -> Router<Provider> {
    gtfs_adapter::topics(Router::new(Client::new("acme", provider.clone())), ENV)
}

fn delivery(topic: &str, payload: impl Into<Vec<u8>>) -> Delivery {
    Delivery {
        topic: Some(topic.to_string()),
        payload: payload.into(),
        ..Delivery::default()
    }
}

/// The `value` of the fixture record at `index`.
fn fixture_value(raw: &[u8], index: usize) -> Value {
    let records: Value = serde_json::from_slice(raw).expect("should parse fixture");
    records[index]["value"].clone()
}

fn message_timestamp(value: &Value) -> DateTime<Utc> {
    value["messageData"]["timestamp"].as_str().expect("timestamp").parse().expect("RFC 3339")
}

/// Whether the router, rather than a handler, produced this response.
fn router_miss(status: StatusCode, body: &[u8]) -> bool {
    status == StatusCode::METHOD_NOT_ALLOWED || (status == StatusCode::NOT_FOUND && body.is_empty())
}

async fn send(provider: Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let router = gtfs_adapter::router().with_state(Client::new("acme", provider));
    let response = router.oneshot(request).await.expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    (status, body.to_vec())
}

#[tokio::test]
async fn vehicle_info() {
    let provider = provider().http(MatchedHttp::default().on(
        Method::GET,
        fleet_query("EMP484"),
        ok(FLEET_QUERY),
    ));
    let request = Request::get("/info/EMP484").body(Body::empty()).expect("request");

    let (status, body) = send(provider, request).await;

    assert!(!router_miss(status, &body), "{status}");
}

#[cfg(feature = "god-mode")]
#[tokio::test]
async fn set_trip() {
    let request = Request::post("/god-mode/set-trip/1/1")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(&b"{}"[..]))
        .expect("request");

    let (status, body) = send(provider(), request).await;

    assert!(!router_miss(status, &body), "{status}");
}

#[tokio::test]
async fn pulse_to_motion() {
    let value = fixture_value(include_bytes!("../data/realtime-pulse-to-motion.v1.json"), 0);
    let provider = provider().http(
        MatchedHttp::default().on(Method::GET, fleet_query("EMP484"), ok(FLEET_QUERY)).on(
            Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );

    topics(&provider)
        .handle(delivery("dev-realtime-pulse-to-motion.v1", value.to_string()))
        .await
        .expect("should deliver");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-gtfs-vp.v1");
    assert_eq!(record.headers.get("key").map(String::as_str), Some("59144"));
}

#[tokio::test]
async fn train_avl() {
    let mut fleet: Value = serde_json::from_slice(FLEET_QUERY).expect("should parse fixture");
    fleet[0]["tag"] = "motion".into();
    let value = fixture_value(include_bytes!("../data/realtime-train-avl.v1.json"), 0);
    let provider = provider().http(
        MatchedHttp::default().on(Method::GET, fleet_query("EM633"), ok(fleet.to_string())).on(
            Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );

    topics(&provider)
        .handle(delivery("dev-realtime-train-avl.v1", value.to_string()))
        .await
        .expect("should deliver");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].0, "dev-realtime-gtfs-vp.v1");
}

#[tokio::test]
async fn passenger_count() {
    let value = fixture_value(include_bytes!("../data/realtime-passenger-count.v1.json"), 0);
    let provider = provider();

    topics(&provider)
        .handle(delivery("dev-realtime-passenger-count.v1", value.to_string()))
        .await
        .expect("should deliver");

    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";
    assert_eq!(provider.storage.state(key), Some(b"\"FEW_SEATS_AVAILABLE\"".to_vec()));
    assert!(provider.publish.sent().is_empty());
}

#[tokio::test]
async fn other_environment() {
    let provider = provider();
    let topic = acme_common::config::topic_for("prod", acme_common::topic::PASSENGER_COUNT);

    let error =
        topics(&provider).handle(delivery(&topic, b"{}".to_vec())).await.expect_err("wrong env");

    assert_eq!(error, DeliveryError::UnhandledTopic(topic));
    assert!(provider.storage.is_empty());
}
