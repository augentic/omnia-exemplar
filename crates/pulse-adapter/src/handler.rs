//! Pulse Position Adapter
//!
//! Transform a Pulse XML message into Motion events.

use anyhow::Context as _;
use axum::body::Body;
use chrono::Utc;
use common::topic;
use http::Method;
use http::header::AUTHORIZATION;
use omnia_sdk::api::messaging::Delivery;
use omnia_sdk::api::{Context, DecodeError, Handlers};
use omnia_sdk::{
    Capabilities, Config, Error, HttpRequest, HttpRequestExt as _, Identity, IdentityExt, Message,
    Publish, Result, Settings, StateStore,
};
use serde::Deserialize;
use thiserror::Error;

use crate::motion::{EventType, MessageData, MotionEvent, RemoteData};
use crate::pulse::TrainUpdate;
use crate::stops;

/// The settings the Pulse handler reads.
#[derive(Debug, Settings)]
pub struct PulseSettings {
    // Base URL of the Block Management API.
    block_mgt_url: String,
    // Identity used to acquire access tokens for the operator APIs.
    api_identity: String,
}

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().topic_with(topic::PULSE, pulse, decode_pulse_xml)
}

fn decode_pulse_xml(delivery: &Delivery) -> std::result::Result<PulseMessage, DecodeError> {
    PulseMessage::from_xml(&delivery.payload).map_err(|error| DecodeError::new(error.to_string()))
}

/// Errors raised while validating an inbound Pulse train update message.
///
/// This type covers the train update consumed from messaging.
#[derive(Error, Debug)]
pub enum PulseMessageError {
    /// The message timestamp is invalid (too old or future-dated).
    #[error("{0}")]
    BadTime(String),

    /// The message contains no updates or the arrival/departure time is
    /// invalid (negative or 0).
    #[error("{0}")]
    NoUpdate(String),

    /// The XML is invalid.
    #[error("{0}")]
    InvalidXml(String),
}

impl PulseMessageError {
    fn code(&self) -> String {
        match self {
            Self::BadTime(_) => "bad_time".to_string(),
            Self::NoUpdate(_) => "no_update".to_string(),
            Self::InvalidXml(_) => "invalid_message".to_string(),
        }
    }
}

impl From<quick_xml::DeError> for PulseMessageError {
    fn from(err: quick_xml::DeError) -> Self {
        Self::InvalidXml(err.to_string())
    }
}

impl From<PulseMessageError> for Error {
    fn from(err: PulseMessageError) -> Self {
        Self::bad_request(err.code(), err.to_string())
    }
}

/// Pulse train update message as deserialized from the XML received from the
/// track-side sensor network.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PulseMessage {
    /// The train update.
    #[serde(rename(deserialize = "ActualizarDatosTren"))]
    pub train_update: TrainUpdate,
}

impl PulseMessage {
    /// Deserialize a Pulse message from raw XML.
    ///
    /// # Errors
    ///
    /// Returns an error if the XML cannot be deserialized.
    pub fn from_xml(input: &[u8]) -> Result<Self> {
        quick_xml::de::from_reader(input).context("deserializing PulseMessage").map_err(Into::into)
    }
}

/// Each Motion event is published this many times: the downstream schedule
/// adherence process treats the repeated location event as confirmation that
/// the train has departed the station, so a single publish would leave
/// departures unsignalled.
///
/// The legacy system spaced the repeats five seconds apart with a blocking
/// `thread::sleep`. Do not copy that pattern — blocking stalls the entire
/// WASM guest. If spacing between publishes ever matters, use a scheduled or
/// delayed message, or debounce in the consumer, instead of sleeping in the
/// handler.
const PUBLISH_REPEATS: usize = 2;

pub(crate) async fn pulse<P>(context: Context<P>, input: PulseMessage) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish + StateStore,
{
    // validate message
    let update = input.train_update;
    update.validate()?;

    // convert to Motion events
    let events = update.into_events(context.provider()).await?;

    // publish events to Motion topic (repeated — see `PUBLISH_REPEATS`)
    for _ in 0..PUBLISH_REPEATS {
        for event in &events {
            tracing::info!(monotonic_counter.motion_events_published = 1);

            let message =
                Message::json(event)?.with_header("key", event.remote_data.external_id.clone());
            context.publish().send(topic::PULSE_TO_MOTION, &message).await?;
        }
    }

    Ok(())
}

impl TrainUpdate {
    /// Transform the Pulse message to Motion events
    async fn into_events<P>(self, provider: &P) -> Result<Vec<MotionEvent>>
    where
        P: Config + HttpRequest + Identity + Publish + StateStore,
    {
        let changes = &self.changes;
        let change_type = changes[0].r#type;

        // filter out irrelevant updates (not related to trip progress)
        if !change_type.is_relevant() {
            tracing::info!(monotonic_counter.irrelevant_change_type = 1, type = %change_type);
            return Ok(vec![]);
        }

        // is station relevant?
        let station = changes[0].station;
        let Some(stop_info) = stops::stop_info(provider, station, change_type.is_arrival()).await?
        else {
            tracing::info!(monotonic_counter.irrelevant_station = 1, station = %station);
            return Ok(vec![]);
        };

        // get train allocations for this trip
        let allocated = trip_allocations(provider, &self.train_id()).await?;

        // publish Motion events
        let mut events = Vec::new();
        for train in allocated {
            events.push(MotionEvent {
                received_at: Utc::now(),
                event_type: EventType::Location,
                message_data: MessageData::default(),
                remote_data: RemoteData {
                    external_id: train.replace(' ', ""),
                    ..RemoteData::default()
                },
                location_data: stop_info.clone().into(),
                ..MotionEvent::default()
            });
        }

        Ok(events)
    }
}

async fn trip_allocations<P>(provider: &P, external_ref_id: &str) -> Result<Vec<String>>
where
    P: Config + HttpRequest + Identity,
{
    let PulseSettings {
        block_mgt_url: url,
        api_identity: identity,
    } = PulseSettings::load(provider).await?;
    let token = IdentityExt::access_token(provider, &identity).await?;

    let request = http::Request::builder()
        .method(Method::GET)
        .uri(format!("{url}/allocations/trips?externalRefId={external_ref_id}"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .context("building trip allocations request")?;

    provider.fetch_json(request).await
}
