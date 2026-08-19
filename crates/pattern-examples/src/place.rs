//! Relational geo queries through `TableStore` and the guest ORM.
//!
//! A radius search does not need a geospatial extension bolted onto the
//! key-value store: model the rows relationally, pre-filter with an indexable
//! bounding box, and refine the survivors with a haversine check in Rust.
//! [`UpsertPlaceRequest`] writes rows with the ORM's `INSERT … ON CONFLICT`
//! builder; [`NearbyPlacesRequest`] is the `GEORADIUS` replacement.

use anyhow::Context as _;
use omnia_guest::api::{CallContext, Provider};
use omnia_guest::orm::{Entity as _, Filter, InsertBuilder, SelectBuilder};
use omnia_guest::{Result, TableStore, entity};
use serde::{Deserialize, Serialize};

/// Named connection configured by the host.
pub const CONNECTION: &str = "db";

/// Mean metres per degree of latitude (and of longitude at the equator).
const METRES_PER_DEGREE: f64 = 111_320.0;

entity!(
    table = "places",
    /// A named point of interest.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Place {
        /// Stable identifier (conflict target for upserts).
        pub id: String,
        /// Display name.
        pub name: String,
        /// Latitude in degrees.
        pub lat: f64,
        /// Longitude in degrees.
        pub lon: f64,
    }
);

/// Insert a place, or update it in full when the id already exists.
#[derive(Debug, Clone, Deserialize)]
pub struct UpsertPlaceRequest {
    /// Stable identifier (primary key).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Latitude in degrees.
    pub lat: f64,
    /// Longitude in degrees.
    pub lon: f64,
}

/// Upsert outcome.
#[derive(Debug, Clone, Serialize)]
pub struct UpsertPlaceReply {
    /// Rows affected by the statement.
    pub affected: u32,
}

#[omnia_guest::operation]
async fn upsert_place_request<P>(
    input: UpsertPlaceRequest, context: CallContext<'_, P>,
) -> Result<UpsertPlaceReply>
where
    P: Provider + TableStore,
{
    let place = Place {
        id: input.id,
        name: input.name,
        lat: input.lat,
        lon: input.lon,
    };

    let query = InsertBuilder::from_entity(&place)
        .on_conflict("id")
        .do_update_all()
        .build()
        .context("building place upsert")?;

    let affected =
        TableStore::exec(context.provider, CONNECTION.to_string(), query.sql, query.params).await?;

    Ok(UpsertPlaceReply { affected })
}

/// Find places within a radius of a point.
#[derive(Debug, Clone, Deserialize)]
pub struct NearbyPlacesRequest {
    /// Centre latitude in degrees.
    pub lat: f64,
    /// Centre longitude in degrees.
    pub lon: f64,
    /// Search radius in metres.
    pub radius_m: f64,
}

/// A place with its distance from the query point.
#[derive(Debug, Clone, Serialize)]
pub struct NearbyPlace {
    /// The matched place.
    pub place: Place,
    /// Great-circle distance from the query point, in metres.
    pub distance_m: f64,
}

/// Places within the radius, nearest first.
#[derive(Debug, Clone, Serialize)]
pub struct NearbyPlacesReply {
    /// Matches ordered by ascending distance.
    pub places: Vec<NearbyPlace>,
}

#[omnia_guest::operation]
async fn nearby_places_request<P>(
    input: NearbyPlacesRequest, context: CallContext<'_, P>,
) -> Result<NearbyPlacesReply>
where
    P: Provider + TableStore,
{
    // A degree bounding box over-approximates the radius: cheap for the
    // database (plain comparisons, indexable), refined exactly below.
    // The longitude span widens with latitude; near the poles it covers
    // the full circle, which is harmless for a pre-filter.
    let lat_delta = input.radius_m / METRES_PER_DEGREE;
    let lon_delta = lat_delta / input.lat.to_radians().cos().abs().max(f64::EPSILON);

    let query = SelectBuilder::<Place>::new()
        .r#where(Filter::and([
            Filter::gte("lat", input.lat - lat_delta),
            Filter::lte("lat", input.lat + lat_delta),
            Filter::gte("lon", input.lon - lon_delta),
            Filter::lte("lon", input.lon + lon_delta),
        ]))
        .build()
        .context("building nearby query")?;

    let rows = TableStore::query(context.provider, CONNECTION.to_string(), query.sql, query.params)
        .await?;

    // Refine the box to the true radius in Rust — no GEORADIUS required.
    let mut places = Vec::new();
    for row in &rows {
        let place = Place::from_row(row).context("mapping place row")?;
        let distance_m = haversine_m(input.lat, input.lon, place.lat, place.lon);
        if distance_m <= input.radius_m {
            places.push(NearbyPlace { place, distance_m });
        }
    }
    places.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));

    Ok(NearbyPlacesReply { places })
}

/// Great-circle distance in metres between two WGS-84 points.
fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;

    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (lat1.to_radians().cos() * lat2.to_radians().cos())
        .mul_add((d_lon / 2.0).sin().powi(2), (d_lat / 2.0).sin().powi(2));
    2.0 * EARTH_RADIUS_M * a.sqrt().atan2((1.0 - a).sqrt())
}
