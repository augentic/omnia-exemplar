#![allow(missing_docs)]
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

use anyhow::{Result, anyhow};
use bytes::Bytes;
use http::{Request, Response, StatusCode};
use omnia_guest::orm::{DataType, Field, Row};
use omnia_guest::{CasError, Config, HttpRequest, StateStore, TableStore};
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

#[allow(clippy::missing_panics_doc)]
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
    fn get(&self, key: &str) -> impl Future<Output = Result<String>> {
        std::future::ready(match key {
            DECODER_URL => Ok("https://decoder.test/decode".to_string()),
            CLIENT_CERT => Ok("test-client-cert".to_string()),
            _ => Err(anyhow!("unknown config key: {key}")),
        })
    }
}

impl HttpRequest for MockProvider {
    fn fetch<T>(&self, request: Request<T>) -> impl Future<Output = Result<Response<Bytes>>>
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
        let Ok(mut requests) = self.requests.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on requests")));
        };
        requests.push(record);

        let body = match request.uri().path() {
            "/decode" => include_bytes!("../data/segment.json").to_vec(),
            path => return std::future::ready(Err(anyhow!("unexpected request path: {path}"))),
        };
        match Response::builder().status(StatusCode::OK).body(Bytes::from(body)) {
            Ok(response) => std::future::ready(Ok(response)),
            Err(error) => std::future::ready(Err(error.into())),
        }
    }
}

impl StateStore for MockProvider {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        let Ok(state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        std::future::ready(Ok(state.get(key).cloned()))
    }

    fn set(
        &self, key: &str, value: &[u8], _ttl_secs: Option<u64>,
    ) -> impl Future<Output = Result<Option<Vec<u8>>>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        std::future::ready(Ok(state.insert(key.to_string(), value.to_vec())))
    }

    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        state.remove(key);
        std::future::ready(Ok(()))
    }

    fn cas(
        &self, key: &str, expected: Option<&[u8]>, value: &[u8],
    ) -> impl Future<Output = Result<(), CasError>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(CasError::Store(
                "failed to obtain lock on state".into(),
            )));
        };
        let observed = state.get(key).cloned();
        if observed.as_deref() != expected {
            return std::future::ready(Err(CasError::Conflict(observed)));
        }
        state.insert(key.to_string(), value.to_vec());
        std::future::ready(Ok(()))
    }

    fn increment(&self, key: &str, delta: i64) -> impl Future<Output = Result<i64>> {
        let Ok(mut state) = self.state.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on state")));
        };
        let current = match state.get(key) {
            None => 0,
            Some(value) => {
                let Ok(bytes) = <[u8; 8]>::try_from(value.as_slice()) else {
                    return std::future::ready(Err(anyhow!(
                        "value is {} bytes, not an 8-byte big-endian integer",
                        value.len()
                    )));
                };
                i64::from_be_bytes(bytes)
            }
        };
        let Some(incremented) = current.checked_add(delta) else {
            return std::future::ready(Err(anyhow!("adding delta overflows i64")));
        };
        state.insert(key.to_string(), incremented.to_be_bytes().to_vec());
        std::future::ready(Ok(incremented))
    }
}

impl TableStore for MockProvider {
    fn query(
        &self, _conn: String, query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<Vec<Row>>> {
        if !query.starts_with("SELECT") {
            return std::future::ready(Err(anyhow!("unexpected query: {query}")));
        }

        // Bound params in the order the nearby operation's filters push
        // them: lat >=, lat <=, lon >=, lon <=.
        let [
            DataType::Double(Some(lat_min)),
            DataType::Double(Some(lat_max)),
            DataType::Double(Some(lon_min)),
            DataType::Double(Some(lon_max)),
        ] = params.as_slice()
        else {
            return std::future::ready(Err(anyhow!("expected four double bound params")));
        };

        let Ok(places) = self.places.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on places")));
        };

        std::future::ready(Ok(places
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
            .collect()))
    }

    fn exec(
        &self, _conn: String, query: String, params: Vec<DataType>,
    ) -> impl Future<Output = Result<u32>> {
        if !query.starts_with("INSERT") {
            return std::future::ready(Err(anyhow!("unexpected statement: {query}")));
        }

        let [
            DataType::Str(Some(id)),
            DataType::Str(Some(name)),
            DataType::Double(Some(lat)),
            DataType::Double(Some(lon)),
        ] = params.as_slice()
        else {
            return std::future::ready(Err(anyhow!("expected id, name, lat, lon params")));
        };

        let Ok(mut places) = self.places.lock() else {
            return std::future::ready(Err(anyhow!("failed to obtain lock on places")));
        };

        // Honour the statement's `ON CONFLICT ("id") DO UPDATE` semantics.
        places.insert(
            id.clone(),
            PlaceRow {
                name: name.clone(),
                lat: *lat,
                lon: *lon,
            },
        );
        std::future::ready(Ok(1))
    }
}
