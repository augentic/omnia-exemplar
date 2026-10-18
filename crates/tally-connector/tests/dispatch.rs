//! The connector's route driven through its router, as the guest serves it.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use tally_connector::{APC, TallyMessage};
use tower::ServiceExt as _;

const TALLY_MESSAGE: &[u8] = include_bytes!("../data/tally-message.json");

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

#[tokio::test]
async fn apc_tally() {
    let provider = provider();
    let request = Request::post(APC)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(TALLY_MESSAGE))
        .expect("request");
    let router = tally_connector::router().with_state(Client::new("acme", provider.clone()));

    let response = router.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), br#""OK""#);

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);
    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-tally-apc.v2");
    let message: TallyMessage = serde_json::from_slice(TALLY_MESSAGE).expect("deserialize");
    let site = message.device.as_ref().expect("device").site.as_str();
    assert_eq!(record.headers.get("key").map(String::as_str), Some(site));
}
