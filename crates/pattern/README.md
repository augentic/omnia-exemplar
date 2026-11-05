# pattern

Composition patterns over the Omnia guest capabilities, distilled from real
replatformed services. Where `capability` proves one capability at a time,
each handler here composes several:

| Module | Capabilities | Handler |
| --- | --- | --- |
| `decode` | `Config` + `HttpRequest` + `StateStore` | `decode_segment` — decode-through-cache with a config-carried client certificate |
| `cas` | `StateStore` (`StateStore::cas`) | `update_quota` — optimistic concurrency over `StateStore::cas`, answering `409 Conflict` via `QuotaError` |
| `identity` | `Config` + `Identity` + `HttpRequest` | `fetch_upstream_status` — resolving an upstream identity via `Config::get`, acquiring a bearer token via `Identity::access_token`, and forwarding `Authorization: Bearer <token>` |
| `place` | `TableStore` + `common::RowExt` | `upsert_place` — parameterized `INSERT … ON CONFLICT` upsert, rejecting bad coordinates with a structured JSON error body (`PlaceError`) |
| `place` | `TableStore` + `common::RowExt` | `nearby_places` — bounding-box `SELECT` refined by haversine |

The crate's `router()` serves these under `/examples/patterns/*`.

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

## Optimistic concurrency (`StateStore::cas`)

`UpdateQuotaRequest` reads the current record from `StateStore::get`, checks
`expected_version`, and writes the next revision with `StateStore::cas`.
When a concurrent writer modifies the entry (or the version does not match),
`QuotaError::Conflict` converts to `HttpError::new(StatusCode::CONFLICT, ..)`
(`409 Conflict`).

## Upstream bearer-token authentication (`Identity::access_token`)

`UpstreamStatusRequest` reads the identity name (`PATTERN_UPSTREAM_IDENTITY`)
and base URL (`PATTERN_UPSTREAM_URL`) from `Config::get`, calls
`Identity::access_token(provider, identity)` to obtain the bearer token, and
sends `Authorization: Bearer <token>` on the outbound `HttpRequest::fetch`
call, turning non-2xx upstream responses into `bad_gateway!` (`502`).

## Relational geo queries — no `GEORADIUS`

`StateStore` is a key-value cache, not a geospatial index. When a service
needs "what is within this radius?", model the rows relationally:

1. Over-approximate the radius with a degree bounding box — four plain,
   indexable comparisons in a parameterized `SELECT`.
2. Map rows with `common::RowExt` (`get_str`, `get_f64`) and refine the
   survivors with a haversine check in Rust.

`UpsertPlaceRequest` writes the rows with a parameterized
`INSERT INTO places ... ON CONFLICT (id) DO UPDATE SET ...` statement.

## Extending the framework's JSON error envelope

`upsert_place` returns `Result<UpsertPlaceReply, PlaceError>` and implements
`From<PlaceError> for HttpError` using `HttpError::with_body` to serialize a
superset of `omnia_sdk::api::ErrorBody` (`error`, `message`, plus the
variant's domain fields).
