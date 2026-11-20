//! The gtfs-adapter's component over the backends a test seeds, and the
//! exact upstream URLs the handlers call, so each test seeds the request it
//! expects rather than a prefix.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use http::{Request, Response};
use omnia::{Provides, Runtime};
use omnia_test::host::{
    self, Backends, Deployment, FixedIdentity, HandleError, MatchedHttp, STATE_BUCKET, Sink,
    precompiled,
};
use omnia_wasi_config::WasiConfig;
use omnia_wasi_http::WasiHttp;
use omnia_wasi_identity::WasiIdentity;
use omnia_wasi_keyvalue::{KeyValueClient as _, Lifetime, WasiKeyValue};
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;
use serde_json::{Value, json};

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

pub const FLEET_BASE: &str = "http://fleet.test";
pub const BLOCK_MGT_BASE: &str = "http://block-mgt.test";
pub const TRIP_MANAGEMENT_BASE: &str = "http://trip-mgt.test";

/// The configuration keys the handlers read.
pub const CONFIG: [(&str, &str); 4] = [
    ("BLOCK_MGT_URL", BLOCK_MGT_BASE),
    ("FLEET_URL", FLEET_BASE),
    ("TRIP_MANAGEMENT_URL", TRIP_MANAGEMENT_BASE),
    ("API_IDENTITY", "test-identity"),
];

/// The stem's backends: configuration and state, the upstream answers a test
/// seeds, and every message the component publishes.
///
/// Each delivery and request boots the component afresh over them, and every
/// run shares their stores.
pub struct Adapter {
    /// The default backends with the handlers' configuration.
    pub backends: Backends,
    /// The upstream answers, recording every request a run sends.
    pub http: MatchedHttp,
    /// Every message a run publishes.
    pub sink: Sink,
}

impl Adapter {
    /// An adapter over [`CONFIG`], answering upstream requests from `http`.
    pub async fn new(http: MatchedHttp) -> Self {
        Self::with_config(http, []).await
    }

    /// An adapter over [`CONFIG`] and `extra`, answering upstream requests
    /// from `http`.
    pub async fn with_config(
        http: MatchedHttp, extra: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Self {
        let backends = Backends::defaults().await.config(CONFIG.into_iter().chain(extra));
        Self {
            backends,
            http,
            sink: Sink::default(),
        }
    }

    /// Stores `value` at `key` in the state bucket, with no expiry.
    pub async fn plant(&self, key: &str, value: &[u8]) {
        let bucket =
            self.backends.keyvalue.open_bucket(STATE_BUCKET.to_owned()).await.expect("bucket");
        bucket
            .set(key.to_owned(), value.to_vec(), Lifetime::Replace(None), None)
            .await
            .expect("value stored")
            .expect("no precondition to fail");
    }

    /// Delivers `payload` on `topic` to the component.
    pub async fn deliver(
        &self, topic: &str, payload: impl Into<Vec<u8>>,
    ) -> Result<(), HandleError> {
        let runtime = self.boot().await;
        let settled = host::deliver(&runtime, topic, payload).await;
        runtime.shutdown();
        settled
    }

    /// Sends `request` to the component and returns its response.
    pub async fn send(&self, request: Request<Bytes>) -> Response<Bytes> {
        let runtime = self.boot().await;
        let response = host::send(&runtime, request).await.expect("request served");
        runtime.shutdown();
        response
    }

    async fn boot(&self) -> Runtime<impl Provides<WasiHttp> + Provides<WasiMessaging>> {
        let backends = self
            .backends
            .clone()
            .http(self.http.clone())
            .identity(FixedIdentity::new("mock_access_token"))
            .messaging(self.sink.clone());
        let guest = precompiled(GTFS_ADAPTER_GUEST).expect("guest precompiles");
        Deployment::new()
            .guest("gtfs-adapter", guest)
            .boot(backends, |deployment| {
                deployment.host::<WasiConfig>()?;
                deployment.host::<WasiHttp>()?;
                deployment.host::<WasiIdentity>()?;
                deployment.host::<WasiKeyValue>()?;
                deployment.host::<WasiMessaging>()?;
                deployment.host::<WasiOtel>()?;
                Ok(())
            })
            .await
            .expect("runtime boots")
    }
}

/// The Fleet API lookup `gtfs_adapter::fleet::vehicle` makes for `vehicle_id`.
pub fn fleet_query(vehicle_id: &str) -> String {
    let query = match vehicle_id {
        "EMP484" => "label=EMP%20%20%20%20%20%20%20%20484",
        "EM633" => "label=EM%20%20%20%20%20%20%20%20%20633",
        other => panic!("unexpected test vehicle_id: {other}"),
    };
    format!("{FLEET_BASE}/vehicles?{query}")
}

/// The Block Management lookup `cached_allocation` makes for a fleet id at a
/// message timestamp.
pub fn allocation_query(fleet_id: &str, at: DateTime<Utc>) -> String {
    format!(
        "{BLOCK_MGT_BASE}/allocations/vehicles/{fleet_id}?currentTrip=true&siblings=true&nowUnixTimeSeconds={}",
        at.timestamp()
    )
}

/// The Trip Management instances endpoint.
pub fn trip_instances() -> String {
    format!("{TRIP_MANAGEMENT_BASE}/tripinstances")
}

/// A body matcher for the Trip Management lookup of `trip_id` on `service_date`.
pub fn trip_query(
    trip_id: &str, service_date: &str,
) -> impl Fn(&[u8]) -> bool + Send + Sync + 'static {
    let expected = json!({ "tripIds": [trip_id], "serviceDate": service_date });
    move |body: &[u8]| serde_json::from_slice::<Value>(body).is_ok_and(|value| value == expected)
}

/// A trip instance as returned by the Trip Management API.
pub fn trip_instance(trip_id: &str) -> Value {
    json!({
        "tripId": trip_id,
        "routeId": "R1",
        "serviceDate": "20251120",
        "startTime": "11:30:00",
        "endTime": "12:30:00",
        "directionId": null,
        "isAddedTrip": false,
    })
}

/// A serial-data message signing `vehicle_id` on to `trip_id` at `at`.
pub fn serial_data(vehicle_id: &str, trip_id: &str, at: DateTime<Utc>) -> String {
    json!({
        "eventType": "serialData",
        "remoteData": { "externalId": vehicle_id },
        "messageData": { "timestamp": at.to_rfc3339() },
        "serialData": {
            "decodedSerialData": { "tripId": trip_id, "tripNumber": trip_id, "lineId": "L1" },
        },
    })
    .to_string()
}

/// A `200` carrying `body`.
pub fn ok(body: impl Into<Bytes>) -> Response<Bytes> {
    Response::new(body.into())
}
