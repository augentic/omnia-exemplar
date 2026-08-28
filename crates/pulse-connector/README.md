# Pulse HTTP Connector

HTTP connector that receives Pulse SOAP/XML data (`POST /inbound/xml`) and
forwards the embedded train update to the `{env}-realtime-pulse.v1` topic.

Pulse data is received from track-side sensors that are triggered when a train
passes. This position data is used to help improve train location information
in underground stations (where GPS is not available).

Rejections are returned as a pre-rendered SOAP fault because the vendor
protocol requires an XML fault envelope — a vendor accommodation, not a
general error-handling pattern. The handler's error type is the `Fault`
envelope itself, and its `HttpError` conversion puts the serialized XML on
the wire via `HttpError::with_body` (`text/xml`, HTTP status from the
fault). The HTTP route passes the body through undecoded so that parse
failures are also answered with the fault envelope — a decoder failing in
the route codec would reach the client as the framework's plain-text 400
instead.

## Tests

- `tests/static.rs` — the captured vendor envelope (`data/receive-message.xml`)
  forwarded through a mock publisher, plus the SOAP fault sad paths
  (invalid message and unparseable envelope).
