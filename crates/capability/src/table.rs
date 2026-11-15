//! SQL example: record a sensor reading and count the sensor's rows.

use axum::Json;
use axum::routing::post;
use omnia_sdk::api::{Context, Handlers};
use omnia_sdk::sql::Value;
use omnia_sdk::{Result, SqlDb};
use serde::{Deserialize, Serialize};

pub(crate) const READING: &str = "/examples/reading";

pub(crate) fn handlers<P: crate::Requires>() -> Handlers<P> {
    Handlers::new().route(READING, post(reading))
}

/// SQL executed against the readings table.
pub mod sql {
    /// Insert one reading.
    pub const INSERT: &str = "INSERT INTO readings (sensor, value) VALUES ($1, $2)";

    /// Select every reading for one sensor.
    pub const SELECT: &str = "SELECT sensor, value FROM readings WHERE sensor = $1";
}

/// Record a sensor reading.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadingRequest {
    /// Database binding declared by the deployment.
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
    pub affected: u64,
    /// Total rows now stored for the sensor.
    pub rows: usize,
}

pub(crate) async fn reading<P>(
    context: Context<P>, Json(input): Json<ReadingRequest>,
) -> Result<Json<ReadingReply>>
where
    P: SqlDb,
{
    let db = context.sql().named(&input.connection);

    let affected = db
        .execute(sql::INSERT, &[Value::from(input.sensor.as_str()), Value::from(input.value)])
        .await?;
    let rows = db.query(sql::SELECT, &[Value::from(input.sensor)]).await?;

    Ok(Json(ReadingReply {
        affected,
        rows: rows.len(),
    }))
}
