//! Table-store example: record a sensor reading and count the sensor's rows.

use axum::Json;
use axum::routing::post;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::sql::DataType;
use omnia_sdk::{Capabilities, Result, TableStore};
use serde::{Deserialize, Serialize};

pub(crate) const READING: &str = "/examples/reading";

pub(crate) fn handlers<P: Capabilities>() -> Handlers<P> {
    Handlers::new().route(READING, post(reading))
}

/// SQL executed against the readings table.
pub mod sql {
    /// Insert one reading.
    pub const INSERT: &str = "INSERT INTO readings (sensor, value) VALUES (?, ?)";

    /// Select every reading for one sensor.
    pub const SELECT: &str = "SELECT sensor, value FROM readings WHERE sensor = ?";
}

/// Record a sensor reading.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadingRequest {
    /// Named connection configured by the host.
    pub connection: String,
    /// Sensor identifier.
    pub sensor: String,
    /// Measured value.
    pub value: f64,
}

/// Reading state after the insert.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadingReply {
    /// Rows affected by the insert.
    pub affected: u32,
    /// Total rows now stored for the sensor.
    pub rows: usize,
}

pub(crate) async fn reading<P>(
    context: Context<P>, Json(input): Json<ReadingRequest>,
) -> Result<Json<ReadingReply>>
where
    P: TableStore,
{
    let tables = context.tables();

    let affected = tables
        .exec(
            &input.connection,
            sql::INSERT,
            &[DataType::Str(Some(input.sensor.clone())), DataType::Double(Some(input.value))],
        )
        .await?;

    let rows =
        tables.query(&input.connection, sql::SELECT, &[DataType::Str(Some(input.sensor))]).await?;

    Ok(Json(ReadingReply {
        affected,
        rows: rows.len(),
    }))
}
