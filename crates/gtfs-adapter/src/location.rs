//! Location event processing.

use acme_common::TIMEZONE;
use acme_common::block_mgt::{self, BlockInstance};
use acme_common::fleet::{self, Vehicle};
use anyhow::Context as _;
use chrono::{Duration, NaiveDate, TimeZone};
use chrono_tz::Tz;
use omnia_guest::{Config, HttpRequest, Identity, Result, StateStore};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::trip::{
    self, DeadReckoningMessage, FeedEntity, Position, PositionDr, TripDescriptor, TripInstance,
    VehicleDescriptor, VehicleDr, VehiclePosition,
};
use crate::{EventType, MotionMessage, state_keys};

const TTL_TRIP_TRAIN: Duration = Duration::seconds(3 * 60 * 60);
const TTL_SIGN_ON: Duration = Duration::seconds(24 * 60 * 60);

const fn duration_secs(duration: Duration) -> u64 {
    duration.num_seconds().unsigned_abs()
}

pub enum Location {
    VehiclePosition(FeedEntity),
    DeadReckoning(DeadReckoningMessage),
}

/// Processes a Motion location payload and emits outbound messages when applicable.
///
/// # Errors
///
/// Returns an error when the incoming payload cannot be parsed or when domain logic
/// encounters an unrecoverable condition.
pub async fn process<P>(message: &MotionMessage, provider: &P) -> Result<Option<Location>>
where
    P: Config + HttpRequest + Identity + StateStore,
{
    // check for location event
    if message.event_type != EventType::Location {
        tracing::debug!("unsupported request type: {:?}", message.event_type);
        return Ok(None);
    }

    let location = &message.location_data;

    if message.remote_data.is_none() || location.gps_accuracy < 0.0 {
        tracing::debug!("invalid location event");
        return Ok(None);
    }

    // get vehicle info
    let Some(vehicle_id) = message.vehicle_id() else {
        tracing::debug!("no vehicle identifier found");
        return Ok(None);
    };
    let Some(vehicle) = fleet::vehicle(vehicle_id, provider).await? else {
        tracing::debug!("vehicle info not found for {vehicle_id}");
        return Ok(None);
    };

    let timestamp = message.timestamp()?;

    if vehicle.is_train() {
        let allocation = block_mgt::cached_allocation(&vehicle.id, timestamp, provider).await?;
        allocate(&vehicle, allocation, timestamp, provider).await?;
    }
    let trip_inst = current_trip(provider, &vehicle.id, timestamp).await?;
    let trip_desc = trip_inst.as_ref().map(TripDescriptor::from);
    let odometer = location.odometer.or(message.event_data.odometer);

    if (location.latitude.is_none() || location.longitude.is_none())
        && let (Some(odometer), Some(descriptor)) = (odometer, trip_desc.clone())
    {
        let dr_message = DeadReckoningMessage {
            id: Uuid::new_v4().to_string(),
            received_at: timestamp,
            position: PositionDr { odometer },
            trip: descriptor,
            vehicle: VehicleDr {
                id: vehicle.id.clone(),
            },
        };

        return Ok(Some(Location::DeadReckoning(dr_message)));
    }

    let descriptor = VehicleDescriptor {
        id: vehicle.id.clone(),
        label: vehicle.label.clone(),
        license_plate: vehicle.registration.clone(),
    };

    let occupancy_status = if let Some(trip) = trip_desc.as_ref() {
        get_occupancy_status(provider, &vehicle, trip).await?
    } else {
        None
    };

    let position = Position {
        latitude: location.latitude,
        longitude: location.longitude,
        bearing: location.heading,
        speed: location.speed.map(|value| value * 1000.0 / 3600.0),
        odometer,
    };

    let vehicle_position = VehiclePosition {
        position: Some(position),
        trip: trip_desc,
        vehicle: Some(descriptor),
        occupancy_status,
        timestamp,
    };

    let entity = FeedEntity {
        id: vehicle.id.clone(),
        vehicle: Some(vehicle_position),
    };
    Ok(Some(Location::VehiclePosition(entity)))
}

fn deserialize_optional<T>(bytes: Option<&[u8]>) -> Option<T>
where
    T: DeserializeOwned,
{
    bytes.and_then(|raw| serde_json::from_slice::<T>(raw).ok())
}

async fn allocate<P>(
    vehicle: &Vehicle, allocation: Option<BlockInstance>, timestamp: i64, provider: &P,
) -> Result<()>
where
    P: Config + HttpRequest + StateStore,
{
    let trip_key = state_keys::trip(&vehicle.id);
    let sign_on_key = state_keys::sign_on(&vehicle.id);

    // no allocation for this vehicle
    let Some(alloc) = allocation else {
        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        return Ok(());
    };

    if alloc.has_error() {
        return Ok(());
    }

    // is the allocated vehicle this vehicle?
    if alloc.vehicle_ids.first() != Some(&vehicle.id) {
        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        return Ok(());
    }

    // is this trip the same as the previous one?
    if let Some(bytes) = StateStore::get(provider, &trip_key).await? {
        let prev = serde_json::from_slice::<TripInstance>(&bytes)?;
        if prev.trip_id == alloc.trip_id
            && prev.start_time == alloc.start_time
            && prev.service_date == alloc.service_date
        {
            return Ok(());
        }
    }

    // try and get the new trip
    let Some(new_trip) =
        trip::get_instance(&alloc.trip_id, &alloc.service_date, &alloc.start_time, provider)
            .await?
    else {
        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        return Ok(());
    };

    if new_trip.has_error() {
        return Ok(());
    }

    // save the new trip
    let bytes = serde_json::to_vec(&new_trip).context("failed to serialize trip")?;
    StateStore::set(provider, &trip_key, &bytes, Some(duration_secs(TTL_TRIP_TRAIN))).await?;

    let bytes = serde_json::to_vec(&timestamp).context("failed to serialize message timestamp")?;
    StateStore::set(provider, &sign_on_key, &bytes, Some(duration_secs(TTL_SIGN_ON))).await?;

    Ok(())
}

async fn current_trip<P>(
    provider: &P, vehicle_id: &str, timestamp: i64,
) -> Result<Option<TripInstance>>
where
    P: StateStore,
{
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);
    let bytes = StateStore::get(provider, &trip_key).await?;

    if let Some(instance) = deserialize_optional::<TripInstance>(bytes.as_deref()) {
        let sign_on_bytes = StateStore::get(provider, &sign_on_key).await?;
        let sign_on = deserialize_optional::<i64>(sign_on_bytes.as_deref());
        if let (Some(sign_on_ts), Some(start), Some(end)) = (
            sign_on,
            time_to_timestamp(&instance.service_date, &instance.start_time, TIMEZONE),
            time_to_timestamp(&instance.service_date, &instance.end_time, TIMEZONE),
        ) {
            let duration = end - start + Duration::seconds(3_600).num_seconds();
            if timestamp - duration > sign_on_ts {
                StateStore::delete(provider, &sign_on_key).await?;
                StateStore::delete(provider, &trip_key).await?;
                return Ok(None);
            }
        }
        return Ok(Some(instance));
    }

    Ok(None)
}

async fn get_occupancy_status<P>(
    provider: &P, vehicle: &Vehicle, trip: &TripDescriptor,
) -> Result<Option<String>>
where
    P: StateStore,
{
    let Some(start_date) = trip.start_date.as_ref() else {
        return Ok(None);
    };

    let Some(start_time) = trip.start_time.as_ref() else {
        return Ok(None);
    };

    let key = state_keys::occupancy_status(&vehicle.id, &trip.trip_id, start_date, start_time);

    let Some(bytes) = StateStore::get(provider, &key).await? else {
        return Ok(None);
    };
    let occupancy_status = serde_json::from_slice(&bytes)?;

    Ok(Some(occupancy_status))
}

fn time_to_timestamp(date: &str, time: &str, tz: Tz) -> Option<i64> {
    if date.is_empty() || time.is_empty() {
        return None;
    }

    let date = NaiveDate::parse_from_str(date, "%Y%m%d").ok()?;
    let parts: Vec<_> = time.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    let seconds: i64 = parts[2].parse().ok()?;
    let base = date.and_hms_opt(0, 0, 0)?;
    let datetime = tz.from_local_datetime(&base).single()?;
    Some((datetime + Duration::seconds(hours * 3_600 + minutes * 60 + seconds)).timestamp())
}
