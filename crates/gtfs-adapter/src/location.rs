//! Location event processing.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Timestamp};
use omnia_sdk::Result;
use omnia_sdk::keyvalue::{self, Bucket};
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

pub async fn process(message: &MotionMessage) -> Result<Option<Location>> {
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
    let Some(vehicle) = fleet::vehicle(vehicle_id).await? else {
        tracing::debug!("vehicle info not found for {vehicle_id}");
        return Ok(None);
    };

    let timestamp = message.timestamp()?;
    let state = keyvalue::default().await?;

    if vehicle.is_train() {
        let allocation = block_mgt::cached_allocation(&vehicle.id, timestamp).await?;
        allocate(&state, &vehicle, allocation, timestamp).await?;
    }
    let trip_inst = current_trip(&state, &vehicle.id, timestamp).await?;
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
        occupancy(&state, &vehicle, trip).await?
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

async fn allocate(
    state: &Bucket, vehicle: &Vehicle, allocation: Option<BlockInstance>, timestamp: i64,
) -> Result<()> {
    let trip_key = state_keys::trip(&vehicle.id);
    let sign_on_key = state_keys::sign_on(&vehicle.id);

    // no allocation for this vehicle
    let Some(alloc) = allocation else {
        state.remove(&sign_on_key).await?;
        state.remove(&trip_key).await?;
        return Ok(());
    };

    if alloc.has_error() {
        return Ok(());
    }

    // is the allocated vehicle this vehicle?
    if alloc.vehicle_ids.first() != Some(&vehicle.id) {
        state.remove(&sign_on_key).await?;
        state.remove(&trip_key).await?;
        return Ok(());
    }

    // is this trip the same as the previous one?
    if let Some(prev) = state.get_json::<TripInstance>(&trip_key).await?
        && prev.trip_id == alloc.trip_id
        && prev.start_time == alloc.start_time
        && prev.service_date == alloc.service_date
    {
        return Ok(());
    }

    // try and get the new trip
    let Some(new_trip) =
        trip::instance(&alloc.trip_id, &alloc.service_date, &alloc.start_time).await?
    else {
        state.remove(&sign_on_key).await?;
        state.remove(&trip_key).await?;
        return Ok(());
    };

    if new_trip.has_error() {
        return Ok(());
    }

    // save the new trip
    state.set_json(&trip_key, &new_trip, Some(TTL_TRIP_TRAIN)).await?;
    state.set_json(&sign_on_key, &timestamp, Some(TTL_SIGN_ON)).await?;

    Ok(())
}

async fn current_trip(
    state: &Bucket, vehicle_id: &str, timestamp: i64,
) -> Result<Option<TripInstance>> {
    let trip_key = state_keys::trip(vehicle_id);
    let sign_on_key = state_keys::sign_on(vehicle_id);

    let Some(instance) = state.get_json::<TripInstance>(&trip_key).await? else {
        return Ok(None);
    };
    let sign_on = state.get_json::<i64>(&sign_on_key).await?;
    if let (Some(sign_on_ts), Some(start), Some(end)) = (
        sign_on,
        time_to_timestamp(&instance.service_date, &instance.start_time, &TIMEZONE),
        time_to_timestamp(&instance.service_date, &instance.end_time, &TIMEZONE),
    ) {
        let duration = end - start + 3_600;
        if timestamp - duration > sign_on_ts {
            state.remove(&sign_on_key).await?;
            state.remove(&trip_key).await?;
            return Ok(None);
        }
    }
    Ok(Some(instance))
}

fn time_to_timestamp(date: &str, time: &str, tz: &TimeZone) -> Option<i64> {
    if date.is_empty() || time.is_empty() {
        return None;
    }

    let date = Date::strptime("%Y%m%d", date).ok()?;
    let parts: Vec<_> = time.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    let seconds: i64 = parts[2].parse().ok()?;
    let midnight = tz.to_ambiguous_timestamp(date.at(0, 0, 0, 0)).unambiguous().ok()?;
    midnight
        .checked_add(SignedDuration::from_secs(hours * 3_600 + minutes * 60 + seconds))
        .ok()
        .map(Timestamp::as_second)
}

async fn occupancy(
    state: &Bucket, vehicle: &Vehicle, trip: &TripDescriptor,
) -> Result<Option<String>> {
    let Some(start_date) = trip.start_date.as_ref() else {
        return Ok(None);
    };

    let Some(start_time) = trip.start_time.as_ref() else {
        return Ok(None);
    };

    let key = state_keys::occupancy_status(&vehicle.id, &trip.trip_id, start_date, start_time);
    state.get_json(&key).await
}
