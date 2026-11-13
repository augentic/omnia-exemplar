//! Integration tests for the docstore collections and filters, driven
//! through the stem's router.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use docstore::route::{CreateRouteRequest, Route, RoutesReply};
use docstore::stop::{CreateStopRequest, DeleteStopReply, DocumentRecord, Stop, StopsReply};
use docstore::stop_time::{CreateStopTimeRequest, StopTime, StopTimesReply};
use omnia_sdk::api::Client;
use omnia_sdk::docstore::Filter;
use omnia_test::guest::{self, Provider};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

fn stop(
    name: &str, coords: (f64, f64), zone: Option<&str>, wheelchair: i32, location: i32,
    parent: Option<&str>, updated: &str,
) -> Stop {
    Stop {
        stop_name: name.to_string(),
        stop_lat: coords.0,
        stop_lon: coords.1,
        zone_id: zone.map(ToString::to_string),
        wheelchair_boarding: Some(wheelchair),
        location_type: Some(location),
        parent_station: parent.map(ToString::to_string),
        last_updated: Some(updated.to_string()),
    }
}

fn route(agency: &str, short: &str, long: &str, route_type: i32, color: &str) -> Route {
    Route {
        agency_id: agency.to_string(),
        route_short_name: short.to_string(),
        route_long_name: long.to_string(),
        route_type,
        route_color: Some(color.to_string()),
    }
}

fn stop_time(trip: &str, stop: &str, arrival: &str, departure: &str, sequence: i32) -> StopTime {
    StopTime {
        trip_id: trip.to_string(),
        stop_id: stop.to_string(),
        arrival_time: arrival.to_string(),
        departure_time: departure.to_string(),
        stop_sequence: sequence,
        pickup_type: Some(0),
        drop_off_type: Some(0),
    }
}

async fn send(
    provider: &Provider, method: Method, uri: &str, body: Option<Vec<u8>>,
) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(bytes) => builder.header(CONTENT_TYPE, "application/json").body(Body::from(bytes)),
        None => builder.body(Body::empty()),
    }
    .expect("request");
    let router = docstore::handlers().with_state(Client::new(provider.clone()));
    let response = guest::send(&router, request).await;
    let json = serde_json::from_slice(response.body()).unwrap_or(Value::Null);
    (response.status(), json)
}

async fn ok<T: DeserializeOwned>(
    provider: &Provider, method: Method, uri: &str, body: Option<Vec<u8>>,
) -> T {
    let (status, value) = send(provider, method, uri, body).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    serde_json::from_value(value).expect("reply")
}

fn json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("serialize")
}

async fn seed(provider: &Provider) {
    let stops = [
        (
            "stop-001",
            stop(
                "Britomart Transport Centre",
                (-36.8442, 174.7676),
                Some("zone-1"),
                1,
                1,
                None,
                "2026-03-19T10:00:00Z",
            ),
        ),
        (
            "stop-002",
            stop(
                "Newmarket Station",
                (-36.8690, 174.7779),
                Some("zone-1"),
                1,
                0,
                Some("stop-001"),
                "2026-03-19T10:00:00Z",
            ),
        ),
        (
            "stop-003",
            stop(
                "Ponsonby Rd at Franklin Rd",
                (-36.8556, 174.7437),
                Some("zone-2"),
                0,
                0,
                None,
                "2026-03-18T08:00:00Z",
            ),
        ),
        (
            "stop-004",
            stop(
                "Albany Station",
                (-36.7275, 174.6986),
                Some("zone-3"),
                1,
                1,
                None,
                "2026-03-17T09:00:00Z",
            ),
        ),
        (
            "stop-005",
            stop(
                "Devonport Ferry Terminal",
                (-36.8326, 174.7950),
                None,
                1,
                1,
                None,
                "2026-03-19T11:00:00Z",
            ),
        ),
    ];
    for (id, stop) in stops {
        let request = CreateStopRequest {
            id: id.to_string(),
            stop,
        };
        let _: DocumentRecord<Stop> =
            ok(provider, Method::POST, "/examples/stops", Some(json_bytes(&request))).await;
    }

    for (id, route) in [
        ("route-nex", route("AT", "NEX", "Northern Express", 3, "00AEEF")),
        ("route-east", route("AT", "EAST", "Eastern Line", 2, "EE4D2D")),
        ("route-dev", route("Fullers", "DEV", "Devonport Ferry", 4, "1D4F91")),
        ("route-ilk", route("AT", "ILK", "Inner Link", 3, "8BC53F")),
    ] {
        let request = CreateRouteRequest {
            id: id.to_string(),
            route,
        };
        let _: DocumentRecord<Route> =
            ok(provider, Method::POST, "/examples/routes", Some(json_bytes(&request))).await;
    }

    for (id, stop_time) in [
        ("nex-0800-1", stop_time("trip-nex-0800", "stop-004", "08:00:00", "08:01:00", 1)),
        ("nex-0800-2", stop_time("trip-nex-0800", "stop-003", "08:15:00", "08:16:00", 2)),
        ("nex-0800-3", stop_time("trip-nex-0800", "stop-001", "08:30:00", "08:31:00", 3)),
        ("east-0900-1", stop_time("trip-east-0900", "stop-001", "09:00:00", "09:01:00", 1)),
        ("east-0900-2", stop_time("trip-east-0900", "stop-002", "09:10:00", "09:11:00", 2)),
    ] {
        let request = CreateStopTimeRequest {
            id: id.to_string(),
            stop_time,
        };
        let _: DocumentRecord<StopTime> =
            ok(provider, Method::POST, "/examples/stop-times", Some(json_bytes(&request))).await;
    }
}

fn with_query(base: &str, query: &str) -> String {
    if query.is_empty() { base.to_string() } else { format!("{base}?{query}") }
}

async fn query_stops(provider: &Provider, query: &str) -> StopsReply {
    ok(provider, Method::GET, &with_query("/examples/stops", query), None).await
}

async fn query_routes(provider: &Provider, query: &str) -> RoutesReply {
    ok(provider, Method::GET, &with_query("/examples/routes", query), None).await
}

async fn query_stop_times(provider: &Provider, query: &str) -> StopTimesReply {
    ok(provider, Method::GET, &with_query("/examples/stop-times", query), None).await
}

#[tokio::test]
async fn stop_crud() {
    let provider = Provider::default();
    seed(&provider).await;

    // Get by id.
    let reply: DocumentRecord<Stop> =
        ok(&provider, Method::GET, "/examples/stops/stop-001", None).await;
    assert_eq!(reply.document.stop_name, "Britomart Transport Centre");

    // A create under a stored id is a 409 and leaves the stored stop as it was.
    let duplicate = CreateStopRequest {
        id: "stop-001".to_string(),
        stop: stop("Imposter", (0.0, 0.0), None, 0, 0, None, "2026-03-20T00:00:00Z"),
    };
    let (status, body) =
        send(&provider, Method::POST, "/examples/stops", Some(json_bytes(&duplicate))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "conflict");
    assert_eq!(body["id"], "stop-001");
    let reply: DocumentRecord<Stop> =
        ok(&provider, Method::GET, "/examples/stops/stop-001", None).await;
    assert_eq!(reply.document.stop_name, "Britomart Transport Centre");

    // Upsert replaces the whole document.
    let replacement = stop(
        "Britomart Transport Centre",
        (-36.8442, 174.7676),
        Some("zone-1"),
        1,
        1,
        None,
        "2026-03-19T12:00:00Z",
    );
    let _: DocumentRecord<Stop> =
        ok(&provider, Method::PUT, "/examples/stops/stop-001", Some(json_bytes(&replacement)))
            .await;
    let reply: DocumentRecord<Stop> =
        ok(&provider, Method::GET, "/examples/stops/stop-001", None).await;
    assert_eq!(reply.document.last_updated.as_deref(), Some("2026-03-19T12:00:00Z"));

    // Delete removes the document; a second delete is a 404.
    let reply: DeleteStopReply =
        ok(&provider, Method::DELETE, "/examples/stops/stop-005", None).await;
    assert_eq!(reply.id, "stop-005");
    let (status, body) = send(&provider, Method::DELETE, "/examples/stops/stop-005", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");
}

#[tokio::test]
async fn stop_filters() {
    let provider = Provider::default();
    seed(&provider).await;

    // All stops, sorted by name.
    let reply = query_stops(&provider, "").await;
    assert_eq!(reply.stops.len(), 5);
    assert_eq!(reply.stops[0].document.stop_name, "Albany Station");

    // Text search: contains on stop_name.
    assert_eq!(query_stops(&provider, "q=Station").await.stops.len(), 2);

    // Zone: eq on zone_id.
    assert_eq!(query_stops(&provider, "zone=zone-1").await.stops.len(), 2);

    // Zone exclusion: ne on zone_id.
    assert_eq!(query_stops(&provider, "exclude_zone=zone-1").await.stops.len(), 3);

    // Accessible: eq(wheelchair_boarding, 1) + is_not_null(zone_id).
    assert_eq!(query_stops(&provider, "accessible=true").await.stops.len(), 3);

    // Top level: is_null(parent_station).
    assert_eq!(query_stops(&provider, "top_level=true").await.stops.len(), 4);

    // Bounding box: and(gte, lte, gte, lte).
    let reply =
        query_stops(&provider, "min_lat=-36.86&max_lat=-36.83&min_lon=174.74&max_lon=174.80").await;
    let ids: Vec<&str> = reply.stops.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["stop-001", "stop-005", "stop-003"]);

    // Updated on a calendar date: on_date(last_updated).
    assert_eq!(query_stops(&provider, "updated_on=2026-03-19").await.stops.len(), 3);

    // An invalid date is rejected as a bad request, not a server error.
    let (status, body) =
        send(&provider, Method::GET, "/examples/stops?updated_on=not-a-date", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");

    // Pagination: limit caps the page and yields a continuation token.
    let page1 = query_stops(&provider, "limit=2").await;
    assert_eq!(page1.stops.len(), 2);
    let token = page1.continuation.expect("page 1 should have a continuation");

    // The token is forwarded, so page 2 resumes where page 1 stopped.
    let page2 = query_stops(&provider, &format!("limit=2&continuation={token}")).await;
    let names: Vec<&str> =
        page2.stops.iter().map(|record| record.document.stop_name.as_str()).collect();
    assert_eq!(names, ["Devonport Ferry Terminal", "Newmarket Station"]);
}

#[tokio::test]
async fn route_filters() {
    let provider = Provider::default();
    seed(&provider).await;

    // Get by id.
    let reply: DocumentRecord<Route> =
        ok(&provider, Method::GET, "/examples/routes/route-nex", None).await;
    assert_eq!(reply.document.route_short_name, "NEX");
    let (status, body) = send(&provider, Method::GET, "/examples/routes/route-missing", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not_found");

    // Name search: or(contains(short), contains(long)).
    let reply = query_routes(&provider, "q=Northern").await;
    assert_eq!(reply.routes.len(), 1);
    assert_eq!(reply.routes[0].id, "route-nex");

    // Route types: in_list(route_type, [2, 3]).
    assert_eq!(query_routes(&provider, "types=2,3").await.routes.len(), 3);

    // More route types than an in_list holds is a bad request, not a server error.
    let types = vec!["3"; Filter::MAX_LIST_LEN + 1].join(",");
    let (status, body) =
        send(&provider, Method::GET, &format!("/examples/routes?types={types}"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "bad_request");

    // Agency: eq(agency_id).
    assert_eq!(query_routes(&provider, "agency=AT").await.routes.len(), 3);

    // Exclude ferries: negate(eq(route_type, 4)).
    assert_eq!(query_routes(&provider, "exclude_type=4").await.routes.len(), 3);

    // Exclude AT buses: negate(and(...)) — De Morgan negation.
    let reply = query_routes(&provider, "not_agency=AT&not_type=3").await;
    let ids: Vec<&str> = reply.routes.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["route-dev", "route-east"]);
}

#[tokio::test]
async fn stop_time_filters() {
    let provider = Provider::default();
    seed(&provider).await;

    // Get by id.
    let reply: DocumentRecord<StopTime> =
        ok(&provider, Method::GET, "/examples/stop-times/nex-0800-1", None).await;
    assert_eq!(reply.document.trip_id, "trip-nex-0800");

    // Stop: eq(stop_id).
    assert_eq!(query_stop_times(&provider, "stop=stop-001").await.stop_times.len(), 2);

    // Arrival window: gte + lte on arrival_time.
    assert_eq!(
        query_stop_times(&provider, "after=08:00:00&before=08:30:00").await.stop_times.len(),
        3
    );

    // Trip plus sequence range: eq + gte + lte.
    assert_eq!(
        query_stop_times(&provider, "trip=trip-nex-0800&min_seq=1&max_seq=2")
            .await
            .stop_times
            .len(),
        2
    );
}
