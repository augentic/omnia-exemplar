# Omnia Exemplar

A reference implementation for building [omnia](https://github.com/augentic/omnia)
services. It models a fictional transit operator ("Acme") that consolidates
realtime vehicle data processing into a WASM guest: SOAP/XML position feeds,
passenger counting, and GTFS realtime adaptation.

Domain logic lives under `crates/*` as `#[omnia_guest::handler]` functions
(each derives its `Handler<P>` impl). The **guest is the workspace root
package** (`src/lib.rs`) — explicit typed HTTP routes and an exact-topic
messaging router over a provider-owning `Client`. Match this layout when
creating a new Omnia service: root `src/`, not a `guests/<name>/` tree.

## Relation to omnia

This repository is the application-scale complement to the
[omnia](https://github.com/augentic/omnia) runtime's per-capability
[`examples/`](https://github.com/augentic/omnia/tree/main/examples): one real
service instead of twenty snippets. The guest is built on the `omnia-guest`
SDK (`Handler<P>` domain logic behind capability traits) and exercises
`wasi:http`, `wasi:messaging`, `wasi:keyvalue`, `wasi:config`,
`wasi:identity`, and — via `crates/capability-examples` — blobstore,
websocket broadcast, docstore, and SQL. Two capabilities get a deeper
treatment: `crates/docstore-examples` and `crates/sql-examples` rebuild the
rich docstore and SQL examples that omnia trimmed from its own
`examples/` tree (commit `c4666ca`, "Example tidy"), rewritten as typed
handlers — every portable docstore filter type, every ORM builder, a
JOIN entity, and server-assigned ids. The example host
(`examples/runtime.rs`) is a single `omnia::runtime!` invocation running
against the in-tree default backends; swapping in production backends from
[omnia-backends](https://github.com/augentic/omnia-backends) is a host-side
change only (see the omnia
[Production Backends guide](https://github.com/augentic/omnia/blob/main/docs/guides/production-backends.md)).
The omnia crates are resolved from the GitHub monorepo via
`[patch.crates-io]` in `Cargo.toml`; `Cargo.lock` records the exact
revision.

## Quick start

```shell
# build the guest
cargo build --target wasm32-wasip2 --release

# run it with the example host runtime
cp .env.example .env   # then fill in values
set -a; source .env; set +a
cargo run --example runtime -- run target/wasm32-wasip2/release/guest.wasm
```

## Architecture

```mermaid
flowchart LR
  subgraph ingress [HTTP ingress]
    apc["POST /api/apc"]
    xml["POST /inbound/xml"]
    info["GET /info/{vehicle_id}"]
  end

  apc --> tallyConn[tally-connector]
  xml --> pulseConn[pulse-connector]

  tallyConn -->|"{env}-realtime-tally-apc.v2"| outOfScope["downstream (out of scope)"]
  pulseConn -->|"{env}-realtime-pulse.v1"| pulseAdapter[pulse-adapter]
  pulseAdapter -->|"{env}-realtime-pulse-to-motion.v1"| gtfsAdapter[gtfs-adapter]
  trainAvl["{env}-realtime-train-avl.v1"] --> gtfsAdapter
  paxCount["{env}-realtime-passenger-count.v1"] --> gtfsAdapter
  gtfsAdapter -->|"{env}-realtime-gtfs-vp.v1"| vp[vehicle positions]
  gtfsAdapter -->|"{env}-realtime-dead-reckoning.v1"| dr[dead reckoning]
  info --> gtfsAdapter
```

Two shapes of service repeat throughout the pipeline:

- **Connector** — thin ingress: decode the transport payload, validate it,
  key it, and publish it to a topic. No enrichment, no state.
  `tally-connector` is the minimal template; `pulse-connector` adds a
  vendor-specific (SOAP/XML) transport.
- **Adapter** — domain transformation: validate, enrich via upstream APIs,
  maintain state, and publish derived events. `pulse-adapter` is a compact
  example; `gtfs-adapter` is the full-size one.

Start a new service from the closest template.

## Guest packaging

The root `Cargo.toml` is both the workspace and the deployable guest package
(`name = "guest"`, `crate-type = ["cdylib"]`). Workspace members are
`crates/*` (plus `templates/check` for the template gate). There is no
`guests/` directory.

The guest:

- Registers typed HTTP routes (`omnia_guest::api::http::{get, post}`, with
  `handle_with` for non-JSON wire formats) on an `axum::Router` and
  serves them through `omnia_guest::api::http::serve`
- Exports messaging with `omnia_wasi_messaging::export!`, dispatching through
  an exact-topic `omnia_guest::api::messaging::Router` of **exact**
  env-qualified topics
- Uses a unit `Provider` declared with `omnia_guest::provider!`, giving it
  the WASI-backed default capability impls (`Config`, `HttpRequest`,
  `Identity`, `Publish`, `StateStore`)
- Routes both transports through one provider-owning `Client` per request
- Ships a native host example at `examples/runtime.rs` via `omnia::runtime!`

## Routes and topics

| HTTP route | Handler |
| --- | --- |
| `POST /api/apc` | `tally_connector::TallyRequest` — passenger-count ingress |
| `POST /inbound/xml` | `pulse_connector::PulseXml` — SOAP/XML position ingress |
| `GET /info/{vehicle_id}` | `gtfs_adapter::VehicleInfoRequest` |
| `POST /god-mode/set-trip/{vehicle_id}/{trip_id}` | `gtfs_adapter::SetTripRequest` (requires the `god-mode` feature) |
| `GET/POST /examples/stops`, `GET/PUT/DELETE /examples/stops/{id}` | `docstore_examples::stop` — docstore CRUD plus filtered queries |
| `GET/POST /examples/routes`, `GET /examples/routes/{id}` | `docstore_examples::route` — OR / `in_list` / negation filters |
| `GET/POST /examples/stop-times`, `GET /examples/stop-times/{id}` | `docstore_examples::stop_time` — string and numeric range filters |
| `GET/POST /examples/agencies`, `GET/PATCH /examples/agencies/{id}` | `sql_examples::agency` — ORM CRUD with server-assigned ids |
| `GET/POST /examples/agencies/{agency_id}/feeds` | `sql_examples::feed` — per-agency feeds with referential checks |
| `GET /examples/feeds`, `DELETE /examples/feeds/{id}` | `sql_examples::feed` — JOIN listing and delete with 404-on-zero-rows |

| Messaging topic | Handler |
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
| `crates/common` (`acme-common`) | Config key catalog, canonical route/topic tables, and shared clients for the Block Management and Fleet APIs |
| `crates/pulse-connector` | HTTP ingress for the vendor "Pulse" SOAP/XML position feed; republishes to messaging |
| `crates/pulse-adapter` | Converts Pulse train updates into Motion location events |
| `crates/tally-connector` | HTTP ingress for the vendor "Tally" passenger-count feed |
| `crates/gtfs-adapter` | Converts Motion events into GTFS-realtime vehicle positions |
| `crates/capability-examples` | Domain-free handlers proving the remaining capabilities: `BlobStore`, `Broadcast`, `DocumentStore`, `TableStore` |
| `crates/docstore-examples` | Rich `wasi:docstore` showcase: GTFS-like collections exercising every portable filter type, sorting, and continuation pagination |
| `crates/sql-examples` | Rich `wasi-sql` showcase: agency/feed schema exercising every ORM builder, a JOIN entity, and server-assigned ids |
| `crates/pattern-examples` | Composition patterns: decode-through-cache, config-carried client certificates, relational geo queries through the ORM, and structured JSON error bodies |
| root package (`guest`) | Typed-router HTTP + exact-topic messaging WASM guest binary |

Domain crates depend only on the `omnia-guest` capability traits (`Config`,
`HttpRequest`, `Identity`, `Publish`, `StateStore`; `capability-examples`
covers `BlobStore`, `Broadcast`, `DocumentStore`, and `TableStore`), so the
same code runs inside the WASM guest and against `omnia_test::provider!`
doubles in tests.

## Adding a new handler

1. Pick a template: `tally-connector` for a connector, `pulse-adapter` for an
   adapter.
2. Create the crate under `crates/`, register it in the workspace
   `Cargo.toml` under `# Internally referenced crates`.
3. Implement the input type and an `#[omnia_guest::handler]` fn
   (`async fn name<P>(input: Input, context: Context<'_, P>) -> Result<Reply>`)
   with the narrowest capability bounds it needs
   (e.g. `P: Config + Publish`).
4. Add any new topic suffix or HTTP path to `acme_common::routes`, and any
   new configuration key to `acme_common::config` plus `.env.example`.
5. Wire the handler into the HTTP or messaging router in `src/lib.rs` from
   the shared route tables (`post::<Input, Provider>()`, `consume::<Input>()`,
   or a `_with` variant for custom wire formats).
6. Add fixtures under the crate's `data/` and native tests under `tests/`
   with an `omnia_test::provider!` declaration over the handler's
   capabilities (copy the shape of `crates/tally-connector/tests` or
   `crates/gtfs-adapter/tests`), then add the route to `tests/routes.rs` or
   the topic to `tests/messaging.rs`.
7. Run `make ci` — fmt, clippy (native + wasm), tests, docs, vet, deny.

## Configuration

All keys are declared as constants in `acme_common::config`; `.env.example`
carries sample values.

| Key | Used by | Purpose |
| --- | --- | --- |
| `ENV` | all publishers/consumers | Deployment environment; prefixes every topic (`dev-…`). Defaults to `dev` with a warning when unset |
| `BLOCK_MGT_URL` | gtfs-adapter, pulse-adapter | Block Management API base URL |
| `FLEET_URL` | gtfs-adapter | Fleet API base URL |
| `TRIP_MANAGEMENT_URL` | gtfs-adapter | Trip Management API base URL |
| `STATIC_API_URL` | pulse-adapter | Static GTFS API base URL |
| `API_IDENTITY` | gtfs-adapter, pulse-adapter | Identity used to acquire access tokens for the operator APIs |
| `GOD_MODE_ENABLED` | gtfs-adapter | Runtime switch for the god-mode override (also requires the `god-mode` build feature) |
| `PATTERN_DECODER_URL` | pattern-examples | Decoder endpoint for the decode-through-cache example |
| `PATTERN_CLIENT_CERT` | pattern-examples | Client certificate forwarded to the decoder as a `Client-Cert` header |

## State keys

The gtfs-adapter keeps its pipeline state under the `motionGtfs:` prefix,
namespaced by purpose then identifier — see `crates/gtfs-adapter/src/state_keys.rs`
for the full catalog and key builders. God-mode overrides live under a
separate `god_mode:` prefix so operational overrides never mix with pipeline
state.

## Copy this, not that

Patterns worth copying into new services:

- `Handler<P>` domain logic behind capability traits, with the guest as a
  thin routing layer at the **workspace root** (`src/lib.rs`).
- One canonical route/topic table (`acme_common::routes`) consumed by
  producers, consumers, and the guest.
- Named config keys with a single documented resolution policy
  (`acme_common::config`).
- Native tests under `omnia_test::provider!` doubles plus captured fixtures
  (`tests/` + `data/` in each crate), and a route rung per router at the
  root (`tests/routes.rs`, `tests/messaging.rs`).
- Decode-through-cache: expensive lookups go through `StateStore` in one
  handler — miss → `Config` → `HttpRequest` → write back with a TTL —
  instead of a separate cache-population process
  (`pattern_examples::decode`).
- Credential material in `Config`, carried as ordinary request data (the
  `Client-Cert` header), so outbound HTTP stays generic.
- Recording doubles: `MatchedHttp` answers only the exact requests a test
  seeds and records what left the guest — shape, headers, and call count
  (`crates/pattern-examples/tests/decode.rs`).
- Radius queries as bounding-box `SELECT`s through `TableStore` and the
  ORM, refined by haversine in Rust — never a geospatial extension bolted
  onto the KV state store (`pattern_examples::place`).
- Structured JSON error bodies: the handler owns its error type and its
  `From<…> for HttpError` conversion serializes it as `application/json`
  via `HttpError::with_body`, so errors match the route's success content
  type instead of the default plain-text `code: …, description: …` body
  (`pattern_examples::place::PlaceError`).

Acme domain quirks that are **not** general patterns:

- **Duplicate publish** — `pulse-adapter` publishes each Motion event twice
  (`PUBLISH_REPEATS`) because a downstream schedule-adherence process needs
  the repeat. The legacy system also spaced the repeats with a blocking
  sleep; that was removed — never block a WASM guest.
- **God-mode** — an operational override tool, off by default behind the
  `god-mode` cargo feature and the `GOD_MODE_ENABLED` config key. Not part of
  a production pipeline.
- **SOAP fault as the handler error** — `pulse-connector` rejects requests
  with a pre-rendered XML `<Fault>` (`HttpError::with_body`, `text/xml`)
  because the vendor protocol demands it, and parses the envelope inside the
  handler so even malformed bodies get the fault. Prefer plain structured
  errors unless a wire protocol dictates otherwise.
- **UTC as the local timezone** — `acme_common::TIMEZONE` is UTC to keep
  fixtures reproducible. A real operator sets its actual IANA zone.
- **Legacy reply fields** — `VehicleInfoReply::pid` and
  `SetTripReply::process` are always `0`, retained only to match the legacy
  reply shapes.

## Testing

```shell
cargo nextest run            # or: cargo test --workspace --all-features
```

Every crate declares its test provider with one `omnia_test::provider!`
line over the capabilities its handlers name, and seeds the doubles
`omnia_test::guest` exports — `MapConfig`, `Sink`, `MatchedHttp`, `Memory`,
`MemoryDocs`, `ScriptedTables`, `FixedIdentity`. There is no hand-written
mock provider anywhere in the workspace.

- `tests/routes.rs`, `tests/messaging.rs` — the route and messaging rungs:
  the root guest's production routers driven natively (`oneshot` and
  `Router::handle`) under the production capability list as doubles
- `crates/tally-connector/tests` — the minimal handler rung
- `crates/pulse-connector/tests` — SOAP happy path and fault sad path
- `crates/gtfs-adapter/tests` — `MatchedHttp` seeded with the exact
  upstream URLs (`tests/support/`), covering motion, dead reckoning,
  sign-on, filtering, occupancy, and (feature-gated) god-mode
- `crates/pulse-adapter/tests` — static fixtures plus replay sessions
  captured from a live system (`data/replay`, `data/static`), loaded by
  `tests/fixture/` onto `MatchedHttp`
- `crates/capability-examples/tests` — one provider covering
  `BlobStore`/`Broadcast`/`DocumentStore`/`TableStore`
- `crates/docstore-examples/tests` — `MemoryDocs`, the filter-evaluating
  `DocumentStore` double, covering every portable filter type, sorting,
  continuation pagination, and the CRUD round-trip
- `crates/sql-examples/tests` — `ScriptedTables` scripting each ORM query's
  rows and recording every statement, covering all four ORM builders, the
  JOIN listing, server-assigned ids, and 404-on-zero-rows
- `crates/pattern-examples/tests` — `MatchedHttp` recording outbound HTTP
  requests, covering the decode cache (hit and miss), the ORM-backed nearby
  query, and the structured-error upsert rejection
- `templates/check/tests` — the template contract gate and the scaffold
  proof (a rendered guest builds for `wasm32-wasip2` and passes its test)

## Guest template contract

This repository is the source of truth for the reusable Omnia guest
tooling. [`templates/guest/manifest.yaml`](templates/guest/manifest.yaml)
maps repository files to consumer scaffold targets
([contract and authoring rules](templates/guest/README.md)).

The Emery omnia target adapter directs each consumer build to a fresh
checkout of `main` and reads the contract from that checkout at build
time: `exact` manifest entries are the repository-root files
themselves, and seed baselines live under `templates/guest/core/`.
There is no vendored or baked-in copy anywhere. **Merges to `main` are
therefore release acts**: the CI gate (including the template contract
check below) is required on merge, not advisory, because downstream
consumers track `main` unpinned.

The adapter pins an exact `schema-version` for
`templates/guest/manifest.yaml`. Bumping that version here requires a
coordinated adapter release, or consumer builds fail closed at the
scaffold prelude.

The contract is enforced by the `template-check` gate, which runs
inside the standard test suite and stand-alone:

```shell
cargo run -p template-check   # schema, tokens, path safety, seed render
```

`exact` entries reference their repository-root file in place
(`source == target`, token-free), so a green root vouches for the
scaffold with no diff to maintain. The `Cargo.toml`, `src/lib.rs` and
`tests/routes.rs` seeds give a new service the shape above — a
provider-generic router, `rlib` alongside `cdylib`, and a route rung
under `omnia_test::provider!` — and the suite's scaffold test renders
the manifest and builds and tests the result against the same omnia the
exemplar uses. To move to a new Omnia rev, update `Cargo.lock` (and any
explicit `rev` on the `[patch.crates-io]` git sources) and the seed's
pins, which the gate holds equal to the workspace's.

## Development

```shell
make ci         # fmt, clippy (native + wasm), test, docs, vet, deny — same targets as omnia
```

The workspace follows omnia's conventions: stable toolchain
(`rust-toolchain.toml`, with the `wasm32-wasip2` target), edition 2024,
workspace lints, `cargo vet` supply-chain audits (`supply-chain/`), and CI as
thin wrappers over the reusable workflows in `augentic/.github`.

The omnia crates are currently resolved from the GitHub monorepo via the
`[patch.crates-io]` section in `Cargo.toml`, pending publication to a public
registry.
