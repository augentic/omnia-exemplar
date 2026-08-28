# pattern-examples

Composition patterns over the Omnia guest capabilities, distilled from real
replatformed services. Where `capability-examples` proves one capability at a
time, each handler here composes several:

| Module | Capabilities | Handler |
| --- | --- | --- |
| `decode` | `Config` + `HttpRequest` + `StateStore` | `DecodeSegmentRequest` — decode-through-cache with a config-carried client certificate |
| `place` | `TableStore` | `UpsertPlaceRequest` — ORM `INSERT … ON CONFLICT` upsert, rejecting bad coordinates with a structured JSON error body (`PlaceError`) |
| `place` | `TableStore` | `NearbyPlacesRequest` — bounding-box `SELECT` refined by haversine |

The guest serves these under `/examples/patterns/*` (see `src/routes.rs`).
The routes are pedagogical: they exist to instantiate the composed default
WASM capability implementations, deliberately outside the canonical transit
tables in `acme_common::routes`.

## Decode-through-cache

`DecodeSegmentRequest` is the canonical capability-composition vignette:

1. `StateStore::get` — on a hit, return immediately: no config read, no
   outbound request.
2. `Config::get` — resolve the decoder endpoint (`PATTERN_DECODER_URL`) and
   the client certificate (`PATTERN_CLIENT_CERT`).
3. `HttpRequest::fetch` — POST to the decoder with the certificate riding
   the `Client-Cert` header.
4. `StateStore::set` — write the decoded segment back with a TTL so the
   next lookup short-circuits.

Two things to copy: expensive lookups go *through* the cache rather than
being populated by a separate process, and credential material stays in
`Config` and travels as ordinary request data — outbound HTTP stays generic
and no TLS-specific capability is needed.

## Relational geo queries — no `GEORADIUS`

`StateStore` is a key-value cache, not a geospatial index. When a service
needs "what is within this radius?", do not reach for a geo extension bolted
onto the KV store; model the rows relationally and let the database do what
it is good at:

1. Over-approximate the radius with a degree bounding box — four plain,
   indexable comparisons built with `SelectBuilder` + `Filter`.
2. Refine the survivors with a haversine check in Rust.

`UpsertPlaceRequest` writes the rows with the ORM's `entity!` macro and
`InsertBuilder::on_conflict("id").do_update_all()`; `NearbyPlacesRequest`
runs the query. Workloads that outgrow this pattern want a real geospatial
backend (e.g. PostGIS behind its own handler), not a richer `StateStore`.

## Custom JSON error bodies

Handler failures never pass through a route's success encoder: the
handler's error type converts to `HttpError`, and that conversion alone
decides the wire shape. The default `omnia_guest::Error` renders as a
plain-text `code: …, description: …` body — even on JSON routes.

`UpsertPlaceRequest` demonstrates the structured alternative. The handler
returns its own error type (`Result<UpsertPlaceReply, PlaceError>` — the
`#[omnia_guest::handler]` macro accepts an explicit error), and a
`From<PlaceError> for HttpError` impl serializes it with
`HttpError::with_body`, so a rejected upsert answers in the same content
type as a successful one:

```json
{ "code": "invalid_coordinate", "field": "lat", "value": 123.4, "min": -90.0, "max": 90.0 }
```

Two things to note:

- This works with the **default** `post` codec — custom error bodies come
  from the error type's `HttpError` conversion, not from `post_with`.
- Decode failures (malformed request JSON) are converted upstream of the
  handler and stay plain-text 400s; the structured body covers failures
  raised by the handler itself.

## Spy mock tests

The crate-level tests (`tests/`) drive every handler through a mock
provider that *records* outbound HTTP requests as well as answering them.
Tests assert on the request shape — method, path, and the `Client-Cert`
header — and on the exact number of calls, which is how the cache tests
prove a hit never reaches HTTP. Recording in the capability mock is the
cheapest way to verify *what left the guest* without a real network.
