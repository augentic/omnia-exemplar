#![allow(missing_docs)]

//! Native port of the pre-trim docstore example's bash test script: seed the
//! GTFS fixtures through the create handlers, then assert the result counts
//! of every filter combination, pagination via the continuation token, and
//! the CRUD round trips.

use docstore_examples::{
    CreateRouteRequest, CreateStopRequest, CreateStopTimeRequest, DeleteStopRequest,
    GetRouteRequest, GetStopRequest, GetStopTimeRequest, ListRoutesRequest, ListStopTimesRequest,
    ListStopsRequest, Route, RoutesReply, Stop, StopTime, StopTimesReply, StopsReply,
    UpsertStopRequest,
};
use omnia_guest::DocumentStore as _;
use omnia_guest::api::{Client, Metadata};

omnia_test::provider! {
    /// The handlers' one capability, as the production default's in-memory twin.
    pub struct TestProvider: DocumentStore;
}

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

/// Seed the five stops, four routes, and five stop times from the pre-trim
/// example's fixtures.
async fn seed(client: &Client<TestProvider>) {
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
        client.call(request, &Metadata::default()).await.expect("stop should seed");
    }

    let routes = [
        ("route-nex", route("AT", "NEX", "Northern Express", 3, "00AEEF")),
        ("route-east", route("AT", "EAST", "Eastern Line", 2, "EE4D2D")),
        ("route-dev", route("Fullers", "DEV", "Devonport Ferry", 4, "1D4F91")),
        ("route-ilk", route("AT", "ILK", "Inner Link", 3, "8BC53F")),
    ];
    for (id, route) in routes {
        let request = CreateRouteRequest {
            id: id.to_string(),
            route,
        };
        client.call(request, &Metadata::default()).await.expect("route should seed");
    }

    let stop_times = [
        ("nex-0800-1", stop_time("trip-nex-0800", "stop-004", "08:00:00", "08:01:00", 1)),
        ("nex-0800-2", stop_time("trip-nex-0800", "stop-003", "08:15:00", "08:16:00", 2)),
        ("nex-0800-3", stop_time("trip-nex-0800", "stop-001", "08:30:00", "08:31:00", 3)),
        ("east-0900-1", stop_time("trip-east-0900", "stop-001", "09:00:00", "09:01:00", 1)),
        ("east-0900-2", stop_time("trip-east-0900", "stop-002", "09:10:00", "09:11:00", 2)),
    ];
    for (id, stop_time) in stop_times {
        let request = CreateStopTimeRequest {
            id: id.to_string(),
            stop_time,
        };
        client.call(request, &Metadata::default()).await.expect("stop time should seed");
    }
}

async fn list_stops(client: &Client<TestProvider>, request: ListStopsRequest) -> StopsReply {
    client.call(request, &Metadata::default()).await.expect("list stops should succeed")
}

async fn list_routes(client: &Client<TestProvider>, request: ListRoutesRequest) -> RoutesReply {
    client.call(request, &Metadata::default()).await.expect("list routes should succeed")
}

async fn list_stop_times(
    client: &Client<TestProvider>, request: ListStopTimesRequest,
) -> StopTimesReply {
    client.call(request, &Metadata::default()).await.expect("list stop times should succeed")
}

#[tokio::test]
async fn stop_crud_round_trip() {
    let provider = TestProvider::default();
    let client = Client::new("acme", provider.clone());
    seed(&client).await;

    // Get by id.
    let request = GetStopRequest {
        id: "stop-001".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should exist");
    assert_eq!(reply.document.stop_name, "Britomart Transport Centre");

    // Insert rejects a duplicate id.
    let request = CreateStopRequest {
        id: "stop-001".to_string(),
        stop: stop("Duplicate", (0.0, 0.0), None, 0, 0, None, "2026-03-19T00:00:00Z"),
    };
    client.call(request, &Metadata::default()).await.expect_err("duplicate id should fail");

    // Upsert replaces the whole document.
    let request = UpsertStopRequest {
        id: "stop-001".to_string(),
        stop: stop(
            "Britomart Transport Centre",
            (-36.8442, 174.7676),
            Some("zone-1"),
            1,
            1,
            None,
            "2026-03-19T12:00:00Z",
        ),
    };
    client.call(request, &Metadata::default()).await.expect("upsert should succeed");
    let request = GetStopRequest {
        id: "stop-001".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should exist");
    assert_eq!(reply.document.last_updated.as_deref(), Some("2026-03-19T12:00:00Z"));

    // Delete removes the document; a second delete is a 404.
    let request = DeleteStopRequest {
        id: "stop-005".to_string(),
    };
    let reply = client.call(request.clone(), &Metadata::default()).await.expect("should delete");
    assert_eq!(reply.id, "stop-005");
    assert!(provider.docs.get("stops", "stop-005").await.expect("get").is_none());
    let error = client.call(request, &Metadata::default()).await.expect_err("second delete fails");
    assert_eq!(error.code(), "not_found");
}

#[tokio::test]
async fn stop_filters() {
    let provider = TestProvider::default();
    let client = Client::new("acme", provider);
    seed(&client).await;

    // All stops, sorted by name.
    let reply = list_stops(&client, ListStopsRequest::default()).await;
    assert_eq!(reply.stops.len(), 5);
    assert_eq!(reply.stops[0].document.stop_name, "Albany Station");

    // Text search: contains on stop_name.
    let request = ListStopsRequest {
        q: Some("Station".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 2);

    // Zone: eq on zone_id.
    let request = ListStopsRequest {
        zone: Some("zone-1".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 2);

    // Zone exclusion: ne on zone_id (a null zone is "not equal").
    let request = ListStopsRequest {
        exclude_zone: Some("zone-1".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 3);

    // Accessible: eq(wheelchair_boarding, 1) + is_not_null(zone_id).
    let request = ListStopsRequest {
        accessible: Some(true),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 3);

    // Top level: is_null(parent_station).
    let request = ListStopsRequest {
        top_level: Some(true),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 4);

    // Bounding box: and(gte, lte, gte, lte).
    let request = ListStopsRequest {
        min_lat: Some(-36.86),
        max_lat: Some(-36.83),
        min_lon: Some(174.74),
        max_lon: Some(174.80),
        ..Default::default()
    };
    let reply = list_stops(&client, request).await;
    let ids: Vec<&str> = reply.stops.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["stop-001", "stop-005", "stop-003"]);

    // Updated on a calendar date: on_date(last_updated).
    let request = ListStopsRequest {
        updated_on: Some("2026-03-19".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 3);

    // Combined: accessible + zone.
    let request = ListStopsRequest {
        accessible: Some(true),
        zone: Some("zone-1".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stops(&client, request).await.stops.len(), 2);

    // An invalid date is rejected as a bad request, not a server error.
    let request = ListStopsRequest {
        updated_on: Some("not-a-date".to_string()),
        ..Default::default()
    };
    let error =
        client.call(request, &Metadata::default()).await.expect_err("invalid date should fail");
    assert_eq!(error.code(), "bad_request");
}

#[tokio::test]
async fn stop_pagination() {
    let provider = TestProvider::default();
    let client = Client::new("acme", provider);
    seed(&client).await;

    // Page 1: the first two stops by name, with a continuation token.
    let request = ListStopsRequest {
        limit: Some(2),
        ..Default::default()
    };
    let page1 = list_stops(&client, request).await;
    let names: Vec<&str> =
        page1.stops.iter().map(|record| record.document.stop_name.as_str()).collect();
    assert_eq!(names, ["Albany Station", "Britomart Transport Centre"]);
    let token = page1.continuation.expect("page 1 should have a continuation");

    // Page 2: resumes exactly where page 1 stopped.
    let request = ListStopsRequest {
        limit: Some(2),
        continuation: Some(token),
        ..Default::default()
    };
    let page2 = list_stops(&client, request).await;
    let names: Vec<&str> =
        page2.stops.iter().map(|record| record.document.stop_name.as_str()).collect();
    assert_eq!(names, ["Devonport Ferry Terminal", "Newmarket Station"]);
    let token = page2.continuation.expect("page 2 should have a continuation");

    // Page 3: the final stop, and no further continuation.
    let request = ListStopsRequest {
        limit: Some(2),
        continuation: Some(token),
        ..Default::default()
    };
    let page3 = list_stops(&client, request).await;
    let names: Vec<&str> =
        page3.stops.iter().map(|record| record.document.stop_name.as_str()).collect();
    assert_eq!(names, ["Ponsonby Rd at Franklin Rd"]);
    assert!(page3.continuation.is_none());
}

#[tokio::test]
async fn route_filters() {
    let provider = TestProvider::default();
    let client = Client::new("acme", provider);
    seed(&client).await;

    // Get by id.
    let request = GetRouteRequest {
        id: "route-nex".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should exist");
    assert_eq!(reply.document.route_short_name, "NEX");
    let request = GetRouteRequest {
        id: "route-missing".to_string(),
    };
    let error = client.call(request, &Metadata::default()).await.expect_err("missing route");
    assert_eq!(error.code(), "not_found");

    // All routes, sorted by short name.
    let reply = list_routes(&client, ListRoutesRequest::default()).await;
    let names: Vec<&str> =
        reply.routes.iter().map(|record| record.document.route_short_name.as_str()).collect();
    assert_eq!(names, ["DEV", "EAST", "ILK", "NEX"]);

    // Name search: or(contains(short), contains(long)).
    let request = ListRoutesRequest {
        q: Some("Northern".to_string()),
        ..Default::default()
    };
    let reply = list_routes(&client, request).await;
    assert_eq!(reply.routes.len(), 1);
    assert_eq!(reply.routes[0].id, "route-nex");

    // Route types: in_list(route_type, [2, 3]).
    let request = ListRoutesRequest {
        types: Some("2,3".to_string()),
        ..Default::default()
    };
    assert_eq!(list_routes(&client, request).await.routes.len(), 3);

    // Agency: eq(agency_id).
    let request = ListRoutesRequest {
        agency: Some("AT".to_string()),
        ..Default::default()
    };
    assert_eq!(list_routes(&client, request).await.routes.len(), 3);

    // Exclude ferries: negate(eq(route_type, 4)).
    let request = ListRoutesRequest {
        exclude_type: Some(4),
        ..Default::default()
    };
    assert_eq!(list_routes(&client, request).await.routes.len(), 3);

    // Exclude AT buses: negate(and(...)) — De Morgan negation.
    let request = ListRoutesRequest {
        not_agency: Some("AT".to_string()),
        not_type: Some(3),
        ..Default::default()
    };
    let reply = list_routes(&client, request).await;
    let ids: Vec<&str> = reply.routes.iter().map(|record| record.id.as_str()).collect();
    assert_eq!(ids, ["route-dev", "route-east"]);

    // Combined: AT bus and rail, no ferries.
    let request = ListRoutesRequest {
        agency: Some("AT".to_string()),
        types: Some("2,3".to_string()),
        exclude_type: Some(4),
        ..Default::default()
    };
    assert_eq!(list_routes(&client, request).await.routes.len(), 3);
}

#[tokio::test]
async fn stop_time_filters() {
    let provider = TestProvider::default();
    let client = Client::new("acme", provider);
    seed(&client).await;

    // Get by id.
    let request = GetStopTimeRequest {
        id: "nex-0800-1".to_string(),
    };
    let reply = client.call(request, &Metadata::default()).await.expect("should exist");
    assert_eq!(reply.document.trip_id, "trip-nex-0800");

    // Trip: eq(trip_id), sorted by sequence.
    let request = ListStopTimesRequest {
        trip: Some("trip-nex-0800".to_string()),
        ..Default::default()
    };
    let reply = list_stop_times(&client, request).await;
    let sequences: Vec<i32> =
        reply.stop_times.iter().map(|record| record.document.stop_sequence).collect();
    assert_eq!(sequences, [1, 2, 3]);

    // Stop: eq(stop_id).
    let request = ListStopTimesRequest {
        stop: Some("stop-001".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stop_times(&client, request).await.stop_times.len(), 2);

    // Arrival window: gte + lte on arrival_time.
    let request = ListStopTimesRequest {
        after: Some("08:00:00".to_string()),
        before: Some("08:30:00".to_string()),
        ..Default::default()
    };
    assert_eq!(list_stop_times(&client, request).await.stop_times.len(), 3);

    // Trip plus sequence range: eq + gte + lte.
    let request = ListStopTimesRequest {
        trip: Some("trip-nex-0800".to_string()),
        min_seq: Some(1),
        max_seq: Some(2),
        ..Default::default()
    };
    assert_eq!(list_stop_times(&client, request).await.stop_times.len(), 2);
}
