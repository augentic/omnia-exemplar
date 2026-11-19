# pattern

Composition patterns over the Omnia host interfaces, distilled from real
replatformed services. Where `capability` proves one interface at a time,
each handler here composes several:

| Module | Interfaces | Handler |
| --- | --- | --- |
| `decode` | `config` + `http` + `keyvalue` | `decode_segment` — decode-through-cache with a config-carried client certificate |
| `cas` | `keyvalue` (`Bucket::compare_and_swap`) | `update_quota` — optimistic concurrency over `Bucket::compare_and_swap`, answering `409 Conflict` via `QuotaError` |
| `identity` | `config` + `identity` + `http` | `fetch_upstream_status` — resolving an upstream identity via `IdentitySettings`, acquiring a bearer token via `identity::open` and `Identity::access_token`, and forwarding `Authorization: Bearer <token>` |
| `place` | `sql` | `upsert_place` — parameterized `INSERT … ON CONFLICT` upsert, rejecting bad coordinates with a structured JSON error body (`PlaceError`) |
| `place` | `sql` + `FromRow` | `nearby_places` — bounding-box `SELECT` refined by haversine |

The crate's `handlers()` serves these under `/examples/patterns/*`. The
routes are pedagogical: they exist to exercise the composed default host
implementations, deliberately apart from the transit crates' paths.

## Decode-through-cache

`DecodeSegmentRequest` is the canonical interface-composition vignette:

1. `Bucket::get_json` on the `default` bucket — on a hit, return
   immediately: no config read, no outbound request.
2. `DecodeSettings::load` — resolve the decoder endpoint
   (`PATTERN_DECODER_URL`) and the client certificate
   (`PATTERN_CLIENT_CERT`).
3. `omnia_sdk::http::fetch_json` — POST to the decoder with the certificate
   riding the `Client-Cert` header.
4. `Bucket::set_json` — write the decoded segment back with a `Duration`
   TTL so the next lookup short-circuits.

The segment code becomes a state key segment (`pattern.segment.{code}`), so a
code that is not portable is refused with `400` before the cache is read.

Two things to copy: expensive lookups go *through* the cache rather than
being populated by a separate process, and credential material stays in
configuration and travels as ordinary request data — outbound HTTP stays
generic and no TLS-specific interface is needed.

## Optimistic concurrency (`Bucket::compare_and_swap`)

`update_quota` (`PUT /examples/patterns/quotas/{tenant}` with an
`UpdateQuotaRequest` body) reads the current record with `Bucket::read`,
checks `expected_version`, and writes the next revision with
`Bucket::compare_and_swap` against the bytes it read.
When a concurrent writer modifies the entry (or the version does not match),
`QuotaError::Conflict` answers `409 Conflict` through `QuotaError`'s
`IntoResponse`, carrying the `tenant` and the observed `current_version` so the caller can
retry without a separate read. A tenant that is not a portable key segment
is refused with `400` before the store is read.

## Upstream bearer-token authentication (`Identity::access_token`)

`fetch_upstream_status` loads the identity name (`PATTERN_UPSTREAM_IDENTITY`)
and base URL (`PATTERN_UPSTREAM_URL`) through `IdentitySettings`, calls
`identity::open(&identity).await?.access_token()` to obtain the bearer
token, and sends `Authorization: Bearer <token>` on the outbound
`omnia_sdk::http::fetch_json` call, which answers a non-2xx upstream
response with `502`.

## Relational geo queries — no `GEORADIUS`

The key-value store is a cache, not a geospatial index. When a service
needs "what is within this radius?", do not reach for a geo extension bolted
onto the KV store; model the rows relationally and let the database do what
it is good at:

1. Over-approximate the radius with a degree bounding box — four plain,
   indexable comparisons in a parameterized `SELECT`.
2. Read the rows as `Place` records (`#[derive(FromRow)]`) through
   `Database::query_as` and refine the survivors with a haversine check
   in Rust.

`UpsertPlaceRequest` writes the rows with a parameterized
`INSERT INTO places ... ON CONFLICT (id) DO UPDATE SET ...` statement;
`NearbyPlacesRequest` runs the query. Workloads that outgrow this pattern
want a real geospatial backend (e.g. PostGIS behind its own handler), not a
richer key-value store.

## Extending the framework's JSON error envelope

A handler's error type implements `IntoResponse`, and that implementation
alone decides the wire shape. The default `omnia_sdk::Error` renders as
`omnia_sdk::ErrorBody` — an `application/json` body with two fields,
`error` (the code discriminant) and `message` (the description) — with no
room for domain data or status codes outside `400`, `404`, `500`, and `502`.

`upsert_place` and `update_quota` demonstrate the structured alternative.
Each handler returns its own error type (`Result<Json<UpsertPlaceReply>,
PlaceError>`, `Result<Json<QuotaRecord>, QuotaError>`), and its
`IntoResponse` serializes a superset of `ErrorBody` as `application/json`:
`error` is the discriminant, `Display` supplies
`message`, and the variant's domain fields ride alongside, so a client that
only knows the framework envelope still parses the rejection, and one that
knows the route reads the extra fields:

```json
{ "error": "invalid_coordinate", "message": "lat 123.4 is outside [-90, 90]", "field": "lat", "value": 123.4, "min": -90.0, "max": 90.0 }
{ "error": "conflict", "message": "quota for `metro` changed concurrently (current version: Some(2))", "tenant": "metro", "current_version": 2 }
```

Two things to note:

- Nothing route-side changes: the handler is bound with the same
  `post` / `put` as one answering `omnia_sdk::Error`, and the custom body
  comes from the error type alone.
- Extractor rejections (a malformed JSON body, a missing `Content-Type`)
  are answered by axum before the handler runs, as plain-text `400`,
  `415`, or `422`; the structured body covers failures raised by the
  handler itself.

## Spy backend tests

The crate-level tests (`tests/`) drive every route through the stem's
component (`guest.rs`, compiled by `build.rs`) over `omnia_test::host`
backends that *record* outbound HTTP requests and SQL statements as well as
answering them. `MatchedHttp` answers only the exact method and URL a test
seeds, so the tests assert on the `Client-Cert` or `Authorization` header and
on the exact number of calls, which is how the cache tests prove a hit never
reaches HTTP. Recording in the host backend is the cheapest way to verify
*what left the guest* without a real network.
