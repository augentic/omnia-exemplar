//! Tests for the relational upsert and nearby query, driven through the
//! stem's router.
//!
//! `ScriptedTables` answers statements, it does not evaluate them: the
//! bounding-box `SELECT` is scripted with the candidate rows and its bound
//! parameters asserted, and the upsert is asserted as the statement the
//! handler issued.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse as _;
use omnia_sdk::api::Client;
use omnia_sdk::sql::{Field, Row, Value};
use omnia_test::guest::{self, Executed, Provider, ScriptedTables};
use pattern::place::{
    NearbyPlacesReply, NearbyPlacesRequest, PlaceError, UpsertPlaceReply, UpsertPlaceRequest,
};

fn place_row(id: &str, name: &str, lat: f64, lon: f64) -> Row {
    let field = |name: &str, value: Value| Field {
        name: name.to_string(),
        value,
    };
    Row {
        fields: vec![
            field("id", Value::from(id)),
            field("name", Value::from(name)),
            field("lat", Value::from(lat)),
            field("lon", Value::from(lon)),
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

async fn send(provider: &Provider, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let router = pattern::handlers().with_state(Client::new(provider.clone()));
    let response = guest::send(&router, request).await;
    (response.status(), response.into_body().to_vec())
}

async fn upsert(
    provider: &Provider, id: &str, name: &str, lat: f64, lon: f64,
) -> (StatusCode, Vec<u8>) {
    let payload = UpsertPlaceRequest {
        id: id.to_string(),
        name: name.to_string(),
        lat,
        lon,
    };
    let request = Request::post("/examples/patterns/places")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&payload).expect("serialize")))
        .expect("request");
    send(provider, request).await
}

async fn nearby(provider: &Provider, lat: f64, lon: f64, radius_m: f64) -> NearbyPlacesReply {
    let payload = NearbyPlacesRequest { lat, lon, radius_m };
    let request = Request::get("/examples/patterns/nearby")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&payload).expect("serialize")))
        .expect("request");
    let (status, body) = send(provider, request).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_slice(&body).expect("reply")
}

#[tokio::test]
async fn radius() {
    const CORNER: (f64, f64) = (-36.8335, 174.7813);
    let provider = Provider::default().sql(ScriptedTables::default().on_query(
        |sql, params| sql.starts_with("SELECT") && params.len() == 4,
        vec![
            // ~2.3 km to the north-east: inside the 2 km bounding box but
            // outside the true radius.
            place_row("corner", "Corner", CORNER.0, CORNER.1),
            place_row("ferry", "Ferry Terminal", -36.8429, 174.7668), // ~700 m away
            place_row("cbd", "City Centre", -36.8485, 174.7633),
        ],
    ));

    let reply = nearby(&provider, -36.8485, 174.7633, 2_000.0).await;

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["cbd", "ferry"], "haversine refinement should drop the box corner");
    assert!(reply.places[0].distance_m < 1.0);
    assert!((500.0..2_000.0).contains(&reply.places[1].distance_m));

    // The box the database was asked for brackets the centre on both axes
    // and reaches the corner, so only Rust dropped it.
    let statements = provider.sql.statements();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].binding, "default");
    let [lat_min, lat_max, lon_min, lon_max] = bounds(&statements[0]);
    assert!(lat_min < -36.8485 && -36.8485 < lat_max);
    assert!(lon_min < 174.7633 && 174.7633 < lon_max);
    assert!((lat_min..=lat_max).contains(&CORNER.0) && (lon_min..=lon_max).contains(&CORNER.1));
}

#[tokio::test]
async fn upserts() {
    let provider = Provider::default().sql(
        ScriptedTables::default()
            .on_exec(|sql, params| sql.starts_with("INSERT") && params.len() == 4, 1),
    );

    let (status, body) = upsert(&provider, "cbd", "City Centre", -36.8485, 174.7633).await;
    assert_eq!(status, StatusCode::OK);
    let reply: UpsertPlaceReply = serde_json::from_slice(&body).expect("reply");
    assert_eq!(reply.affected, 1);

    // The write is the conflict-updating statement, keyed on the id.
    let statements = provider.sql.statements();
    let [statement] = statements.as_slice() else {
        panic!("expected one upsert, got {statements:?}");
    };
    assert!(statement.sql.contains("ON CONFLICT"), "{}", statement.sql);
    assert_eq!(upserted_id(statement), Some("cbd"));
    assert_eq!(statement.params[1], Value::from("City Centre"));

    // An out-of-range coordinate is the structured 400: omnia's `ErrorBody`
    // envelope plus the variant's domain fields, with nothing written.
    let (status, bytes) = upsert(&provider, "bad", "Nowhere", 123.4, 0.0).await;
    assert_eq!(provider.sql.statements().len(), 1, "rejected row must not reach the store");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body is JSON");
    assert_eq!(
        body,
        serde_json::json!({
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
    let error = PlaceError::from(anyhow::anyhow!("connection refused").context("executing upsert"));
    let message = error.to_string();

    let (status, body) = wire(error).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body, serde_json::json!({ "error": "storage", "message": message }));
}

async fn wire(error: PlaceError) -> (StatusCode, serde_json::Value) {
    let response = error.into_response();
    assert_eq!(
        response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.expect("body collects");
    (status, serde_json::from_slice(&bytes).expect("body is JSON"))
}
