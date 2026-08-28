//! Static tests for the Pulse SOAP/XML connector.

mod provider;

use omnia_guest::api::{Client, Metadata};
use pulse_connector::{PulseRequest, PulseXml};

use self::provider::MockProvider;

const OWNER: &str = "acme";

#[tokio::test]
async fn forwards_train_update_to_pulse_topic() {
    let provider = MockProvider::new();

    let xml = include_bytes!("../data/receive-message.xml");
    let request = PulseRequest::from_xml(xml).expect("should deserialize");
    let expected_payload = request.body.receive_message.axml_message;

    let reply = Client::new(OWNER, provider.clone())
        .call(PulseXml(xml.to_vec()), &Metadata::default())
        .await
        .expect("should succeed");

    // the connector acknowledges in the vendor's XML shape
    let reply_xml = reply.to_xml().expect("should serialize");
    assert_eq!(reply_xml, b"<Return>OK</Return>");

    // the embedded train update is forwarded verbatim
    let published = provider.published();
    assert_eq!(published.len(), 1);

    let (topic, record) = &published[0];
    assert_eq!(topic, "dev-realtime-pulse.v1");
    assert_eq!(record.payload, expected_payload.as_bytes());
}

#[tokio::test]
async fn rejects_message_without_train_update() {
    let provider = MockProvider::new();

    let xml = br#"<?xml version="1.0" encoding="utf-8"?>
        <soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
          <soap:Body>
            <ReceiveMessage>
              <AXMLMessage>not a train update</AXMLMessage>
            </ReceiveMessage>
          </soap:Body>
        </soap:Envelope>"#;

    let error = Client::new(OWNER, provider.clone())
        .call(PulseXml(xml.to_vec()), &Metadata::default())
        .await
        .expect_err("should reject a message without a train update");

    // the rejection is the vendor's SOAP fault envelope
    assert_eq!(
        error.to_string(),
        "<Fault><StatusCode>400</StatusCode><Response><Message>Bad Request</Message></Response></Fault>"
    );
    assert!(provider.published().is_empty());
}

#[tokio::test]
async fn rejects_malformed_envelope() {
    let provider = MockProvider::new();

    // parsing happens inside the handler, so even an unparseable body is
    // answered with the vendor's SOAP fault rather than a plain-text 400
    let error = Client::new(OWNER, provider.clone())
        .call(PulseXml(b"not xml at all".to_vec()), &Metadata::default())
        .await
        .expect_err("should reject a malformed envelope");

    assert!(error.to_string().contains("<Fault>"));
    assert!(provider.published().is_empty());
}
