//! # Typed guest (style A)
//!
//! Wires the shared transit operations to WASI HTTP and WASI Messaging using
//! the typed `omnia_guest::api` routers. Each route binds an [`Operation`]
//! directly; transport decoding, invocation, and response projection are
//! handled by the router.
//!
//! Routes and topics come from the canonical tables in
//! [`acme_common::routes`], shared with `guests/axum` (style B), which serves
//! the same surface with hand-written Axum handlers.
//!
//! [`Operation`]: omnia_guest::api::Operation

#![cfg(target_arch = "wasm32")]

use acme_common::{config, routes};
use axum::body::Bytes;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use capability_examples::{
    AlertRequest, ArchiveRequest, NoteRequest, ReadingRequest, routes as example_routes,
};
#[cfg(feature = "god-mode")]
use gtfs_adapter::SetTripRequest;
use gtfs_adapter::{MotionMessage, PassengerCountMessage, TrainAvlMessage, VehicleInfoRequest};
use omnia_guest::Error;
use omnia_guest::api::http::{HttpError, Router, get, post};
use omnia_guest::api::messaging::{self, Delivery, consume};
use omnia_guest::api::{Invocation, Invoker};
use pulse_adapter::PulseMessage;
use pulse_connector::PulseRequest;
use tally_connector::TallyRequest;
use tracing::Level;
use wasip3::exports::http::handler::Guest;
use wasip3::http::types as p3;

/// The tenant that owns this deployment.
const OWNER: &str = "acme";

/// Bare provider backed by the default WASI capability implementations.
#[derive(Clone)]
pub struct Provider;

impl omnia_guest::BlobStore for Provider {}
impl omnia_guest::Broadcast for Provider {}
impl omnia_guest::Config for Provider {}
impl omnia_guest::DocumentStore for Provider {}
impl omnia_guest::HttpRequest for Provider {}
impl omnia_guest::Identity for Provider {}
impl omnia_guest::Publish for Provider {}
impl omnia_guest::StateStore for Provider {}
impl omnia_guest::TableStore for Provider {}

/// WASI HTTP export.
pub struct Http;
wasip3::http::service::export!(Http);

impl Guest for Http {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
    async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
        let router = Router::new(Invoker::new(OWNER, Provider))
            .route(routes::http::APC, post::<TallyRequest, Provider>())
            .route(routes::http::VEHICLE_INFO, get::<VehicleInfoRequest, Provider>())
            // Capability-example routes (typed guest only): they instantiate
            // the default WASM BlobStore / Broadcast / DocumentStore /
            // TableStore implementations in a real guest, outside the
            // canonical transit tables both guest styles share.
            .route(example_routes::ARCHIVE, post::<ArchiveRequest, Provider>())
            .route(example_routes::ALERT, post::<AlertRequest, Provider>())
            .route(example_routes::NOTE, post::<NoteRequest, Provider>())
            .route(example_routes::READING, post::<ReadingRequest, Provider>());

        #[cfg(feature = "god-mode")]
        let router = router.route(routes::http::SET_TRIP, post::<SetTripRequest, Provider>());

        // The Pulse ingress is SOAP/XML; that sits outside the JSON-typed
        // router, so drop down to the underlying Axum router for this route.
        let router =
            router.into_axum().route(routes::http::PULSE_XML, axum::routing::post(pulse_message));

        omnia_wasi_http::serve(router, request).await
    }
}

/// Decode a Pulse SOAP envelope, invoke the operation, and reply in XML.
async fn pulse_message(body: Bytes) -> Response {
    let request = match PulseRequest::from_xml(&body) {
        Ok(request) => request,
        Err(error) => return HttpError::from(error).into_response(),
    };

    let invoker = Invoker::new(OWNER, Provider);
    let reply = match invoker.invoke::<PulseRequest>(Invocation::new(request)).await {
        Ok(reply) => reply,
        Err(error) => return HttpError::from(error).into_response(),
    };

    match reply.to_xml() {
        Ok(xml) => ([(CONTENT_TYPE, "text/xml")], xml).into_response(),
        Err(error) => HttpError::from(error).into_response(),
    }
}

/// WASI Messaging export.
pub struct Messaging;
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(
        message: omnia_wasi_messaging::types::Message,
    ) -> Result<(), omnia_wasi_messaging::types::Error> {
        let env = config::env(&Provider).await;

        let router = messaging::Router::new(Invoker::new(OWNER, Provider))
            .route(
                config::topic_for(&env, routes::topic::PULSE),
                consume::<PulseMessage>().decode_with(decode_pulse_xml),
            )
            .route(
                config::topic_for(&env, routes::topic::PULSE_TO_MOTION),
                consume::<MotionMessage>(),
            )
            .route(config::topic_for(&env, routes::topic::TRAIN_AVL), consume::<TrainAvlMessage>())
            .route(
                config::topic_for(&env, routes::topic::PASSENGER_COUNT),
                consume::<PassengerCountMessage>(),
            );

        messaging::handle(&router, message).await
    }
}

/// Decode a Pulse XML payload into the operation input.
fn decode_pulse_xml(delivery: &Delivery) -> Result<PulseMessage, Error> {
    PulseMessage::from_xml(&delivery.payload)
}
