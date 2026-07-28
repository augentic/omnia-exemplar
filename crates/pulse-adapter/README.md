# Pulse Position Adapter

The Pulse Position Adapter transforms Pulse data for specific stations into
Motion events. The transformed events are published to the
`{env}-realtime-pulse-to-motion.v1` topic for downstream processing by the
GTFS adapter.

## Tests

The crate carries two native test suites:

- `tests/static.rs` — hand-authored scenarios exercising arrival/departure
  handling, unmapped stations, and validation failures.
- `tests/replay.rs` — snapshot fixtures captured from a live system, replayed
  through the adapter with mocked HTTP responses (`augentic-test`).
