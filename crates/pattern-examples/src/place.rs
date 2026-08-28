//! Relational geo queries through `TableStore` and the guest ORM.
//!
//! A radius search does not need a geospatial extension bolted onto the
//! key-value store: model the rows relationally, pre-filter with an indexable
//! bounding box, and refine the survivors with a haversine check in Rust.
//! [`UpsertPlaceRequest`] writes rows with the ORM's `INSERT … ON CONFLICT`
//! builder; [`NearbyPlacesRequest`] is the `GEORADIUS` replacement.
//!
//! The upsert also demonstrates a structured wire error: [`PlaceError`]
//! replaces `omnia_guest::Error` as the handler's error type, and its
//! [`HttpError`] conversion serializes it as an `application/json` body —
//! so error responses carry domain fields in the same content type as
//! success responses, instead of the default plain-text
//! `code: …, description: …` body.

use anyhow::Context as _;
use http::{HeaderValue, StatusCode};
use omnia_guest::api::Context;
use omnia_guest::orm::{Entity as _, Filter, InsertBuilder, SelectBuilder};
use omnia_guest::{Error, HttpError, TableStore, entity};
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

/// Why a place request was rejected, serialized verbatim as the wire body.
///
/// The exemplar for structured error responses: instead of flattening
/// failures into `omnia_guest::Error`'s plain-text `code: …, description: …`
/// body, the handler keeps its own error type with domain fields, and the
/// [`HttpError`] conversion below puts the JSON on the wire. The serde `tag`
/// doubles as the error code, keeping the code/description convention.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum PlaceError {
    /// A coordinate is outside its valid range (or not a number).
    #[error("{field} {value} is outside [{min}, {max}]")]
    InvalidCoordinate {
        /// The rejected field.
        field: &'static str,
        /// The offending value.
        value: f64,
        /// The smallest allowed value.
        min: f64,
        /// The largest allowed value.
        max: f64,
    },

    /// The statement could not be built or executed.
    #[error("{description}")]
    Storage {
        /// What failed, including the error chain.
        description: String,
    },
}

impl PlaceError {
    /// The HTTP status paired with the JSON body.
    const fn status(&self) -> StatusCode {
        match self {
            Self::InvalidCoordinate { .. } => StatusCode::BAD_REQUEST,
            Self::Storage { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Encode the error as an `application/json` response body.
///
/// This conversion is what the HTTP route uses on the error path: handler
/// errors bypass the route's success encoder, so the wire shape of an error
/// lives here. `HttpError::with_body` carries the preformatted bytes and
/// content type to the response unchanged.
impl From<PlaceError> for HttpError {
    fn from(error: PlaceError) -> Self {
        serde_json::to_vec(&error).map_or_else(
            // Unreachable in practice (a non-finite coordinate is the only
            // unserializable field); degrade to the plain-text form.
            |_| Self::new(error.status(), error.to_string()),
            |body| {
                Self::with_body(error.status(), HeaderValue::from_static("application/json"), body)
            },
        )
    }
}

/// Storage failures keep their error chain but no domain fields.
impl From<anyhow::Error> for PlaceError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage {
            description: format!("{error:#}"),
        }
    }
}

/// Reject a coordinate outside its inclusive range (`NaN` always fails).
fn coordinate(field: &'static str, value: f64, min: f64, max: f64) -> Result<(), PlaceError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(PlaceError::InvalidCoordinate {
            field,
            value,
            min,
            max,
        })
    }
}

#[omnia_guest::handler]
async fn upsert_place_request<P>(
    input: UpsertPlaceRequest, context: Context<'_, P>,
) -> Result<UpsertPlaceReply, PlaceError>
where
    P: TableStore,
{
    coordinate("lat", input.lat, -90.0, 90.0)?;
    coordinate("lon", input.lon, -180.0, 180.0)?;

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

#[omnia_guest::handler]
async fn nearby_places_request<P>(
    input: NearbyPlacesRequest, context: Context<'_, P>,
) -> Result<NearbyPlacesReply, Error>
where
    P: TableStore,
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
