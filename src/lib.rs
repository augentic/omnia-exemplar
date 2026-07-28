//! # Root-package Axum guest
//!
//! Wires the shared transit operations to WASI HTTP and WASI Messaging with
//! hand-written Axum handlers served through `omnia_wasi_http::serve`, and a
//! raw `incoming-handler` messaging export. Each handler decodes the
//! transport payload itself and invokes the shared operation through an
//! `Invoker`.
//!
//! This root-package layout (`src/lib.rs`) is the compiling reference for
//! new Omnia services. Routes and topics come from the canonical tables in
//! [`acme_common::routes`].

#![cfg(target_arch = "wasm32")]

use acme_common::{config, routes};
use anyhow::Context;
use axum::extract::Path;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use gtfs_adapter::{
    MotionMessage, PassengerCountMessage, TrainAvlMessage, VehicleInfoReply, VehicleInfoRequest,
};
#[cfg(feature = "god-mode")]
use gtfs_adapter::{SetTripReply, SetTripRequest};
use omnia_guest::api::{Invocation, Invoker};
use omnia_guest::{HttpError, HttpResult};
use omnia_wasi_messaging::types::{Error, Message};
use pattern_examples::{
    DecodeSegmentReply, DecodeSegmentRequest, NearbyPlacesReply, NearbyPlacesRequest,
    UpsertPlaceReply, UpsertPlaceRequest,
};
use pulse_adapter::PulseMessage;
use pulse_connector::PulseRequest;
use tally_connector::{TallyReply, TallyRequest};
use tracing::Level;
use wasip3::exports::http::handler::Guest;
use wasip3::http::types as p3;

/// The tenant that owns this deployment.
const OWNER: &str = "acme";

/// Bare provider backed by the default WASI capability implementations.
#[derive(Clone)]
pub struct Provider;

impl omnia_guest::Config for Provider {}
impl omnia_guest::HttpRequest for Provider {}
impl omnia_guest::Identity for Provider {}
impl omnia_guest::Publish for Provider {}
impl omnia_guest::StateStore for Provider {}
impl omnia_guest::TableStore for Provider {}

fn invoker() -> Invoker<Provider> {
    Invoker::new(OWNER, Provider)
}

/// WASI HTTP export.
pub struct Http;
wasip3::http::service::export!(Http);

impl Guest for Http {
    #[omnia_wasi_otel::instrument(name = "http_guest_handle", level = Level::INFO)]
    async fn handle(request: p3::Request) -> Result<p3::Response, p3::ErrorCode> {
        let router = Router::new()
            .route(routes::http::APC, post(tally_message))
            .route(routes::http::PULSE_XML, post(pulse_message))
            .route(routes::http::VEHICLE_INFO, get(vehicle_info))
            // Pattern-example routes, outside the canonical transit tables.
            .route(pattern_examples::routes::DECODE, post(decode_segment))
            .route(pattern_examples::routes::PLACES, post(upsert_place))
            .route(pattern_examples::routes::NEARBY, get(nearby_places));

        #[cfg(feature = "god-mode")]
        let router = router.route(routes::http::SET_TRIP, post(set_trip));

        omnia_wasi_http::serve(router, request).await
    }
}

async fn tally_message(Json(request): Json<TallyRequest>) -> HttpResult<Json<TallyReply>> {
    let reply = invoker().invoke::<TallyRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

async fn pulse_message(body: Bytes) -> HttpResult<Response> {
    let request = PulseRequest::from_xml(&body).map_err(HttpError::from)?;
    let reply = invoker().invoke::<PulseRequest>(Invocation::new(request)).await?;
    let xml = reply.to_xml().context("serializing reply")?;
    Ok(([(CONTENT_TYPE, "text/xml")], xml).into_response())
}

async fn vehicle_info(Path(vehicle_id): Path<String>) -> HttpResult<Json<VehicleInfoReply>> {
    let request = VehicleInfoRequest { vehicle_id };
    let reply = invoker().invoke::<VehicleInfoRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

#[cfg(feature = "god-mode")]
async fn set_trip(
    Path((vehicle_id, trip_id)): Path<(String, String)>,
) -> HttpResult<Json<SetTripReply>> {
    let request = SetTripRequest { vehicle_id, trip_id };
    let reply = invoker().invoke::<SetTripRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

async fn decode_segment(
    Json(request): Json<DecodeSegmentRequest>,
) -> HttpResult<Json<DecodeSegmentReply>> {
    let reply = invoker().invoke::<DecodeSegmentRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

async fn upsert_place(
    Json(request): Json<UpsertPlaceRequest>,
) -> HttpResult<Json<UpsertPlaceReply>> {
    let reply = invoker().invoke::<UpsertPlaceRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

async fn nearby_places(
    Json(request): Json<NearbyPlacesRequest>,
) -> HttpResult<Json<NearbyPlacesReply>> {
    let reply = invoker().invoke::<NearbyPlacesRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

/// WASI Messaging export.
pub struct Messaging;
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        let Some(topic) = message.topic() else {
            return Err(Error::Other("missing topic".to_string()));
        };

        // Match the exact `{env}-` qualified topics — the same names the
        // typed guest registers — rather than a substring, so a topic from
        // another environment is rejected instead of silently consumed.
        let env = config::env(&Provider).await;
        let result = match &topic {
            t if *t == config::topic_for(&env, routes::topic::PULSE) => pulse(message.data()).await,
            t if *t == config::topic_for(&env, routes::topic::PULSE_TO_MOTION) => {
                motion(message.data()).await
            }
            t if *t == config::topic_for(&env, routes::topic::TRAIN_AVL) => {
                train_avl(message.data()).await
            }
            t if *t == config::topic_for(&env, routes::topic::PASSENGER_COUNT) => {
                passenger_count(message.data()).await
            }
            _ => return Err(Error::Other(format!("unhandled topic: {topic}"))),
        };

        // The WIT contract only carries a string, so forward the domain
        // error's full display — including its structured code — instead of
        // discarding it behind a generic message.
        result.map_err(|error| Error::Other(error.to_string()))
    }
}

#[omnia_wasi_otel::instrument]
async fn pulse(payload: Vec<u8>) -> omnia_guest::Result<()> {
    let message = PulseMessage::from_xml(&payload)?;
    invoker().invoke::<PulseMessage>(Invocation::new(message)).await
}

#[omnia_wasi_otel::instrument]
async fn motion(payload: Vec<u8>) -> omnia_guest::Result<()> {
    let message: MotionMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<MotionMessage>(Invocation::new(message)).await
}

#[omnia_wasi_otel::instrument]
async fn train_avl(payload: Vec<u8>) -> omnia_guest::Result<()> {
    let message: TrainAvlMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<TrainAvlMessage>(Invocation::new(message)).await
}

#[omnia_wasi_otel::instrument]
async fn passenger_count(payload: Vec<u8>) -> omnia_guest::Result<()> {
    let message: PassengerCountMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<PassengerCountMessage>(Invocation::new(message)).await
}
