# Tally Connector

HTTP connector that accepts a Tally automatic passenger counting (APC) message
(`POST /api/apc`) and forwards it to the `realtime-tally-apc.v2` topic.

The downstream consumer of that topic (the passenger counting enrichment
pipeline) is out of scope for this exemplar; the crate demonstrates the
simplest possible ingress operation — validate, key, and publish. Use it as
the starting template for new connectors.

## Tests

- `tests/static.rs` — fixture messages (`data/`) posted to the stem's
  component (`guest.rs`, compiled by `build.rs`) over `omnia_test::host`
  backends, asserting the topic, key, and payload each publish reaches the
  recording `Sink` with.
