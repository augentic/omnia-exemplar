# SQL examples

The rich `wasi-sql` showcase: a two-table agency/feed schema exercising
parameterized SQL over `omnia_sdk::TableStore` (`TableStore::query`,
`TableStore::exec`) and `common::RowExt` (`get_i64`, `get_str`,
`get_opt_str`) — `SELECT` with `ORDER BY` and optional `LIMIT`, `INSERT`
with server-assigned ids, dynamic `UPDATE ... SET` writing only provided
fields, `DELETE` answering 404 on zero rows affected, and a multi-column
`LEFT JOIN` with aliased columns.

One deliberate deviation from the pre-trim original: schema DDL goes through
`TableStore::exec` instead of the wasm-only `Connection`/`Statement`/
`readwrite` bindings, so the handlers run unchanged against native mock
providers (see `src/schema.rs`).

## Quick start

Run the guest under the example host, then exercise the endpoints:

```shell
cargo build --target wasm32-wasip2 --release
set -a; source .env; set +a
cargo run --example runtime -- run target/wasm32-wasip2/release/guest.wasm
```

## Endpoints

| Route | Methods | Handler |
| --- | --- | --- |
| `/examples/agencies` | GET, POST | `list_agencies(Query<ListAgenciesRequest>)`, `create_agency(Json<CreateAgencyRequest>)` |
| `/examples/agencies/{id}` | GET, PATCH | `get_agency(Path<i64>)`, `update_agency(Path<i64>, Json<UpdateAgencyRequest>)` |
| `/examples/agencies/{agency_id}/feeds` | GET, POST | `list_agency_feeds(Path<i64>)`, `create_feed(Path<i64>, Json<CreateFeedRequest>)` |
| `/examples/feeds` | GET | `list_all_feeds(Query<ListAllFeedsRequest>)` |
| `/examples/feeds/{id}` | DELETE | `delete_feed(Path<i64>)` |

### Agencies

```shell
# Create an agency — the id is server-assigned (max + 1)
curl -s -X POST http://localhost:8080/examples/agencies \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ritchies Transport","url":"https://ritchies.co.nz","timezone":"Pacific/Auckland"}'

# List all agencies, newest first
curl -s http://localhost:8080/examples/agencies

# Get one agency (404 when absent)
curl -s http://localhost:8080/examples/agencies/1

# Partial update — only provided fields are written; the reply is the
# row fetched after the update
curl -s -X PATCH http://localhost:8080/examples/agencies/1 \
  -H 'Content-Type: application/json' \
  -d '{"name":"Ritchies Transport Agency"}'
```

### Feeds

```shell
# Create a feed for an agency (404 when the agency does not exist)
curl -s -X POST http://localhost:8080/examples/agencies/1/feeds \
  -H 'Content-Type: application/json' \
  -d '{"description":"Bus routes and schedules"}'

# List feeds for one agency
curl -s http://localhost:8080/examples/agencies/1/feeds

# List all feeds with agency info (demonstrates the JOIN projection)
curl -s http://localhost:8080/examples/feeds

# Delete a feed (404 when zero rows are affected)
curl -s -X DELETE http://localhost:8080/examples/feeds/1
```

## Features demonstrated

- **Typed row extraction (`common::RowExt`)** — `TryFrom<&Row>` for
  `Agency`, `Feed`, and `FeedWithAgency` reads columns through `row.get_i64`,
  `row.get_str`, and `row.get_opt_str`
- **JOINs** — `FeedWithAgency` selects `agency.name AS agency_name`,
  `agency.url AS agency_url`, and `agency.timezone AS agency_timezone`
  through a `LEFT JOIN`
- **Server-assigned ids** — an id-only max probe (`SELECT <pk> FROM <table>
  ORDER BY <pk> DESC LIMIT 1`) then max + 1
- **Existence checks** — get/update answer 404 for a missing agency, and a
  feed for a missing agency is rejected
- **Partial updates** — dynamic `SET` assignments for each provided field,
  an empty patch rejected as 400, and a fetch-after-update reply
- **Delete semantics** — `DELETE` answering 404 when zero rows are affected
- **Parameterized SQL** — every statement binds values as `$1, $2, ...`
  placeholders over `omnia_sdk::sql::DataType`

## Testing

[`tests/operations.rs`](tests/operations.rs) drives `sql::routes()` over
`omnia_test::guest::ScriptedTables`, a responder-only `TableStore` double:
every scenario scripts the rows the `SELECT` should see and asserts the
recorded statements — SQL text and bound parameters — for id assignment,
partial update, the referential check, the JOIN listing, and delete-with-404.
There is no in-memory database; the tests pin the contract between the
handlers and the store.

```shell
cargo nextest run -p sql
```
