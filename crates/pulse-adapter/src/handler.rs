//! Pulse Position Adapter
//!
//! Transform a Pulse XML message into Motion events.

use acme_common::{block_mgt, config, routes};
use anyhow::Context as _;
use chrono::Utc;
use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Config, Error, HttpRequest, Identity, Message, Publish, Result};
use serde::Deserialize;

use crate::motion::{EventType, MessageData, MotionEvent, RemoteData};
use crate::pulse::TrainUpdate;
use crate::stops;

/// Each Motion event is published this many times: the downstream schedule
/// adherence process treats the repeated location event as confirmation that
/// the train has departed the station, so a single publish would leave
/// departures unsignalled.
///
/// The legacy system spaced the repeats five seconds apart with a blocking
/// `thread::sleep`. Do not copy that pattern — blocking stalls the entire
/// WASM guest. If spacing between publishes ever matters, use a scheduled or
/// delayed message, or debounce in the consumer, instead of sleeping in the
/// operation.
const PUBLISH_REPEATS: usize = 2;

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

impl<P> Operation<P> for PulseMessage
where
    P: Provider + Config + HttpRequest + Identity + Publish,
{
    type Error = Error;
    type Input = Self;
    type Output = ();

    #[tracing::instrument(
        name = "pulse_message",
        skip_all,
        fields(
            owner = context.owner,
            vehicle_id = input.train_update.train_id(),
            topic = routes::topic::PULSE,
        ),
    )]
    async fn call(input: Self, context: CallContext<'_, P>) -> Result<()> {
        let provider = context.provider;

        // validate message
        let update = input.train_update;
        update.validate()?;

        // convert to Motion events
        let events = update.into_events(context.owner, provider).await?;

        // publish events to Motion topic (repeated — see `PUBLISH_REPEATS`)
        let topic = config::topic(provider, routes::topic::PULSE_TO_MOTION).await;

        for _ in 0..PUBLISH_REPEATS {
            for event in &events {
                tracing::info!(monotonic_counter.motion_events_published = 1);

                let payload = serde_json::to_vec(&event).context("serializing event")?;
                let external_id = &event.remote_data.external_id;

                let mut message = Message::new(&payload);
                message.headers.insert("key".to_string(), external_id.clone());

                Publish::send(provider, &topic, &message).await?;
            }
        }

        Ok(())
    }
}

impl TrainUpdate {
    /// Transform the Pulse message to Motion events
    async fn into_events<P>(self, owner: &str, provider: &P) -> Result<Vec<MotionEvent>>
    where
        P: Config + HttpRequest + Identity + Publish,
    {
        let changes = &self.changes;
        let change_type = changes[0].r#type;

        // filter out irrelevant updates (not related to trip progress)
        if !change_type.is_relevant() {
            tracing::info!(monotonic_counter.irrelevant_change_type = 1, type = %change_type);
            return Ok(vec![]);
        }

        // is station is relevant?
        let station = changes[0].station;
        let Some(stop_info) =
            stops::stop_info(owner, provider, station, change_type.is_arrival()).await?
        else {
            tracing::info!(monotonic_counter.irrelevant_station = 1, station = %station);
            return Ok(vec![]);
        };

        // get train allocations for this trip
        let allocated = block_mgt::trip_allocations(&self.train_id(), provider)
            .await
            .context("fetching train allocations")?;

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

#[cfg(test)]
mod tests {
    use super::PulseMessage;

    #[test]
    fn deserialization() {
        let xml = include_str!("../data/sample.xml");
        let message: PulseMessage = quick_xml::de::from_str(xml).expect("should deserialize");

        let update = message.train_update;
        assert_eq!(update.even_train_id, Some("1234".to_string()));
        assert!(!update.changes.is_empty(), "should have changes");
    }
}
