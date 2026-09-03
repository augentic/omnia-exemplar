//! Tests for the relational upsert and nearby query, driven exactly as the
//! guest invokes them.
//!
//! `ScriptedTables` answers statements, it does not evaluate them: the
//! bounding-box `SELECT` is scripted with the candidate rows and its bound
//! parameters asserted, and the upserts are asserted as the statements the
//! handler issued.

use omnia_guest::api::{Client, Metadata};
use omnia_guest::orm::{DataType, Field, Row};
use omnia_test::guest::{ScriptedTables, Statement};
use pattern_examples::{NearbyPlacesRequest, UpsertPlaceRequest};

omnia_test::provider! {
    /// The handlers' one capability, as a scripted double.
    pub struct TestProvider: TableStore;
}

/// A `places` row as the nearby query maps it.
fn place_row(id: &str, name: &str, lat: f64, lon: f64) -> Row {
    let field = |name: &str, value: DataType| Field {
        name: name.to_string(),
        value,
    };
    Row {
        index: id.to_string(),
        fields: vec![
            field("id", DataType::Str(Some(id.to_string()))),
            field("name", DataType::Str(Some(name.to_string()))),
            field("lat", DataType::Double(Some(lat))),
            field("lon", DataType::Double(Some(lon))),
        ],
    }
}

/// Upserts succeed; the bounding-box query returns `candidates`.
fn provider(candidates: Vec<Row>) -> TestProvider {
    TestProvider::default().tables(
        ScriptedTables::default()
            .on_exec(|sql, params| sql.starts_with("INSERT") && params.len() == 4, 1)
            .on_query(|sql, params| sql.starts_with("SELECT") && params.len() == 4, candidates),
    )
}

/// The four bounds of a `SELECT`, in the order the handler binds them:
/// `lat >=`, `lat <=`, `lon >=`, `lon <=`.
fn bounds(statement: &Statement) -> [f64; 4] {
    let [
        DataType::Double(Some(lat_min)),
        DataType::Double(Some(lat_max)),
        DataType::Double(Some(lon_min)),
        DataType::Double(Some(lon_max)),
    ] = statement.params.as_slice()
    else {
        panic!("expected four double bounds, got {:?}", statement.params);
    };
    [*lat_min, *lat_max, *lon_min, *lon_max]
}

/// The id of an upsert statement, its first bound parameter.
fn upserted_id(statement: &Statement) -> Option<&str> {
    match statement.params.first() {
        Some(DataType::Str(Some(id))) if statement.sql.starts_with("INSERT") => Some(id),
        _ => None,
    }
}

async fn upsert(client: &Client<TestProvider>, id: &str, name: &str, lat: f64, lon: f64) {
    let request = UpsertPlaceRequest {
        id: id.to_string(),
        name: name.to_string(),
        lat,
        lon,
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should succeed");
    assert_eq!(reply.affected, 1);
}

async fn nearby(
    client: &Client<TestProvider>, lat: f64, lon: f64, radius_m: f64,
) -> pattern_examples::NearbyPlacesReply {
    let request = NearbyPlacesRequest { lat, lon, radius_m };
    client.call(request, &Metadata::default()).await.expect("should succeed")
}

#[tokio::test]
async fn radius_filters_and_orders_by_distance() {
    let provider = provider(vec![
        place_row("airport", "Airport", -37.0082, 174.7850), // ~18 km away
        place_row("ferry", "Ferry Terminal", -36.8429, 174.7668), // ~700 m away
        place_row("cbd", "City Centre", -36.8485, 174.7633),
    ]);
    let client = Client::new("acme", provider.clone());

    let reply = nearby(&client, -36.8485, 174.7633, 2_000.0).await;

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["cbd", "ferry"]);
    assert!(reply.places[0].distance_m < 1.0);
    assert!((500.0..2_000.0).contains(&reply.places[1].distance_m));

    // The box the database was asked for brackets the centre on both axes.
    let statements = provider.tables.statements();
    assert_eq!(statements.len(), 1);
    assert_eq!(statements[0].connection, pattern_examples::place::CONNECTION);
    let [lat_min, lat_max, lon_min, lon_max] = bounds(&statements[0]);
    assert!(lat_min < -36.8485 && -36.8485 < lat_max);
    assert!(lon_min < 174.7633 && 174.7633 < lon_max);
}

#[tokio::test]
async fn bounding_box_corner_is_refined_by_haversine() {
    let provider = provider(vec![
        // ~557 m due north: inside the radius.
        place_row("near", "Near", 0.005, 0.0),
        // ~1,259 m to the north-east corner: inside the 1 km *bounding box*
        // (which spans ~0.009 degrees each way) but outside the true radius.
        place_row("corner", "Corner", 0.008, 0.008),
    ]);
    let client = Client::new("acme", provider.clone());

    let reply = nearby(&client, 0.0, 0.0, 1_000.0).await;

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["near"], "haversine refinement should drop the box corner");

    // The corner is inside the box the handler asked for; only Rust dropped it.
    let [lat_min, lat_max, lon_min, lon_max] = bounds(&provider.tables.statements()[0]);
    assert!((lat_min..=lat_max).contains(&0.008) && (lon_min..=lon_max).contains(&0.008));
}

#[tokio::test]
async fn upsert_rejects_out_of_range_coordinates() {
    let provider = provider(vec![]);
    let client = Client::new("acme", provider.clone());

    let request = UpsertPlaceRequest {
        id: "bad".to_string(),
        name: "Nowhere".to_string(),
        lat: 123.4,
        lon: 0.0,
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("should reject");

    // The error serializes to the exact JSON body the HTTP route puts on
    // the wire via the `From<PlaceError> for HttpError` conversion.
    let body = serde_json::to_value(&error).expect("should serialize");
    assert_eq!(
        body,
        serde_json::json!({
            "code": "invalid_coordinate",
            "field": "lat",
            "value": 123.4,
            "min": -90.0,
            "max": 90.0,
        })
    );
    assert!(provider.tables.statements().is_empty(), "rejected row must not reach the store");
}

#[tokio::test]
async fn conflicting_upsert_updates_in_place() {
    let provider = provider(vec![]);
    let client = Client::new("acme", provider.clone());

    upsert(&client, "cbd", "City Centre", -36.8485, 174.7633).await;
    upsert(&client, "cbd", "Downtown", -36.8486, 174.7634).await;

    // Both writes are the same conflict-updating statement on the same id,
    // so the store keeps one row and the second write wins.
    let statements = provider.tables.statements();
    assert_eq!(statements.len(), 2);
    assert_eq!(statements[0].sql, statements[1].sql);
    assert!(statements[0].sql.contains("ON CONFLICT"), "{}", statements[0].sql);
    assert_eq!(upserted_id(&statements[0]), Some("cbd"));
    assert_eq!(upserted_id(&statements[1]), Some("cbd"));
    assert!(matches!(&statements[1].params[1], DataType::Str(Some(name)) if name == "Downtown"));
}
