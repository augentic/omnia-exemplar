//! Relational geo queries through [`TableStore`] and [`common::RowExt`].
//!
//! A radius search does not need a geospatial extension bolted onto the
//! key-value store: model the rows relationally, pre-filter with an indexable
//! bounding box, and refine the survivors with a haversine check in Rust.
//! [`UpsertPlaceRequest`] writes rows with a parameterized `INSERT … ON
//! CONFLICT` statement; [`NearbyPlacesRequest`] is the `GEORADIUS`
//! replacement.
//!
//! The upsert also demonstrates a structured wire error: [`PlaceError`]
//! replaces `omnia_sdk::Error` as the handler's error type, and its
//! [`IntoResponse`] implementation serializes it as an `application/json`
//! body that is a superset of the framework's `omnia_sdk::api::ErrorBody`
//! envelope — the fixed `error` / `message` pair every omnia transport
//! emits, plus the variant's own domain fields.

use axum::Json;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use common::{Capabilities, RowExt as _};
use http::{HeaderValue, StatusCode};
use omnia_sdk::api::{Client, Context};
use omnia_sdk::sql::{DataType, Row};
use omnia_sdk::{Error, HttpError, TableStore};
use serde::{Deserialize, Serialize};

pub(crate) const PLACES: &str = "/examples/patterns/places";
pub(crate) const NEARBY: &str = "/examples/patterns/nearby";
pub(crate) const CONNECTION: &str = "db";

/// Mean metres per degree of latitude (and of longitude at the equator).
const METRES_PER_DEGREE: f64 = 111_320.0;

pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {
    axum::Router::new().route(PLACES, post(upsert_place)).route(NEARBY, get(nearby_places))
}

/// A named point of interest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

impl TryFrom<&Row> for Place {
    type Error = Error;

    fn try_from(row: &Row) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.get_str("id")?.to_owned(),
            name: row.get_str("name")?.to_owned(),
            lat: row.get_f64("lat")?,
            lon: row.get_f64("lon")?,
        })
    }
}

/// Insert a place, or update it in full when the id already exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertPlaceReply {
    /// Rows affected by the statement.
    pub affected: u32,
}

/// Why a place request was rejected, the source of the JSON wire body.
///
/// The exemplar for structured error responses: instead of flattening
/// failures into `omnia_sdk::Error`, whose body is the fixed two-field
/// `omnia_sdk::api::ErrorBody` (`error`, `message`), the handler keeps its
/// own error type with domain fields, and the [`IntoResponse`] implementation
/// below puts the JSON on the wire. The serde `tag` is `ErrorBody`'s `error`
/// discriminant, `Display` supplies its `message`, and every other field is
/// domain data — so a client that only knows the framework envelope still
/// parses the body, and one that knows the route reads the extra fields.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
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
        /// What failed, including the error chain. Skipped by serde: it is
        /// the `Display` text, which reaches the wire as `message`.
        #[serde(skip)]
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

/// The wire body: omnia's `ErrorBody` fields, then the variant's own.
#[derive(Serialize)]
struct Body<'a> {
    message: String,
    #[serde(flatten)]
    error: &'a PlaceError, // `error` tag + domain fields
}

// A non-finite coordinate is the only unserializable field; the body then
// degrades to the plain-text form.
impl IntoResponse for PlaceError {
    fn into_response(self) -> Response {
        let body = Body {
            message: self.to_string(),
            error: &self,
        };
        serde_json::to_vec(&body)
            .map_or_else(
                |_| HttpError::new(self.status(), self.to_string()),
                |body| {
                    HttpError::with_body(
                        self.status(),
                        HeaderValue::from_static("application/json"),
                        body,
                    )
                },
            )
            .into_response()
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

pub(crate) async fn upsert_place<P>(
    context: Context<P>, Json(input): Json<UpsertPlaceRequest>,
) -> Result<Json<UpsertPlaceReply>, PlaceError>
where
    P: TableStore + 'static,
{
    coordinate("lat", input.lat, -90.0, 90.0)?;
    coordinate("lon", input.lon, -180.0, 180.0)?;

    let affected = TableStore::exec(
        context.provider(),
        CONNECTION.to_owned(),
        "INSERT INTO places (id, name, lat, lon) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, lat = EXCLUDED.lat, lon = EXCLUDED.lon"
            .to_owned(),
        vec![
            DataType::Str(Some(input.id)),
            DataType::Str(Some(input.name)),
            DataType::Double(Some(input.lat)),
            DataType::Double(Some(input.lon)),
        ],
    )
    .await?;

    Ok(Json(UpsertPlaceReply { affected }))
}

/// Find places within a radius of a point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyPlacesRequest {
    /// Centre latitude in degrees.
    pub lat: f64,
    /// Centre longitude in degrees.
    pub lon: f64,
    /// Search radius in metres.
    pub radius_m: f64,
}

/// A place with its distance from the query point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyPlace {
    /// The matched place.
    pub place: Place,
    /// Great-circle distance from the query point, in metres.
    pub distance_m: f64,
}

/// Places within the radius, nearest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyPlacesReply {
    /// Matches ordered by ascending distance.
    pub places: Vec<NearbyPlace>,
}

// A GET with a JSON body: axum's `Json` extractor reads it under any method.
pub(crate) async fn nearby_places<P>(
    context: Context<P>, Json(input): Json<NearbyPlacesRequest>,
) -> Result<Json<NearbyPlacesReply>, Error>
where
    P: TableStore + 'static,
{
    // A degree bounding box over-approximates the radius: cheap for the
    // database (plain comparisons, indexable), refined exactly below.
    // The longitude span widens with latitude; near the poles it covers
    // the full circle, which is harmless for a pre-filter.
    let lat_delta = input.radius_m / METRES_PER_DEGREE;
    let lon_delta = lat_delta / input.lat.to_radians().cos().abs().max(f64::EPSILON);

    let rows = TableStore::query(
        context.provider(),
        CONNECTION.to_owned(),
        "SELECT id, name, lat, lon FROM places \
         WHERE lat >= $1 AND lat <= $2 AND lon >= $3 AND lon <= $4"
            .to_owned(),
        vec![
            DataType::Double(Some(input.lat - lat_delta)),
            DataType::Double(Some(input.lat + lat_delta)),
            DataType::Double(Some(input.lon - lon_delta)),
            DataType::Double(Some(input.lon + lon_delta)),
        ],
    )
    .await?;

    // refine the box to the true radius in Rust — no GEORADIUS required
    let mut places = Vec::new();
    for row in &rows {
        let place = Place::try_from(row)?;
        let distance_m = haversine_m(input.lat, input.lon, place.lat, place.lon);
        if distance_m <= input.radius_m {
            places.push(NearbyPlace { place, distance_m });
        }
    }
    places.sort_by(|a, b| a.distance_m.total_cmp(&b.distance_m));

    Ok(Json(NearbyPlacesReply { places }))
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
