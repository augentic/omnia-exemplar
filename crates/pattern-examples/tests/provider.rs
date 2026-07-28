#![allow(missing_docs, clippy::missing_panics_doc)]
// Shared by several test binaries; not every binary uses every helper.
#![allow(dead_code)]

//! Spy mock provider for the pattern-example operations.
//!
//! Beyond canned responses, the mock *records* every outbound HTTP request
//! so tests can assert on the request shape — method, path, and the
//! `Client-Cert` header — without a real network.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::error::Error;
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result, anyhow, ensure};
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use omnia_guest::orm::{DataType, Field, Row};
use omnia_guest::{Config, HttpRequest, StateStore, TableStore};
use pattern_examples::decode::{CLIENT_CERT, DECODER_URL};

/// One outbound HTTP request as seen by the spy.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub client_cert: Option<String>,
}

/// One row of the in-memory `places` table, keyed by id.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceRow {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Default, Clone)]
pub struct MockProvider {
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    state: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    places: Arc<Mutex<BTreeMap<String, PlaceRow>>>,
}

impl MockProvider {
    #[must_use]
    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("lock").clone()
    }

    #[must_use]
    pub fn requests_for(&self, path: &str) -> Vec<RecordedRequest> {
        self.requests().into_iter().filter(|record| record.path == path).collect()
    }

    #[must_use]
    pub fn state(&self, key: &str) -> Option<Vec<u8>> {
        self.state.lock().expect("lock").get(key).cloned()
    }

    pub fn seed_state(&self, key: &str, value: impl Into<Vec<u8>>) {
        self.state.lock().expect("lock").insert(key.to_string(), value.into());
    }

    #[must_use]
    pub fn place(&self, id: &str) -> Option<PlaceRow> {
        self.places.lock().expect("lock").get(id).cloned()
    }
}

impl Config for MockProvider {
    async fn get(&self, key: &str) -> Result<String> {
        Ok(match key {
            DECODER_URL => "https://decoder.test/decode".to_string(),
            CLIENT_CERT => "test-client-cert".to_string(),
            _ => return Err(anyhow!("unknown config key: {key}")),
        })
    }
}

impl HttpRequest for MockProvider {
    async fn fetch<T>(&self, request: Request<T>) -> Result<Response<Bytes>>
    where
        T: http_body::Body + Any + Send,
        T::Data: Into<Vec<u8>>,
        T::Error: Into<Box<dyn Error + Send + Sync + 'static>>,
    {
        let record = RecordedRequest {
            method: request.method().to_string(),
            path: request.uri().path().to_string(),
            client_cert: request
                .headers()
                .get("Client-Cert")
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string),
        };
        self.requests.lock().expect("lock").push(record);

        let body = match request.uri().path() {
            "/decode" => include_bytes!("../data/segment.json").to_vec(),
            path => return Err(anyhow!("unexpected request path: {path}")),
        };
        Response::builder()
            .status(StatusCode::OK)
            .body(Bytes::from(body))
            .context("building mock response")
    }
}

impl StateStore for MockProvider {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().expect("lock").get(key).cloned())
    }

    async fn set(
        &self, key: &str, value: &[u8], _ttl_secs: Option<u64>,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().expect("lock").insert(key.to_string(), value.to_vec()))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.state.lock().expect("lock").remove(key);
        Ok(())
    }
}

impl TableStore for MockProvider {
    async fn query(
        &self, _conn: String, query: String, params: Vec<DataType>,
    ) -> Result<Vec<Row>> {
        ensure!(query.starts_with("SELECT"), "unexpected query: {query}");

        // Bound params in the order the nearby operation's filters push
        // them: lat >=, lat <=, lon >=, lon <=.
        let [
            DataType::Double(Some(lat_min)),
            DataType::Double(Some(lat_max)),
            DataType::Double(Some(lon_min)),
            DataType::Double(Some(lon_max)),
        ] = params.as_slice()
        else {
            return Err(anyhow!("expected four double bound params"));
        };

        Ok(self
            .places
            .lock()
            .expect("lock")
            .iter()
            .filter(|(_, row)| {
                row.lat >= *lat_min
                    && row.lat <= *lat_max
                    && row.lon >= *lon_min
                    && row.lon <= *lon_max
            })
            .enumerate()
            .map(|(index, (id, row))| Row {
                index: index.to_string(),
                fields: vec![
                    Field {
                        name: "id".to_string(),
                        value: DataType::Str(Some(id.clone())),
                    },
                    Field {
                        name: "name".to_string(),
                        value: DataType::Str(Some(row.name.clone())),
                    },
                    Field {
                        name: "lat".to_string(),
                        value: DataType::Double(Some(row.lat)),
                    },
                    Field {
                        name: "lon".to_string(),
                        value: DataType::Double(Some(row.lon)),
                    },
                ],
            })
            .collect())
    }

    async fn exec(&self, _conn: String, query: String, params: Vec<DataType>) -> Result<u32> {
        ensure!(query.starts_with("INSERT"), "unexpected statement: {query}");

        let [
            DataType::Str(Some(id)),
            DataType::Str(Some(name)),
            DataType::Double(Some(lat)),
            DataType::Double(Some(lon)),
        ] = params.as_slice()
        else {
            return Err(anyhow!("expected id, name, lat, lon params"));
        };

        // Honour the statement's `ON CONFLICT ("id") DO UPDATE` semantics.
        self.places.lock().expect("lock").insert(
            id.clone(),
            PlaceRow {
                name: name.clone(),
                lat: *lat,
                lon: *lon,
            },
        );
        Ok(1)
    }
}
