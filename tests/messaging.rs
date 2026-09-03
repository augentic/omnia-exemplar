//! Messaging rung: the production topic table driven natively.
//!
//! `messaging_router` is a guest-side value, so one delivery per subscribed
//! topic goes straight to `Router::handle` under `omnia_test` doubles, and
//! the downstream publish or store write is asserted on the provider.

use acme_common::fleet::Identifier;
use acme_common::{TIMEZONE, config, routes};
use bytes::Bytes;
use chrono::{DateTime, Timelike, Utc};
use http::{Method, Response};
use omnia_guest::api::messaging::{Delivery, DeliveryError};
use omnia_test::guest::{FixedIdentity, MapConfig, MatchedHttp};
use serde_json::Value;

omnia_test::provider! {
    /// The production capability list, as doubles.
    pub struct TestProvider: Config + DocumentStore + HttpRequest + Identity + Publish + StateStore
        + TableStore;
}

const STATIC_API_URL: &str = "http://static.test";
const BLOCK_MGT_URL: &str = "http://block-mgt.test";
const FLEET_URL: &str = "http://fleet.test";
const FLEET_QUERY: &[u8] = include_bytes!("../crates/gtfs-adapter/data/fleet-query.json");

fn provider(http: MatchedHttp) -> TestProvider {
    TestProvider::default()
        .config(MapConfig::default().with([
            (config::ENV, "dev"),
            (config::STATIC_API_URL, STATIC_API_URL),
            (config::BLOCK_MGT_URL, BLOCK_MGT_URL),
            (config::FLEET_URL, FLEET_URL),
            (config::TRIP_MANAGEMENT_URL, "http://trip-mgt.test"),
            (config::API_IDENTITY, "test-identity"),
        ]))
        .http(http)
        .identity(FixedIdentity::new("test-token"))
}

fn delivery(topic: &str, payload: impl Into<Vec<u8>>) -> Delivery {
    Delivery {
        topic: Some(topic.to_string()),
        payload: payload.into(),
        ..Delivery::default()
    }
}

fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
}

/// The `value` of the fixture record at `index`.
fn fixture_value(raw: &[u8], index: usize) -> Value {
    let records: Value = serde_json::from_slice(raw).expect("should parse fixture");
    records[index]["value"].clone()
}

fn message_timestamp(value: &Value) -> DateTime<Utc> {
    value["messageData"]["timestamp"].as_str().expect("timestamp").parse().expect("RFC 3339")
}

fn fleet_query(vehicle_id: &str) -> String {
    let identifier: Identifier = vehicle_id.parse().expect("identifier parse is infallible");
    format!("{FLEET_URL}/vehicles?{}", identifier.to_query())
}

fn allocation_query(fleet_id: &str, at: DateTime<Utc>) -> String {
    format!(
        "{BLOCK_MGT_URL}/allocations/vehicles/{fleet_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={}",
        at.timestamp()
    )
}

/// A Pulse arrival at station 0 (stop 133) timestamped now, so it passes
/// the adapter's freshness window.
fn pulse_arrival_xml(train: &str) -> String {
    let now = Utc::now().with_timezone(&TIMEZONE);
    let created_date = now.format("%d/%m/%Y");
    let event_secs = now.num_seconds_from_midnight();
    format!(
        r#"<CCO xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="CCO">
            <ActualizarDatosTren>
                <trenPar>{train}</trenPar>
                <trenImpar>{train}</trenImpar>
                <fechaCreacion>{created_date}</fechaCreacion>
                <numeroRegistro>9299669</numeroRegistro>
                <operadorComercial>METRO</operadorComercial>
                <pasoTren>
                    <tipoCambio>3</tipoCambio>
                    <estacion>0</estacion>
                    <idPaso>181353261</idPaso>
                    <horaEntrada>{event_secs}</horaEntrada>
                    <horaEntradaReal>{event_secs}</horaEntradaReal>
                    <haEntrado>true</haEntrado>
                    <tipoParada>4</tipoParada>
                    <paridad>p</paridad>
                    <sentido>0</sentido>
                    <horaSalida>{event_secs}</horaSalida>
                    <horaSalidaReal>{event_secs}</horaSalidaReal>
                    <haSalido>false</haSalido>
                    <viaEntradaMallas>2</viaEntradaMallas>
                    <retrasoEntrada>-3</retrasoEntrada>
                    <viaCirculacionMallas>2</viaCirculacionMallas>
                    <retrasoSalida>0</retrasoSalida>
                    <horaInicioDetencion>-1</horaInicioDetencion>
                    <duracionDetencion>-1</duracionDetencion>
                </pasoTren>
                <codigoOperadorComercial>-1</codigoOperadorComercial>
                <origenActualizaTren>GAC</origenActualizaTren>
            </ActualizarDatosTren>
        </CCO>"#
    )
}

#[tokio::test]
async fn pulse_xml_publishes_motion_events() {
    let stops = r#"[{"stop_code":"133","stop_lat":-36.12345,"stop_lon":174.12345}]"#;
    let provider = provider(
        MatchedHttp::default()
            .on(
                Method::GET,
                format!("{STATIC_API_URL}/gtfs/stops?fields=stop_code,stop_lon,stop_lat"),
                ok(stops),
            )
            .on(
                Method::GET,
                format!("{BLOCK_MGT_URL}/allocations/trips?externalRefId=5226"),
                ok(r#"["vehicle 1"]"#),
            ),
    );
    let router = guest::messaging_router(provider.clone()).await;

    router
        .handle(delivery("dev-realtime-pulse.v1", pulse_arrival_xml("5226")))
        .await
        .expect("should deliver");

    // One allocated train, published twice (see `pulse_adapter`'s
    // `PUBLISH_REPEATS`), keyed by the vehicle.
    let published = provider.publish.sent();
    assert_eq!(published.len(), 2);
    for (topic, record) in &published {
        assert_eq!(topic, "dev-realtime-pulse-to-motion.v1");
        assert_eq!(record.headers.get("key").map(String::as_str), Some("vehicle1"));
        let event: Value = serde_json::from_slice(&record.payload).expect("event JSON");
        assert_eq!(event["locationData"]["latitude"], -36.12345);
    }
}

#[tokio::test]
async fn pulse_to_motion_publishes_vehicle_position() {
    let value = fixture_value(
        include_bytes!("../crates/gtfs-adapter/data/realtime-pulse-to-motion.v1.json"),
        0,
    );
    let provider = provider(
        MatchedHttp::default().on(Method::GET, fleet_query("EMP484"), ok(FLEET_QUERY)).on(
            Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );
    let router = guest::messaging_router(provider.clone()).await;

    router
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
async fn train_avl_publishes_vehicle_position() {
    let mut fleet: Value = serde_json::from_slice(FLEET_QUERY).expect("should parse fixture");
    fleet[0]["tag"] = "motion".into();
    let value =
        fixture_value(include_bytes!("../crates/gtfs-adapter/data/realtime-train-avl.v1.json"), 0);
    let provider = provider(
        MatchedHttp::default().on(Method::GET, fleet_query("EM633"), ok(fleet.to_string())).on(
            Method::GET,
            allocation_query("59144", message_timestamp(&value)),
            ok("null"),
        ),
    );
    let router = guest::messaging_router(provider.clone()).await;

    router
        .handle(delivery("dev-realtime-train-avl.v1", value.to_string()))
        .await
        .expect("should deliver");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].0, "dev-realtime-gtfs-vp.v1");
}

#[tokio::test]
async fn passenger_count_stores_occupancy_status() {
    let value = fixture_value(
        include_bytes!("../crates/gtfs-adapter/data/realtime-passenger-count.v1.json"),
        0,
    );
    let provider = provider(MatchedHttp::default());
    let router = guest::messaging_router(provider.clone()).await;

    router
        .handle(delivery("dev-realtime-passenger-count.v1", value.to_string()))
        .await
        .expect("should deliver");

    let key = "motionGtfs:occupancyStatus:32161:1347-05004-41400-2-89c4020e:20251120:11:30:00";
    assert_eq!(provider.storage.state(key), Some(b"\"FEW_SEATS_AVAILABLE\"".to_vec()));
    assert!(provider.publish.sent().is_empty());
}

#[tokio::test]
async fn other_environment_topic_is_unhandled() {
    let provider = provider(MatchedHttp::default());
    let router = guest::messaging_router(provider.clone()).await;

    let topic = config::topic_for("prod", routes::topic::PASSENGER_COUNT);
    let error = router.handle(delivery(&topic, b"{}".to_vec())).await.expect_err("wrong env");

    assert_eq!(error, DeliveryError::UnhandledTopic(topic));
    assert!(provider.storage.is_empty());
}

#[tokio::test]
async fn undecodable_payload_is_rejected() {
    let provider = provider(MatchedHttp::default());
    let router = guest::messaging_router(provider.clone()).await;

    let error = router
        .handle(delivery("dev-realtime-pulse.v1", b"not xml".to_vec()))
        .await
        .expect_err("malformed XML");

    assert!(matches!(error, DeliveryError::Rejected(_)), "{error}");
    assert!(provider.publish.sent().is_empty());
}
