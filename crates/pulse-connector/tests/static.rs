//! Static tests for the Pulse SOAP/XML connector driven through its component.

use bytes::Bytes;
use common::topic;
use http::header::CONTENT_TYPE;
use http::{Request, Response, StatusCode};
use omnia::{Provides, Runtime};
use omnia_test::host::{Backends, Deployment, Sink, precompiled, send};
use omnia_wasi_http::WasiHttp;
use omnia_wasi_messaging::WasiMessaging;
use omnia_wasi_otel::WasiOtel;

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

const RECEIVE_MESSAGE: &[u8] = include_bytes!("../data/receive-message.xml");

const NO_TRAIN_UPDATE: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
    <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
      <soap:Body>
        <ReceiveMessage>
          <AXMLMessage>not a train update</AXMLMessage>
        </ReceiveMessage>
      </soap:Body>
    </soap:Envelope>"#;

async fn boot<B>(backends: B) -> Runtime<B>
where
    B: Provides<WasiHttp> + Provides<WasiMessaging> + Provides<WasiOtel>,
{
    let guest = precompiled(PULSE_CONNECTOR_GUEST).expect("guest precompiles");
    Deployment::new()
        .guest("pulse-connector", guest)
        .boot(backends, |deployment| {
            deployment.host::<WasiHttp>()?;
            deployment.host::<WasiMessaging>()?;
            deployment.host::<WasiOtel>()?;
            Ok(())
        })
        .await
        .expect("runtime boots")
}

async fn post<B: Provides<WasiHttp>>(runtime: &Runtime<B>, body: &'static [u8]) -> Response<Bytes> {
    let request = Request::post("/inbound/xml")
        .header(CONTENT_TYPE, "text/xml")
        .body(Bytes::from_static(body))
        .expect("request");
    send(runtime, request).await.expect("request served")
}

fn content_type(response: &Response<Bytes>) -> &str {
    response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default()
}

#[tokio::test]
async fn train_update() {
    let sink = Sink::default();
    let runtime = boot(Backends::defaults().await.messaging(sink.clone())).await;

    let response = post(&runtime, RECEIVE_MESSAGE).await;
    runtime.shutdown();

    // the connector acknowledges in the vendor's XML shape
    assert_eq!(response.status(), StatusCode::OK);
    assert!(content_type(&response).starts_with("text/xml"), "{:?}", content_type(&response));
    let reply_xml = response.into_body();
    assert_eq!(reply_xml.as_ref(), b"<Return>OK</Return>");

    // the fixture's CDATA train update is forwarded verbatim
    let fixture = std::str::from_utf8(RECEIVE_MESSAGE).expect("fixture is UTF-8");
    let (_, cdata) = fixture.split_once("<![CDATA[").expect("fixture opens a CDATA section");
    let (expected, _) = cdata.split_once("]]>").expect("fixture closes the CDATA section");

    let [forwarded] = sink.sent().try_into().expect("one message forwarded");
    assert_eq!(forwarded.topic, topic::PULSE);
    let forwarded = std::str::from_utf8(&forwarded.payload).expect("forwarded payload is UTF-8");
    assert!(forwarded.contains("<ActualizarDatosTren>"), "{forwarded}");
    assert_eq!(forwarded.trim(), expected.trim());
}

#[tokio::test]
async fn bad_requests() {
    let sink = Sink::default();
    let runtime = boot(Backends::defaults().await.messaging(sink.clone())).await;

    // parsing happens inside the handler, so an unparseable body is answered
    // with the same SOAP fault as a parsed one, not an extractor's plain-text
    // 400
    for body in [NO_TRAIN_UPDATE, b"not xml at all".as_slice()] {
        let request = String::from_utf8_lossy(body);
        let response = post(&runtime, body).await;

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
        assert!(sink.sent().is_empty(), "{request}");
    }
    runtime.shutdown();
}
