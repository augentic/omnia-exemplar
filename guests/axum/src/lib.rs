//! # Axum guest (style B)
//!
//! Wires the shared transit operations to WASI HTTP and WASI Messaging with
//! hand-written Axum handlers served through [`omnia_wasi_http::serve`], and
//! a raw `incoming-handler` messaging export. Each handler decodes the
//! transport payload itself and invokes the shared operation through an
//! [`Invoker`].
//!
//! Compare with `guests/typed` (style A), which serves the same routes and
//! topics through the typed `omnia_guest::api` routers.

#![cfg(target_arch = "wasm32")]

use anyhow::{Context, Result};
use axum::extract::Path;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use gtfs_adapter::{
    MotionMessage, PassengerCountMessage, SetTripReply, SetTripRequest, TrainAvlMessage,
    VehicleInfoReply, VehicleInfoRequest,
};
use omnia_guest::api::{Invocation, Invoker};
use omnia_guest::{HttpError, HttpResult};
use omnia_wasi_messaging::types::{Error, Message};
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
            .route("/api/apc", post(tally_message))
            .route("/inbound/xml", post(pulse_message))
            .route("/info/{vehicle_id}", get(vehicle_info))
            .route("/god-mode/set-trip/{vehicle_id}/{trip_id}", get(set_trip));
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

async fn set_trip(
    Path((vehicle_id, trip_id)): Path<(String, String)>,
) -> HttpResult<Json<SetTripReply>> {
    let request = SetTripRequest { vehicle_id, trip_id };
    let reply = invoker().invoke::<SetTripRequest>(Invocation::new(request)).await?;
    Ok(Json(reply))
}

/// WASI Messaging export.
pub struct Messaging;
omnia_wasi_messaging::export!(Messaging with_types_in omnia_wasi_messaging);

impl omnia_wasi_messaging::incoming_handler::Guest for Messaging {
    #[omnia_wasi_otel::instrument(name = "messaging_guest_handle")]
    async fn handle(message: Message) -> Result<(), Error> {
        if let Err(e) = match &message.topic().unwrap_or_default() {
            t if t.contains("realtime-pulse.v1") => pulse(message.data()).await,
            t if t.contains("realtime-pulse-to-motion.v1") => motion(message.data()).await,
            t if t.contains("realtime-train-avl.v1") => train_avl(message.data()).await,
            t if t.contains("realtime-passenger-count.v1") => passenger_count(message.data()).await,
            _ => {
                return Err(Error::Other("Unhandled topic".to_string()));
            }
        } {
            return Err(Error::Other(e.to_string()));
        }
        Ok(())
    }
}

#[omnia_wasi_otel::instrument]
async fn pulse(payload: Vec<u8>) -> Result<()> {
    let message = PulseMessage::from_xml(&payload)?;
    invoker().invoke::<PulseMessage>(Invocation::new(message)).await.map_err(Into::into)
}

#[omnia_wasi_otel::instrument]
async fn motion(payload: Vec<u8>) -> Result<()> {
    let message: MotionMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<MotionMessage>(Invocation::new(message)).await.map_err(Into::into)
}

#[omnia_wasi_otel::instrument]
async fn train_avl(payload: Vec<u8>) -> Result<()> {
    let message: TrainAvlMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<TrainAvlMessage>(Invocation::new(message)).await.map_err(Into::into)
}

#[omnia_wasi_otel::instrument]
async fn passenger_count(payload: Vec<u8>) -> Result<()> {
    let message: PassengerCountMessage = serde_json::from_slice(&payload)?;
    invoker().invoke::<PassengerCountMessage>(Invocation::new(message)).await.map_err(Into::into)
}
