//! Static tests for the Pulse SOAP/XML connector driven through its router.

use axum::body::{Body, to_bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, Response, StatusCode};
use omnia_sdk::api::Client;
use omnia_test::guest::{MapConfig, Provider};
use pulse_connector::{PULSE_XML, PulseRequest};
use tower::ServiceExt as _;

const OWNER: &str = "acme";

// `config::env` falls back to `dev` when `ENV` is unset; seeding it keeps the
// topic assertions honest rather than leaning on the fallback.
fn provider() -> Provider {
    Provider::default().config(MapConfig::default().with([("ENV", "dev")]))
}

async fn post(provider: Provider, body: &'static [u8]) -> Response<Body> {
    let request = Request::post(PULSE_XML)
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(body))
        .expect("request");
    let router = pulse_connector::router().with_state(Client::new(OWNER, provider));
    router.oneshot(request).await.expect("response")
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

#[tokio::test]
async fn train_update() {
    let provider = provider();

    #[expect(
        clippy::disallowed_macros,
        reason = "integration test loads captured SOAP XML from disk"
    )]
    let xml = include_bytes!("../data/receive-message.xml");
    let request = PulseRequest::from_xml(xml).expect("should deserialize");
    let expected_payload = request.body.receive_message.axml_message;

    let response = post(provider.clone(), xml).await;

    // the connector acknowledges in the vendor's XML shape
    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let reply_xml = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(reply_xml.as_ref(), b"<Return>OK</Return>");

    // the embedded train update is forwarded verbatim
    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-pulse.v1");
    assert_eq!(record.payload, expected_payload.as_bytes());
}

#[tokio::test]
async fn without_train_update() {
    let provider = provider();

    let xml = br#"<?xml version="1.0" encoding="utf-8"?>
        <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
          <soap:Body>
            <ReceiveMessage>
              <AXMLMessage>not a train update</AXMLMessage>
            </ReceiveMessage>
          </soap:Body>
        </soap:Envelope>"#;

    let response = post(provider.clone(), xml).await;

    // the rejection is the vendor's SOAP fault envelope
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert_eq!(
        body.as_ref(),
        b"<Fault><StatusCode>400</StatusCode><Response><Message>Bad Request</Message></Response></Fault>"
    );
    assert!(provider.publish.sent().is_empty());
}

#[tokio::test]
async fn malformed_envelope() {
    let provider = provider();

    // parsing happens inside the handler, so even an unparseable body is
    // answered with the vendor's SOAP fault rather than the framework's JSON
    // `invalid_request` 400
    let response = post(provider.clone(), b"not xml at all").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let body = to_bytes(response.into_body(), usize::MAX).await.expect("body");
    assert!(String::from_utf8_lossy(&body).contains("<Fault>"));
    assert!(provider.publish.sent().is_empty());
}
