//! The connector's route driven through its router: the codec contract of
//! the `handle_with` route, which answers in the vendor's XML either way.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, Response, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use pulse_connector::PULSE_XML;
use tower::ServiceExt as _;

const RECEIVE_MESSAGE: &[u8] = include_bytes!("../data/receive-message.xml");

fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

async fn post(provider: Provider, body: &'static [u8]) -> Response<Body> {
    let request = Request::post(PULSE_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(body))
        .expect("request");
    let router = pulse_connector::router().with_state(Client::new("acme", provider));
    router.oneshot(request).await.expect("response")
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

// Malformed XML is answered with the vendor's fault, not the framework's
// JSON `invalid_request` 400.
#[tokio::test]
async fn pulse_malformed_xml() {
    let response = post(provider(), b"<garbage/>").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert!(String::from_utf8_lossy(&body).contains("<Fault>"));
}

#[tokio::test]
async fn pulse_receive() {
    let provider = provider();

    let response = post(provider.clone(), RECEIVE_MESSAGE).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(body.as_ref(), b"<Return>OK</Return>");
    assert_eq!(provider.publish.sent().len(), 1);
}
