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

#![cfg(target_arch = "wasm32")]

use acme_common::{config, routes};
use axum::Json;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
#[cfg(feature = "god-mode")]
use gtfs_adapter::SetTripRequest;
use gtfs_adapter::{MotionMessage, PassengerCountMessage, TrainAvlMessage, VehicleInfoRequest};
use omnia_guest::HttpError;
use omnia_guest::api::http::{RawRequest, get, get_with, post, post_with, serve};
use omnia_guest::api::messaging::{self, Delivery, consume, consume_with};
use omnia_guest::api::{Client, DecodeError};
use omnia_wasi_messaging::types::{Error, Message};
use pattern_examples::{
    DecodeSegmentRequest, NearbyPlacesReply, NearbyPlacesRequest, UpsertPlaceRequest,
};
use pulse_adapter::PulseMessage;
use pulse_connector::{PulseReply, PulseXml};
use tally_connector::TallyRequest;
use tracing::Level;
use wasip3::exports::http::handler::Guest;
use wasip3::http::types as p3;

/// The tenant that owns this deployment.
const OWNER: &str = "acme";

omnia_guest::provider! {
    /// Bare provider backed by the default WASI capability implementations.
    pub struct Provider: Config + HttpRequest + Identity + Publish + StateStore + TableStore;
}

/// WASI HTTP export.
pub struct Http;
wasip3::http::service::export!(Http);

impl Guest for Http {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
    async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
        serve(router(), request).await
    }
}

/// Build the HTTP router over one provider-owning [`Client`].
///
/// Omnia creates one WASI component instance per HTTP request, so the router
/// and client are constructed inside each `handle` call; axum's route-state
/// clones share the client's provider allocation for that request only.
fn router() -> axum::Router {
    let router = axum::Router::new()
        .route(routes::http::APC, post::<TallyRequest, Provider>())
        .route(
            routes::http::PULSE_XML,
            post_with(|raw: RawRequest<'_>| decode_pulse(raw.body), |reply| encode_pulse(&reply)),
        )
        .route(routes::http::VEHICLE_INFO, get::<VehicleInfoRequest, Provider>())
        // Pattern-example routes, outside the canonical transit tables.
        .route(pattern_examples::routes::DECODE, post::<DecodeSegmentRequest, Provider>())
        .route(pattern_examples::routes::PLACES, post::<UpsertPlaceRequest, Provider>())
        // The default `get` codec only reads path and query parameters. The
        // custom codec passed in here decodes the body instead, to demonstrate
        // `get_with`.
        .route(
            pattern_examples::routes::NEARBY,
            get_with(|raw: RawRequest<'_>| decode_nearby(raw.body), encode_nearby),
        );

    #[cfg(feature = "god-mode")]
    let router = router.route(routes::http::SET_TRIP, post::<SetTripRequest, Provider>());

    router.with_state(Client::new(OWNER, Provider))
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
/// Demonstration only: `get_with` requires both halves of the codec, so
/// this supplies the same JSON encoding the default routes already use.
///
/// You could also use this technique to implement a custom decoding that is
/// not just a straight serialization.
fn encode_nearby(reply: NearbyPlacesReply) -> Response {
    Json(reply).into_response()
}

/// WASI Messaging export.
pub struct Messaging;
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        let router = messaging_router().await;
        messaging::handle(&router, message).await
    }
}

/// Build the exact-topic messaging router.
///
/// Topics are registered with their full `{env}-` qualified names, so a
/// topic from another environment is rejected as unhandled instead of
/// silently consumed. Router failures — including handler errors with their
/// structured codes — flow back as `error.other` with the full display
/// string, since the WIT contract only carries a string.
async fn messaging_router() -> messaging::Router<Provider> {
    let env = config::env(&Provider).await;
    messaging::Router::new(Client::new(OWNER, Provider))
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
