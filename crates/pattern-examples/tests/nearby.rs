//! Tests for the relational upsert and nearby query, driven through the
//! mock provider exactly as the guest invokes them.

mod provider;

use omnia_guest::api::{Invocation, Invoker};
use pattern_examples::{NearbyPlacesRequest, UpsertPlaceRequest};

use self::provider::MockProvider;

async fn upsert(invoker: &Invoker<MockProvider>, id: &str, name: &str, lat: f64, lon: f64) {
    let request = UpsertPlaceRequest {
        id: id.to_string(),
        name: name.to_string(),
        lat,
        lon,
    };
    let reply = invoker
        .invoke::<UpsertPlaceRequest>(Invocation::new(request))
        .await
        .expect("should succeed");
    assert_eq!(reply.affected, 1);
}

async fn nearby(
    invoker: &Invoker<MockProvider>, lat: f64, lon: f64, radius_m: f64,
) -> pattern_examples::NearbyPlacesReply {
    let request = NearbyPlacesRequest { lat, lon, radius_m };
    invoker.invoke::<NearbyPlacesRequest>(Invocation::new(request)).await.expect("should succeed")
}

#[tokio::test]
async fn radius_filters_and_orders_by_distance() {
    let provider = MockProvider::default();
    let invoker = Invoker::new("acme", provider.clone());

    upsert(&invoker, "cbd", "City Centre", -36.8485, 174.7633).await;
    upsert(&invoker, "ferry", "Ferry Terminal", -36.8429, 174.7668).await; // ~700 m away
    upsert(&invoker, "airport", "Airport", -37.0082, 174.7850).await; // ~18 km away

    let reply = nearby(&invoker, -36.8485, 174.7633, 2_000.0).await;

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["cbd", "ferry"]);
    assert!(reply.places[0].distance_m < 1.0);
    assert!((500.0..2_000.0).contains(&reply.places[1].distance_m));
}

#[tokio::test]
async fn bounding_box_corner_is_refined_by_haversine() {
    let provider = MockProvider::default();
    let invoker = Invoker::new("acme", provider.clone());

    // ~557 m due north: inside the radius.
    upsert(&invoker, "near", "Near", 0.005, 0.0).await;
    // ~1,259 m to the north-east corner: inside the 1 km *bounding box*
    // (which spans ~0.009 degrees each way) but outside the true radius.
    upsert(&invoker, "corner", "Corner", 0.008, 0.008).await;

    let reply = nearby(&invoker, 0.0, 0.0, 1_000.0).await;

    let ids: Vec<&str> = reply.places.iter().map(|found| found.place.id.as_str()).collect();
    assert_eq!(ids, ["near"], "haversine refinement should drop the box corner");
    assert!(provider.place("corner").is_some(), "corner row should still be stored");
}

#[tokio::test]
async fn conflicting_upsert_updates_in_place() {
    let provider = MockProvider::default();
    let invoker = Invoker::new("acme", provider.clone());

    upsert(&invoker, "cbd", "City Centre", -36.8485, 174.7633).await;
    upsert(&invoker, "cbd", "Downtown", -36.8486, 174.7634).await;

    let stored = provider.place("cbd").expect("stored");
    assert_eq!(stored.name, "Downtown");

    let reply = nearby(&invoker, -36.8485, 174.7633, 1_000.0).await;
    assert_eq!(reply.places.len(), 1);
    assert_eq!(reply.places[0].place.name, "Downtown");
}
