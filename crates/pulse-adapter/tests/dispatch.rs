//! The adapter's topic driven through its registration: a delivery on the
//! Pulse topic reaches the handler and its events reach the publisher.

use acme_common::{TIMEZONE, config};
use bytes::Bytes;
use chrono::{Timelike, Utc};
use http::{Method, Response};
use omnia_sdk::api::Client;
use omnia_sdk::api::messaging::{Delivery, DeliveryError, Router};
use omnia_test::guest::{MapConfig, MatchedHttp, Provider};
use serde_json::Value;

const STATIC_API_URL: &str = "http://static.test";
const BLOCK_MGT_URL: &str = "http://block-mgt.test";
const ENV: &str = "dev";

fn provider(http: MatchedHttp) -> Provider {
    Provider::default()
        .config(MapConfig::default().with([
            (config::ENV, ENV),
            (config::STATIC_API_URL, STATIC_API_URL),
            (config::BLOCK_MGT_URL, BLOCK_MGT_URL),
            (config::API_IDENTITY, "test-identity"),
        ]))
        .http(http)
}

fn router(provider: &Provider) -> Router<Provider> {
    pulse_adapter::topics(Router::new(Client::new("acme", provider.clone())), ENV)
}

fn delivery(payload: impl Into<Vec<u8>>) -> Delivery {
    Delivery {
        topic: Some("dev-realtime-pulse.v1".to_string()),
        payload: payload.into(),
        ..Delivery::default()
    }
}

fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
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
async fn pulse_xml() {
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

    router(&provider).handle(delivery(pulse_arrival_xml("5226"))).await.expect("should deliver");

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
async fn undecodable_payload() {
    let provider = provider(MatchedHttp::default());

    let error =
        router(&provider).handle(delivery(b"not xml".to_vec())).await.expect_err("malformed XML");

    assert!(matches!(error, DeliveryError::Rejected(_)), "{error}");
    assert!(provider.publish.sent().is_empty());
}

#[tokio::test]
async fn other_environment() {
    let provider = provider(MatchedHttp::default());
    let topic = config::topic_for("prod", acme_common::topic::PULSE);

    let error = router(&provider)
        .handle(Delivery {
            topic: Some(topic.clone()),
            payload: pulse_arrival_xml("5226").into_bytes(),
            ..Delivery::default()
        })
        .await
        .expect_err("wrong env");

    assert_eq!(error, DeliveryError::UnhandledTopic(topic));
    assert!(provider.publish.sent().is_empty());
}
