//! Location event processing.

use chrono::{Duration, NaiveDate, TimeZone};
use chrono_tz::Tz;
use omnia_sdk::{Config, HttpClient, Identity, Result, StateStore, StateStoreExt as _};
use uuid::Uuid;

use crate::block_mgt::{self, BlockInstance};
use crate::fleet::{self, Vehicle};
use crate::motion::{EventType, MotionMessage};
use crate::state_keys;
use crate::trip::{
    self, DeadReckoningMessage, FeedEntity, Position, PositionDr, TIMEZONE, TripDescriptor,
    TripInstance, VehicleDescriptor, VehicleDr, VehiclePosition,
};

const TTL_TRIP_TRAIN: std::time::Duration = std::time::Duration::from_hours(3);
const TTL_SIGN_ON: std::time::Duration = std::time::Duration::from_hours(24);

pub enum Location {
    VehiclePosition(FeedEntity),
    DeadReckoning(DeadReckoningMessage),
}

pub async fn process<P>(provider: &P, message: &MotionMessage) -> Result<Option<Location>>
where
    P: Config + HttpClient + Identity + StateStore,
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
    let Some(vehicle) = fleet::vehicle(provider, vehicle_id).await? else {
        tracing::debug!("vehicle info not found for {vehicle_id}");
        return Ok(None);
    };

    let timestamp = message.timestamp()?;

    if vehicle.is_train() {
        let allocation = block_mgt::cached_allocation(provider, &vehicle.id, timestamp).await?;
        allocate(provider, &vehicle, allocation, timestamp).await?;
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
        occupancy(provider, &vehicle, trip).await?
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

async fn allocate<P>(
    provider: &P, vehicle: &Vehicle, allocation: Option<BlockInstance>, timestamp: i64,
) -> Result<()>
where
    P: Config + HttpClient + StateStore,
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
    if let Some(prev) = provider.get_json::<TripInstance>(&trip_key).await?
        && prev.trip_id == alloc.trip_id
        && prev.start_time == alloc.start_time
        && prev.service_date == alloc.service_date
    {
        return Ok(());
    }

    // try and get the new trip
    let Some(new_trip) =
        trip::instance(provider, &alloc.trip_id, &alloc.service_date, &alloc.start_time).await?
    else {
        StateStore::delete(provider, &sign_on_key).await?;
        StateStore::delete(provider, &trip_key).await?;
        return Ok(());
    };

    if new_trip.has_error() {
        return Ok(());
    }

    // save the new trip
    provider.set_json(&trip_key, &new_trip, Some(TTL_TRIP_TRAIN)).await?;
    provider.set_json(&sign_on_key, &timestamp, Some(TTL_SIGN_ON)).await?;

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

    let Some(instance) = provider.get_json::<TripInstance>(&trip_key).await? else {
        return Ok(None);
    };
    let sign_on = provider.get_json::<i64>(&sign_on_key).await?;
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
    Ok(Some(instance))
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

async fn occupancy<P>(
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
    Ok(provider.get_json(&key).await?)
}
