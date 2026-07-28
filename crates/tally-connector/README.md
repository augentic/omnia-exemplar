# Tally Connector

HTTP connector that accepts a Tally automatic passenger counting (APC) message
(`POST /api/apc`) and forwards it to the `{env}-realtime-tally-apc.v2` topic.

The downstream consumer of that topic (the passenger counting enrichment
pipeline) is out of scope for this exemplar; the crate demonstrates the
simplest possible ingress operation — validate, key, and publish. Use it as
the starting template for new connectors.

## Tests

- `tests/static.rs` — fixture messages (`data/`) forwarded through a mock
  publisher, asserting topic, key, and payload.
