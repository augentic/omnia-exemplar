//! # Root-package guest
//!
//! Wires the shared transit handlers to WASI HTTP and WASI Messaging with
//! the explicit typed routers from `omnia_guest::api`: HTTP routes are
//! `axum::routing::MethodRouter`s over a provider-owning `Client`, and
//! messaging topics dispatch through an exact-topic `messaging::Router`.
//! Each route is bound to a handler fn (`post(tally)`, `consume(motion)`);
//! routes that speak JSON use the default `get` / `post` / `consume`
//! codecs, and the Pulse SOAP/XML routes supply their own.
//!
//! This root-package layout (`src/lib.rs`) is the compiling reference for
//! new Omnia services. Routes and topics come from the canonical tables in
//! [`acme_common::routes`].
//!
//! Only the WASI exports are `wasm32`-gated. The routers are generic over
//! the provider so the native route rung (`tests/routes.rs`) drives the
//! production routing table under `omnia_test::provider!` doubles.

use acme_common::{config, routes};
use axum::Json;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use capability::{alert, archive, note, reading};
use docstore::{
    create_route, create_stop, create_stop_time, delete_stop, get_route, get_stop, get_stop_time,
    list_routes, list_stop_times, list_stops, upsert_stop,
};
#[cfg(feature = "god-mode")]
use gtfs_adapter::set_trip;
use gtfs_adapter::{motion, passenger_count, train_avl, vehicle_info};
#[cfg(target_arch = "wasm32")]
use omnia_guest::api::http::serve;
use omnia_guest::api::http::{
    MethodFilter, RawRequest, delete, get, handle_with, patch, post, put,
};
use omnia_guest::api::messaging::{self, Delivery, consume, consume_with};
use omnia_guest::api::{Client, DecodeError};
use omnia_guest::{
    BlobStore, Broadcast, Config, DocumentStore, HttpError, HttpRequest, Identity, Publish,
    StateStore, TableStore,
};
#[cfg(target_arch = "wasm32")]
use omnia_wasi_messaging::types::{Error, Message};
use pattern::{
    NearbyPlacesReply, NearbyPlacesRequest, decode_segment, nearby_places, upsert_place,
};
use pulse_adapter::PulseMessage;
use pulse_connector::{PulseReply, PulseXml};
use sql::{
    create_agency, create_feed, delete_feed, get_agency, list_agencies, list_agency_feeds,
    list_all_feeds, update_agency,
};
use tally_connector::tally;
#[cfg(target_arch = "wasm32")]
use tracing::Level;
#[cfg(target_arch = "wasm32")]
use wasip3::exports::http::handler::Guest;
#[cfg(target_arch = "wasm32")]
use wasip3::http::types as p3;

/// The tenant that owns this deployment.
pub const OWNER: &str = "acme";

#[cfg(target_arch = "wasm32")]
omnia_guest::provider! {
    /// Bare provider backed by the default WASI capability implementations.
    pub struct Provider: BlobStore + Broadcast + Config + DocumentStore + HttpRequest + Identity
        + Publish + StateStore + TableStore;
}

/// WASI HTTP export.
#[cfg(target_arch = "wasm32")]
pub struct Http;
#[cfg(target_arch = "wasm32")]
wasip3::http::service::export!(Http);

#[cfg(target_arch = "wasm32")]
impl Guest for Http {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
    async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
        serve(router(Provider), request).await
    }
}

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one WASI component instance per HTTP request, so the router
/// and client are constructed inside each `handle` call; axum's route-state
/// clones share the client's provider allocation for that request only. The
/// bound is the union of every route handler's capability list.
pub fn router<P>(provider: P) -> axum::Router
where
    P: BlobStore
        + Broadcast
        + Config
        + DocumentStore
        + HttpRequest
        + Identity
        + Publish
        + StateStore
        + TableStore
        + Send
        + Sync
        + 'static,
{
    let router = axum::Router::new()
        .route(routes::http::APC, post(tally))
        .route(
            routes::http::PULSE_XML,
            handle_with(
                MethodFilter::POST,
                pulse_connector::pulse,
                |raw: RawRequest<'_>| decode_pulse(raw.body),
                |reply| encode_pulse(&reply),
            ),
        )
        .route(routes::http::VEHICLE_INFO, get(vehicle_info))
        // Pattern-example routes, outside the canonical transit tables.
        .route(pattern::routes::DECODE, post(decode_segment))
        .route(pattern::routes::PLACES, post(upsert_place))
        // The default `get` codec only reads path and query parameters. The
        // custom codec passed in here decodes the body instead, to demonstrate
        // `handle_with`.
        .route(
            pattern::routes::NEARBY,
            handle_with(
                MethodFilter::GET,
                nearby_places,
                |raw: RawRequest<'_>| decode_nearby(raw.body),
                encode_nearby,
            ),
        )
        // Capability-example routes: one domain-free handler each for
        // `BlobStore`, `Broadcast`, `DocumentStore`, and `TableStore`.
        .route(capability::routes::ARCHIVE, post(archive))
        .route(capability::routes::ALERT, post(alert))
        .route(capability::routes::NOTE, post(note))
        .route(capability::routes::READING, post(reading))
        // Docstore-example routes: the rich `wasi:docstore` showcase (full
        // CRUD and every filter type over GTFS-like collections).
        .route(docstore::paths::STOPS, get(list_stops).merge(post(create_stop)))
        .route(
            docstore::paths::STOP,
            get(get_stop).merge(put(upsert_stop)).merge(delete(delete_stop)),
        )
        .route(docstore::paths::ROUTES, get(list_routes).merge(post(create_route)))
        .route(docstore::paths::ROUTE, get(get_route))
        .route(docstore::paths::STOP_TIMES, get(list_stop_times).merge(post(create_stop_time)))
        .route(docstore::paths::STOP_TIME, get(get_stop_time))
        // SQL-example routes: the rich `wasi-sql` ORM showcase (agency/feed
        // schema with JOINs and server-assigned ids).
        .route(sql::paths::AGENCIES, get(list_agencies).merge(post(create_agency)))
        .route(sql::paths::AGENCY, get(get_agency).merge(patch(update_agency)))
        .route(sql::paths::AGENCY_FEEDS, get(list_agency_feeds).merge(post(create_feed)))
        .route(sql::paths::FEEDS, get(list_all_feeds))
        .route(sql::paths::FEED, delete(delete_feed));

    #[cfg(feature = "god-mode")]
    let router = router.route(routes::http::SET_TRIP, post(set_trip));

    router.with_state(Client::new(OWNER, provider))
}

/// Pass the Pulse body through undecoded.
///
/// The handler parses the SOAP envelope itself so a malformed body is
/// answered with the vendor's XML `<Fault>` (via the handler error's
/// `HttpError` conversion). A decoder that failed here would instead reach
/// the client as the framework's plain-text 400.
#[allow(clippy::unnecessary_wraps, reason = "the route codec requires a fallible decoder")]
fn decode_pulse(body: &[u8]) -> Result<PulseXml, DecodeError> {
    Ok(PulseXml(body.to_vec()))
}

/// Encode the Pulse acknowledgement in the vendor's XML shape.
fn encode_pulse(reply: &PulseReply) -> Response {
    match reply.to_xml() {
        Ok(xml) => ([(CONTENT_TYPE, "text/xml")], xml).into_response(),
        Err(error) => HttpError::from(error).into_response(),
    }
}

/// Decode the nearby request from a JSON body.
///
/// Demonstration only: this does exactly what the built-in `post` codec
/// does. It exists because this route is a GET, whose default codec reads
/// the query string, not the body. Routes with ordinary JSON bodies should
/// use `post(handler)` — no custom decoder needed.
fn decode_nearby(body: &[u8]) -> Result<NearbyPlacesRequest, DecodeError> {
    serde_json::from_slice(body)
        .map_err(|error| DecodeError::new(format!("malformed JSON body: {error}")))
}

/// Encode the nearby reply as JSON — identical to the built-in encoder.
///
/// Demonstration only: `handle_with` requires both halves of the codec, so
/// this supplies the same JSON encoding the default routes already use.
///
/// You could also use this technique to implement a custom decoding that is
/// not just a straight serialization.
fn encode_nearby(reply: NearbyPlacesReply) -> Response {
    Json(reply).into_response()
}

/// WASI Messaging export.
#[cfg(target_arch = "wasm32")]
pub struct Messaging;
#[cfg(target_arch = "wasm32")]
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

#[cfg(target_arch = "wasm32")]
impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        let router = messaging_router(Provider).await;
        messaging::handle(&router, message).await
    }
}

/// Build the exact-topic messaging router over one provider-owning [`Client`].
///
/// Topics are registered with their full `{env}-` qualified names, so a
/// topic from another environment is rejected as unhandled instead of
/// silently consumed. Router failures — including handler errors with their
/// structured codes — flow back as `error.other` with the full display
/// string, since the WIT contract only carries a string. Resolving the
/// environment reads `Config`, hence the `async`.
pub async fn messaging_router<P>(provider: P) -> messaging::Router<P>
where
    P: Config + HttpRequest + Identity + Publish + StateStore + Send + Sync + 'static,
{
    let env = config::env(&provider).await;
    messaging::Router::new(Client::new(OWNER, provider))
        .route(
            config::topic_for(&env, routes::topic::PULSE),
            consume_with(pulse_adapter::pulse, decode_pulse_xml),
        )
        .route(config::topic_for(&env, routes::topic::PULSE_TO_MOTION), consume(motion))
        .route(config::topic_for(&env, routes::topic::TRAIN_AVL), consume(train_avl))
        .route(config::topic_for(&env, routes::topic::PASSENGER_COUNT), consume(passenger_count))
}

/// Decode an inbound Pulse train update from its raw XML payload.
fn decode_pulse_xml(delivery: &Delivery) -> Result<PulseMessage, DecodeError> {
    PulseMessage::from_xml(&delivery.payload).map_err(|error| DecodeError::new(error.to_string()))
}
