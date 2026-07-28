# Omnia Exemplar

A reference implementation for building [omnia](https://github.com/augentic/omnia)
services. It models a fictional transit operator ("Acme") that consolidates
realtime vehicle data processing into WASM guests: SOAP/XML position feeds,
passenger counting, and GTFS realtime adaptation.

The same domain logic is exposed through **two guest styles, side by side**, so
you can compare the typed `omnia-guest` API with hand-written Axum handlers and
pick the style that fits your service.

## Quick start

```shell
# build both guests
cargo build --target wasm32-wasip2 --release

# run one of them with its example host runtime
cp guests/typed/examples/.env.example .env   # then fill in values
set -a; source .env; set +a
cargo run -p guest-typed --example runner -- run target/wasm32-wasip2/release/guest_typed.wasm
```

Substitute `guest-axum` / `guest_axum.wasm` to run the Axum-style guest — both
serve identical routes and topics.

## The two guest styles

### Style A — typed routers (`guests/typed`)

Routes bind an `Operation` directly to a path or topic; the router handles
transport decoding, invocation, and response projection:

- `omnia_guest::api::http::Router` with `get::<Op, P>()` / `post::<Op, P>()`
- `omnia_guest::api::messaging::Router` with `consume::<Op>()`, plus
  `decode_with` for non-JSON payloads (the Pulse XML feed)
- Non-JSON HTTP ingress drops down to the underlying Axum router
  (`router.into_axum()`) for a single hand-written route

Use this style when your endpoints map one-to-one onto operations and payloads
are (mostly) JSON. It is the least code and the hardest to get wrong.

### Style B — plain Axum (`guests/axum`)

Hand-written Axum handlers served through `omnia_wasi_http::serve`, and a raw
`incoming-handler` messaging export that matches on topic. Each handler decodes
its own payload and invokes the shared operation through an `Invoker`.

Use this style when you need full control over the transport layer: custom
extractors, middleware, response shaping, or messaging dispatch that doesn't
fit the typed router.

### What both share

- Domain logic lives in `crates/*` as `Operation<P>` implementations; the
  guests are thin routing layers over the same operations.
- A unit `Provider` struct with the WASI-backed default capability
  implementations (`Config`, `HttpRequest`, `Identity`, `Publish`,
  `StateStore`).
- `wasip3::http::service::export!` + `omnia_wasi_messaging::export!` exports,
  with `#[omnia_wasi_otel::instrument]` on the entry handlers.
- A native host-runner example (`examples/runner.rs`) built with
  `omnia::runtime!`.

## Routes and topics

| HTTP route | Operation |
| --- | --- |
| `POST /api/apc` | `tally_connector::TallyRequest` — passenger-count ingress |
| `POST /inbound/xml` | `pulse_connector::PulseRequest` — SOAP/XML position ingress |
| `GET /info/{vehicle_id}` | `gtfs_adapter::VehicleInfoRequest` |
| `GET /god-mode/set-trip/{vehicle_id}/{trip_id}` | `gtfs_adapter::SetTripRequest` |

| Messaging topic | Operation |
| --- | --- |
| `{env}-realtime-pulse.v1` | `pulse_adapter::PulseMessage` (XML) |
| `{env}-realtime-pulse-to-motion.v1` | `gtfs_adapter::MotionMessage` |
| `{env}-realtime-train-avl.v1` | `gtfs_adapter::TrainAvlMessage` |
| `{env}-realtime-passenger-count.v1` | `gtfs_adapter::PassengerCountMessage` |

`POST /api/apc` publishes to `{env}-realtime-tally-apc.v2`; its downstream
consumer is out of scope for the exemplar.

## Crate map

| Crate | Role |
| --- | --- |
| `crates/common` | Shared clients for the Block Management and Fleet APIs |
| `crates/pulse-connector` | HTTP ingress for the vendor "Pulse" SOAP/XML position feed; republishes to messaging |
| `crates/pulse-adapter` | Converts Pulse train updates into Motion location events |
| `crates/tally-connector` | HTTP ingress for the vendor "Tally" passenger-count feed |
| `crates/gtfs-adapter` | Converts Motion events into GTFS-realtime vehicle positions |
| `guests/typed` | Style A guest binary |
| `guests/axum` | Style B guest binary |

Domain crates depend only on the `omnia-guest` capability traits (`Config`,
`HttpRequest`, `Identity`, `Publish`, `StateStore`), so the same code runs
inside the WASM guest and against native mock providers in tests.

## Testing

```shell
cargo nextest run            # or: cargo test --workspace
```

- `crates/tally-connector/tests` — mock-provider tests invoking operations natively
- `crates/pulse-adapter/tests` — static fixtures plus `augentic-test` replay
  sessions captured from a live system (`data/replay`, `data/static`)

## Development

```shell
make            # fmt, clippy, test, docs, vet, deny — same targets as omnia
```

The workspace follows omnia's conventions: stable toolchain
(`rust-toolchain.toml`, with the `wasm32-wasip2` target), edition 2024,
workspace lints, `cargo vet` supply-chain audits (`supply-chain/`), and CI as
thin wrappers over the reusable workflows in `augentic/.github`.

The omnia crates are currently resolved from the GitHub monorepo via the
`[patch.crates-io]` section in `Cargo.toml`, pending publication to a public
registry.
