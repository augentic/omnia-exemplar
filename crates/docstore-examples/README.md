# Docstore examples

The rich `wasi:docstore` showcase: three GTFS-like collections — stops,
routes, and stop times — exercising full CRUD and every portable filter type
through combined query endpoints, plus sorting and limit/continuation
pagination. This crate restores the full docstore example that omnia's
"Example tidy" trimmed, rewritten in this repository's typed-handler style:
`#[omnia_guest::handler]` functions over `P: DocumentStore`, mounted by the
root guest under `/examples/*`, and tested natively against a
filter-evaluating mock provider.

## Quick start

Run the guest under the example host, then exercise the endpoints:

```shell
cargo build --target wasm32-wasip2 --release
set -a; source .env; set +a
cargo run --example runtime -- run target/wasm32-wasip2/release/guest.wasm
```

## Endpoints

| Route | Methods | Handler input |
| --- | --- | --- |
| `/examples/stops` | GET, POST | `ListStopsRequest`, `CreateStopRequest` |
| `/examples/stops/{id}` | GET, PUT, DELETE | `GetStopRequest`, `UpsertStopRequest`, `DeleteStopRequest` |
| `/examples/routes` | GET, POST | `ListRoutesRequest`, `CreateRouteRequest` |
| `/examples/routes/{id}` | GET | `GetRouteRequest` |
| `/examples/stop-times` | GET, POST | `ListStopTimesRequest`, `CreateStopTimeRequest` |
| `/examples/stop-times/{id}` | GET | `GetStopTimeRequest` |

Replies flatten the document alongside its id:
`{"id": "stop-001", "stop_name": "...", ...}`.

### Stops

Create, read, upsert, and delete:

```shell
# Britomart — station, accessible, zone-1
curl -s -X POST http://localhost:8080/examples/stops \
  -H 'Content-Type: application/json' \
  -d '{"id":"stop-001","stop_name":"Britomart Transport Centre","stop_lat":-36.8442,"stop_lon":174.7676,"zone_id":"zone-1","wheelchair_boarding":1,"location_type":1,"parent_station":null,"last_updated":"2026-03-19T10:00:00Z"}'

# Devonport — no fare zone (exercises the null filters)
curl -s -X POST http://localhost:8080/examples/stops \
  -H 'Content-Type: application/json' \
  -d '{"id":"stop-005","stop_name":"Devonport Ferry Terminal","stop_lat":-36.8326,"stop_lon":174.7950,"zone_id":null,"wheelchair_boarding":1,"location_type":1,"parent_station":null,"last_updated":"2026-03-19T11:00:00Z"}'

curl -s http://localhost:8080/examples/stops/stop-001

curl -s -X PUT http://localhost:8080/examples/stops/stop-001 \
  -H 'Content-Type: application/json' \
  -d '{"stop_name":"Britomart Transport Centre","stop_lat":-36.8442,"stop_lon":174.7676,"zone_id":"zone-1","wheelchair_boarding":1,"location_type":1,"parent_station":null,"last_updated":"2026-03-19T12:00:00Z"}'

curl -s -X DELETE http://localhost:8080/examples/stops/stop-005
```

Query with any combination of the supported filters:

```shell
# All stops (sorted by name)
curl -s "http://localhost:8080/examples/stops"

# Text search — contains on stop_name
curl -s "http://localhost:8080/examples/stops?q=Station"

# By zone — eq on zone_id
curl -s "http://localhost:8080/examples/stops?zone=zone-1"

# Exclude a zone — ne on zone_id (direct ComparisonOp::Ne codepath)
curl -s "http://localhost:8080/examples/stops?exclude_zone=zone-1"

# Accessible stops — eq(wheelchair_boarding, 1) + is_not_null(zone_id)
curl -s "http://localhost:8080/examples/stops?accessible=true"

# Top-level stops only — is_null(parent_station)
curl -s "http://localhost:8080/examples/stops?top_level=true"

# Bounding box (Auckland CBD) — and(gte, lte, gte, lte)
curl -s "http://localhost:8080/examples/stops?min_lat=-36.86&max_lat=-36.83&min_lon=174.74&max_lon=174.80"

# Updated on date — on_date(last_updated)
curl -s "http://localhost:8080/examples/stops?updated_on=2026-03-19"

# Combined: accessible + zone + limit
curl -s "http://localhost:8080/examples/stops?accessible=true&zone=zone-1&limit=5"

# Pagination: limit + continuation token from the previous page
curl -s "http://localhost:8080/examples/stops?limit=2"
curl -s "http://localhost:8080/examples/stops?limit=2&continuation=<token>"
```

### Routes

```shell
curl -s -X POST http://localhost:8080/examples/routes \
  -H 'Content-Type: application/json' \
  -d '{"id":"route-nex","agency_id":"AT","route_short_name":"NEX","route_long_name":"Northern Express","route_type":3,"route_color":"00AEEF"}'

curl -s http://localhost:8080/examples/routes/route-nex

# Name search — or(contains(short_name), contains(long_name))
curl -s "http://localhost:8080/examples/routes?q=Northern"

# By route types — in_list(route_type, [2, 3])
curl -s "http://localhost:8080/examples/routes?types=2,3"

# By agency — eq(agency_id)
curl -s "http://localhost:8080/examples/routes?agency=AT"

# Exclude ferries — negate(eq(route_type, 4))
curl -s "http://localhost:8080/examples/routes?exclude_type=4"

# Exclude AT buses — negate(and(eq(agency), eq(type))) (De Morgan negation)
curl -s "http://localhost:8080/examples/routes?not_agency=AT&not_type=3"
```

### Stop times

```shell
curl -s -X POST http://localhost:8080/examples/stop-times \
  -H 'Content-Type: application/json' \
  -d '{"id":"nex-0800-1","trip_id":"trip-nex-0800","stop_id":"stop-004","arrival_time":"08:00:00","departure_time":"08:01:00","stop_sequence":1,"pickup_type":0,"drop_off_type":0}'

curl -s http://localhost:8080/examples/stop-times/nex-0800-1

# All stop times for a trip — eq(trip_id), sorted by sequence
curl -s "http://localhost:8080/examples/stop-times?trip=trip-nex-0800"

# Stop times at a stop — eq(stop_id)
curl -s "http://localhost:8080/examples/stop-times?stop=stop-001"

# Time range — gte + lte on arrival_time
curl -s "http://localhost:8080/examples/stop-times?after=08:00:00&before=08:30:00"

# Trip + sequence range — eq + gte + lte
curl -s "http://localhost:8080/examples/stop-times?trip=trip-nex-0800&min_seq=1&max_seq=2"
```

## Features demonstrated

- **CRUD** — insert, get, put (upsert), delete on stops; insert + get on
  routes and stop times
- **Combined query endpoints** — each collection has one query endpoint that
  builds `Filter::and(...)` from whichever query params are present
- **`Filter::eq`** — zone, agency, trip, stop
- **`Filter::ne`** — exclude a specific zone (direct `ComparisonOp::Ne`
  codepath)
- **`Filter::gte` / `lte`** — bounding box (lat/lon), time range, sequence
  range
- **`Filter::contains`** — text search on stop and route names
- **`Filter::in_list`** — route types (bus, rail, ferry)
- **`Filter::is_not_null`** — accessible stops require a fare zone
- **`Filter::is_null`** — top-level stops (no parent station)
- **`Filter::or`** — route name search across short and long names
- **`Filter::negate`** — exclude a route type
- **`Filter::negate(Filter::and(...))`** — exclude an agency + type combo
  (De Morgan negation)
- **`Filter::on_date`** — stops updated on a calendar date (rejects invalid
  dates as `400`)
- **Pagination** — limit + continuation token
- **Sort** — results sorted by name or sequence

## Testing

The pre-trim example verified its behaviour with a bash script of `curl` +
`jq` assertions; that script is now the native test suite. The tests in
[`tests/operations.rs`](tests/operations.rs) seed the same five stops, four
routes, and five stop times through `Client::call` and assert the exact
result count of every filter combination, page 2 via the continuation token,
and the CRUD round trips. [`tests/provider.rs`](tests/provider.rs) is the
mock `DocumentStore` that makes this possible natively: it *evaluates* the
filter tree over the stored JSON documents and honours `order_by`, `limit`,
and continuation, rather than canning responses.

```shell
cargo nextest run -p docstore-examples
```
