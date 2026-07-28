# GTFS Adapter

The largest domain crate. Handles multiple message types for realtime train
position and passenger count data:

- **Motion messages** (`{env}-realtime-pulse-to-motion.v1`) — vehicle position
  and arrival/departure updates, published onward as GTFS-realtime vehicle
  positions (`{env}-realtime-gtfs-vp.v1`) or dead-reckoning estimates
  (`{env}-realtime-dead-reckoning.v1`).
- **Train AVL** (`{env}-realtime-train-avl.v1`) — Motion AVL events filtered
  to Motion-tagged train vehicles before standard processing.
- **Passenger count** (`{env}-realtime-passenger-count.v1`) — onboard
  passenger count events, stored as occupancy status.

Also provides an HTTP vehicle info lookup, and — behind the off-by-default
`god-mode` cargo feature — an operational override that forces a vehicle onto
a trip via the state store.

State lives under the `motionGtfs:` prefix; see `src/state_keys.rs` for the
key catalog.

## Tests

- `tests/static.rs` — captured Kafka fixtures (`data/`) replayed through the
  operations against an in-memory mock provider (`tests/provider.rs`),
  covering vehicle positions, dead reckoning, sign-on, tag filtering,
  occupancy status, and vehicle info.
- `tests/god_mode.rs` — the feature-gated override, disabled and enabled
  (run with `--features god-mode`).
