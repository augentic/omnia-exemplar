# Pulse Position Adapter

The Pulse Position Adapter transforms Pulse data for specific stations into
Motion events. The transformed events are published to the
`realtime-pulse-to-motion.v1` topic for downstream processing by the
GTFS adapter.

Each event is published `PUBLISH_REPEATS` times because the downstream
schedule-adherence process relies on the repeat to detect departures — see
the note on `PUBLISH_REPEATS` in `src/handler.rs` before copying that
behaviour.

## Tests

- `tests/static.rs` — hand-authored scenarios exercising the stops-table
  cache (fetch, hit, and refetch after corruption), the arrival location,
  unmapped stations, missing vehicles and stops, and validation failures.
- `tests/replay.rs` — snapshot fixtures captured from a live system, replayed
  through the adapter with the captured HTTP answers seeded into
  `omnia_test::guest::MatchedHttp` (`tests/fixture/mod.rs` is the loader; each
  fixture names the exact request it answers, query string included).
