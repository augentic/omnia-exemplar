//! Table-store example: record a sensor reading and count the sensor's rows.

use omnia_guest::api::{CallContext, Operation, Provider};
use omnia_guest::{Error, Result, TableStore};
use omnia_wasi_sql::DataType;
use serde::{Deserialize, Serialize};

/// SQL executed against the readings table.
pub mod sql {
    /// Insert one reading.
    pub const INSERT: &str = "INSERT INTO readings (sensor, value) VALUES (?, ?)";

    /// Select every reading for one sensor.
    pub const SELECT: &str = "SELECT sensor, value FROM readings WHERE sensor = ?";
}

/// Record a sensor reading.
#[derive(Debug, Clone, Deserialize)]
pub struct ReadingRequest {
    /// Named connection configured by the host.
    pub connection: String,
    /// Sensor identifier.
    pub sensor: String,
    /// Measured value.
    pub value: f64,
}

/// Reading state after the insert.
#[derive(Debug, Clone, Serialize)]
pub struct ReadingReply {
    /// Rows affected by the insert.
    pub affected: u32,
    /// Total rows now stored for the sensor.
    pub rows: usize,
}

impl<P> Operation<P> for ReadingRequest
where
    P: Provider + TableStore,
{
    type Error = Error;
    type Input = Self;
    type Output = ReadingReply;

    async fn call(input: Self, context: CallContext<'_, P>) -> Result<ReadingReply> {
        let provider = context.provider;

        let affected = TableStore::exec(
            provider,
            input.connection.clone(),
            sql::INSERT.to_string(),
            vec![DataType::Str(Some(input.sensor.clone())), DataType::Double(Some(input.value))],
        )
        .await?;

        let rows = TableStore::query(
            provider,
            input.connection,
            sql::SELECT.to_string(),
            vec![DataType::Str(Some(input.sensor))],
        )
        .await?;

        Ok(ReadingReply {
            affected,
            rows: rows.len(),
        })
    }
}
