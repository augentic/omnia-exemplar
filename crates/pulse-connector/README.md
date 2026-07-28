# Pulse HTTP Connector

HTTP connector that receives Pulse SOAP/XML data (`POST /inbound/xml`) and
forwards the embedded train update to the `{env}-realtime-pulse.v1` topic.

Pulse data is received from track-side sensors that are triggered when a train
passes. This position data is used to help improve train location information
in underground stations (where GPS is not available).

Rejections are returned as a pre-rendered SOAP fault because the vendor
protocol requires an XML fault envelope — a vendor accommodation, not a
general error-handling pattern.

## Tests

- `tests/static.rs` — the captured vendor envelope (`data/receive-message.xml`)
  forwarded through a mock publisher, plus the SOAP fault sad path.
