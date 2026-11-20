//! Tests for the relational upsert and nearby query, driven through the
//! stem's component.
//!
//! `ScriptedTables` answers statements, it does not evaluate them: the
//! bounding-box `SELECT` is scripted with the candidate rows and its bound
//! parameters asserted, and the upsert is asserted as the statement the
//! handler issued.

mod support;

use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, Response, StatusCode};
use bytes::Bytes;
use omnia::{Provides, Runtime};
use omnia_test::host::sql::{Error, Field, Row, Value};
use omnia_test::host::{self, Backends, Executed, ScriptedTables};
use omnia_wasi_http::WasiHttp;
use pattern::place::{
    NearbyPlacesReply, NearbyPlacesRequest, UpsertPlaceReply, UpsertPlaceRequest,
};
use serde_json::json;
use support::boot;

fn place_row(id: &str, name: &str, lat: f64, lon: f64) -> Row {
    let field = |name: &str, value: Value| Field {
        name: name.to_string(),
        value,
    };
    Row {
        fields: vec![
            field("id", Value::Str(Some(id.to_string()))),
            field("name", Value::Str(Some(name.to_string()))),
            field("lat", Value::Double(Some(lat))),
            field("lon", Value::Double(Some(lon))),
        ],
    }
}

// The handler binds the bounds in this order: `lat >=`, `lat <=`, `lon >=`,
// `lon <=`.
fn bounds(statement: &Executed) -> [f64; 4] {
    let [
        Value::Double(Some(lat_min)),
        Value::Double(Some(lat_max)),
        Value::Double(Some(lon_min)),
        Value::Double(Some(lon_max)),
    ] = statement.params.as_slice()
    else {
        panic!("expected four double bounds, got {:?}", statement.params);
    };
    [*lat_min, *lat_max, *lon_min, *lon_max]
}

fn upserted_id(statement: &Executed) -> Option<&str> {
    match statement.params.first() {
        Some(Value::Str(Some(id))) if statement.sql.starts_with("INSERT") => Some(id),
        _ => None,
    }
}

async fn send<B: Provides<WasiHttp>>(
    runtime: &Runtime<B>, method: Method, path: &str, body: Vec<u8>,
) -> Response<Bytes> {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(CONTENT_TYPE, "application/json")
        .body(Bytes::from(body))
        .expect("request");
    host::send(runtime, request).await.expect("request served")
}

async fn upsert<B: Provides<WasiHttp>>(
    runtime: &Runtime<B>, id: &str, name: &str, lat: f64, lon: f64,
) -> Response<Bytes> {
    let payload = UpsertPlaceRequest {
        id: id.to_string(),
        name: name.to_string(),
        lat,
        lon,
    };
    let body = serde_json::to_vec(&payload).expect("serialize");
    send(runtime, Method::POST, "/examples/patterns/places", body).await
}

async fn nearby<B: Provides<WasiHttp>>(
    runtime: &Runtime<B>, lat: f64, lon: f64, radius_m: f64,
) -> NearbyPlacesReply {
    let payload = NearbyPlacesRequest { lat, lon, radius_m };
    let body = serde_json::to_vec(&payload).expect("serialize");
    let response = send(runtime, Method::GET, "/examples/patterns/nearby", body).await;
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_slice(response.body()).expect("reply")
}

#[tokio::test]
async fn radius() {
    const CORNER: (f64, f64) = (-36.8335, 174.7813);
    let tables = ScriptedTables::default().on_query(
        |sql, params| sql.starts_with("SELECT") && params.len() == 4,
        vec![
            // ~2.3 km to the north-east: inside the 2 km bounding box but
            // outside the true radius.
            place_row("corner", "Corner", CORNER.0, CORNER.1),
            place_row("ferry", "Ferry Terminal", -36.8429, 174.7668), // ~700 m away
            place_row("cbd", "City Centre", -36.8485, 174.7633),
        ],
    );
    let runtime = boot(Backends::defaults().await.sql(tables.clone())).await;

    let reply = nearby(&runtime, -36.8485, 174.7633, 2_000.0).await;
    runtime.shutdown();

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["cbd", "ferry"], "haversine refinement should drop the box corner");
    assert!(reply.places[0].distance_m < 1.0);
    assert!((500.0..2_000.0).contains(&reply.places[1].distance_m));

    // The box the database was asked for brackets the centre on both axes
    // and reaches the corner, so only Rust dropped it.
    let statements = tables.statements();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].binding, "default");
    let [lat_min, lat_max, lon_min, lon_max] = bounds(&statements[0]);
    assert!(lat_min < -36.8485 && -36.8485 < lat_max);
    assert!(lon_min < 174.7633 && 174.7633 < lon_max);
    assert!((lat_min..=lat_max).contains(&CORNER.0) && (lon_min..=lon_max).contains(&CORNER.1));
}

#[tokio::test]
async fn upserts() {
    let tables = ScriptedTables::default()
        .on_exec(|sql, params| sql.starts_with("INSERT") && params.len() == 4, 1);
    let runtime = boot(Backends::defaults().await.sql(tables.clone())).await;

    let response = upsert(&runtime, "cbd", "City Centre", -36.8485, 174.7633).await;
    assert_eq!(response.status(), StatusCode::OK);
    let reply: UpsertPlaceReply = serde_json::from_slice(response.body()).expect("reply");
    assert_eq!(reply.affected, 1);

    // The write is the conflict-updating statement, keyed on the id.
    let statements = tables.statements();
    let [statement] = statements.as_slice() else {
        panic!("expected one upsert, got {statements:?}");
    };
    assert!(statement.sql.contains("ON CONFLICT"), "{}", statement.sql);
    assert_eq!(upserted_id(statement), Some("cbd"));
    assert_eq!(statement.params[1], Value::Str(Some("City Centre".to_string())));

    // An out-of-range coordinate is the structured 400: omnia's `ErrorBody`
    // envelope plus the variant's domain fields, with nothing written.
    let response = upsert(&runtime, "bad", "Nowhere", 123.4, 0.0).await;
    runtime.shutdown();
    assert_eq!(tables.statements().len(), 1, "rejected row must not reach the store");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(response.body()).expect("body is JSON");
    assert_eq!(
        body,
        json!({
            "error": "invalid_coordinate",
            "message": "lat 123.4 is outside [-90, 90]",
            "field": "lat",
            "value": 123.4,
            "min": -90.0,
            "max": 90.0,
        })
    );
}

// Exact equality pins that no `description` field leaks beside `message`.
#[tokio::test]
async fn storage_error_body() {
    let refused = Error::Unavailable("connection refused".to_string());
    let tables = ScriptedTables::default().failing_on(|sql, _| sql.starts_with("INSERT"), refused);
    let runtime = boot(Backends::defaults().await.sql(tables)).await;

    let response = upsert(&runtime, "cbd", "City Centre", -36.8485, 174.7633).await;
    runtime.shutdown();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let body: serde_json::Value = serde_json::from_slice(response.body()).expect("body is JSON");
    assert_eq!(body, json!({ "error": "storage", "message": "connection refused" }));
}
