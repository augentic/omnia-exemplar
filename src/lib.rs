//! # Root-package guest
//!
//! Wires the shared transit handlers to WASI HTTP and WASI Messaging with
//! the explicit typed routers from `omnia_guest::api`: HTTP routes are
//! `axum::routing::MethodRouter`s over a provider-owning `Client`, and
//! messaging topics dispatch through an exact-topic `messaging::Router`.
//! Routes that speak JSON use the default `get` / `post` / `consume`
//! codecs; the Pulse SOAP/XML routes supply their own.
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
use capability_examples::{AlertRequest, ArchiveRequest, NoteRequest, ReadingRequest};
use docstore_examples::{
    CreateRouteRequest, CreateStopRequest, CreateStopTimeRequest, DeleteStopRequest,
    GetRouteRequest, GetStopRequest, GetStopTimeRequest, ListRoutesRequest, ListStopTimesRequest,
    ListStopsRequest, UpsertStopRequest,
};
#[cfg(feature = "god-mode")]
use gtfs_adapter::SetTripRequest;
use gtfs_adapter::{MotionMessage, PassengerCountMessage, TrainAvlMessage, VehicleInfoRequest};
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
use pattern_examples::{
    DecodeSegmentRequest, NearbyPlacesReply, NearbyPlacesRequest, UpsertPlaceRequest,
};
use pulse_adapter::PulseMessage;
use pulse_connector::{PulseReply, PulseXml};
use sql_examples::{
    CreateAgencyRequest, CreateFeedRequest, DeleteFeedRequest, GetAgencyRequest,
    ListAgenciesRequest, ListAgencyFeedsRequest, ListAllFeedsRequest, UpdateAgencyRequest,
};
use tally_connector::TallyRequest;
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
        .route(routes::http::APC, post::<TallyRequest, P>())
        .route(
            routes::http::PULSE_XML,
            handle_with(
                MethodFilter::POST,
                |raw: RawRequest<'_>| decode_pulse(raw.body),
                |reply| encode_pulse(&reply),
            ),
        )
        .route(routes::http::VEHICLE_INFO, get::<VehicleInfoRequest, P>())
        // Pattern-example routes, outside the canonical transit tables.
        .route(pattern_examples::routes::DECODE, post::<DecodeSegmentRequest, P>())
        .route(pattern_examples::routes::PLACES, post::<UpsertPlaceRequest, P>())
        // The default `get` codec only reads path and query parameters. The
        // custom codec passed in here decodes the body instead, to demonstrate
        // `handle_with`.
        .route(
            pattern_examples::routes::NEARBY,
            handle_with(
                MethodFilter::GET,
                |raw: RawRequest<'_>| decode_nearby(raw.body),
                encode_nearby,
            ),
        )
        // Capability-example routes: one domain-free handler each for
        // `BlobStore`, `Broadcast`, `DocumentStore`, and `TableStore`.
        .route(capability_examples::routes::ARCHIVE, post::<ArchiveRequest, P>())
        .route(capability_examples::routes::ALERT, post::<AlertRequest, P>())
        .route(capability_examples::routes::NOTE, post::<NoteRequest, P>())
        .route(capability_examples::routes::READING, post::<ReadingRequest, P>())
        // Docstore-example routes: the rich `wasi:docstore` showcase (full
        // CRUD and every filter type over GTFS-like collections).
        .route(
            docstore_examples::paths::STOPS,
            get::<ListStopsRequest, P>().merge(post::<CreateStopRequest, P>()),
        )
        .route(
            docstore_examples::paths::STOP,
            get::<GetStopRequest, P>()
                .merge(put::<UpsertStopRequest, P>())
                .merge(delete::<DeleteStopRequest, P>()),
        )
        .route(
            docstore_examples::paths::ROUTES,
            get::<ListRoutesRequest, P>().merge(post::<CreateRouteRequest, P>()),
        )
        .route(docstore_examples::paths::ROUTE, get::<GetRouteRequest, P>())
        .route(
            docstore_examples::paths::STOP_TIMES,
            get::<ListStopTimesRequest, P>().merge(post::<CreateStopTimeRequest, P>()),
        )
        .route(docstore_examples::paths::STOP_TIME, get::<GetStopTimeRequest, P>())
        // SQL-example routes: the rich `wasi-sql` ORM showcase (agency/feed
        // schema with JOINs and server-assigned ids).
        .route(
            sql_examples::paths::AGENCIES,
            get::<ListAgenciesRequest, P>().merge(post::<CreateAgencyRequest, P>()),
        )
        .route(
            sql_examples::paths::AGENCY,
            get::<GetAgencyRequest, P>().merge(patch::<UpdateAgencyRequest, P>()),
        )
        .route(
            sql_examples::paths::AGENCY_FEEDS,
            get::<ListAgencyFeedsRequest, P>().merge(post::<CreateFeedRequest, P>()),
        )
        .route(sql_examples::paths::FEEDS, get::<ListAllFeedsRequest, P>())
        .route(sql_examples::paths::FEED, delete::<DeleteFeedRequest, P>());

    #[cfg(feature = "god-mode")]
    let router = router.route(routes::http::SET_TRIP, post::<SetTripRequest, P>());

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
/// use `post::<Input, Provider>()` — no custom decoder needed.
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
        .route(config::topic_for(&env, routes::topic::PULSE), consume_with(decode_pulse_xml))
        .route(config::topic_for(&env, routes::topic::PULSE_TO_MOTION), consume::<MotionMessage>())
        .route(config::topic_for(&env, routes::topic::TRAIN_AVL), consume::<TrainAvlMessage>())
        .route(
            config::topic_for(&env, routes::topic::PASSENGER_COUNT),
            consume::<PassengerCountMessage>(),
        )
}

/// Decode an inbound Pulse train update from its raw XML payload.
fn decode_pulse_xml(delivery: &Delivery) -> Result<PulseMessage, DecodeError> {
    PulseMessage::from_xml(&delivery.payload).map_err(|error| DecodeError::new(error.to_string()))
}
