//! Static tests for the Pulse SOAP/XML connector driven through its router.

use axum::body::{Body, Bytes};
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, Response, StatusCode};
use common::topic;
use omnia_sdk::api::Client;
use omnia_test::guest::{Provider, send};

const RECEIVE_MESSAGE: &[u8] = include_bytes!("../data/receive-message.xml");

const NO_TRAIN_UPDATE: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
    <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
      <soap:Body>
        <ReceiveMessage>
          <AXMLMessage>not a train update</AXMLMessage>
        </ReceiveMessage>
      </soap:Body>
    </soap:Envelope>"#;

async fn post(provider: Provider, body: &'static [u8]) -> Response<Bytes> {
    let request = Request::post("/inbound/xml")
        .header(CONTENT_TYPE, "text/xml")
        .body(Body::from(body))
        .expect("request");
    let router = pulse_connector::handlers().with_state(Client::new(provider));
    send(&router, request).await
}

fn content_type(response: &Response<Bytes>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

#[tokio::test]
async fn train_update() {
    let provider = Provider::default();

    let response = post(provider.clone(), RECEIVE_MESSAGE).await;

    // the connector acknowledges in the vendor's XML shape
    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let reply_xml = response.into_body();
    assert_eq!(reply_xml.as_ref(), b"<Return>OK</Return>");

    // the fixture's CDATA train update is forwarded verbatim
    let fixture = std::str::from_utf8(RECEIVE_MESSAGE).expect("fixture is UTF-8");
    let (_, cdata) = fixture.split_once("<![CDATA[").expect("fixture opens a CDATA section");
    let (expected, _) = cdata.split_once("]]>").expect("fixture closes the CDATA section");

    let published = provider.publish.sent();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, topic::PULSE);
    let forwarded = std::str::from_utf8(&record.payload).expect("forwarded payload is UTF-8");
    assert!(forwarded.contains("<ActualizarDatosTren>"), "{forwarded}");
    assert_eq!(forwarded.trim(), expected.trim());
}

#[tokio::test]
async fn bad_requests() {
    let provider = Provider::default();

    // parsing happens inside the handler, so an unparseable body is answered
    // with the same SOAP fault as a parsed one, not an extractor's plain-text
    // 400
    for body in [NO_TRAIN_UPDATE, b"not xml at all".as_slice()] {
        let request = String::from_utf8_lossy(body);
        let response = post(provider.clone(), body).await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{request}");
        assert!(
            content_type(&response).starts_with("text/xml"),
            "{request}: {:?}",
            content_type(&response)
        );
        assert_eq!(
            response.into_body().as_ref(),
            b"<Fault><StatusCode>400</StatusCode><Response><Message>Bad Request</Message></Response></Fault>",
            "{request}"
        );
        assert!(provider.publish.sent().is_empty(), "{request}");
    }
}
