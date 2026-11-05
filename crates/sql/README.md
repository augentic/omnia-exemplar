# SQL examples

The rich `wasi-sql` showcase: a two-table agency/feed schema exercising
parameterized SQL over `omnia_sdk::TableStore` (`TableStore::query`,
`TableStore::exec`) and `common::RowExt` (`get_i64`, `get_str`,
`get_opt_str`) — `SELECT` with `ORDER BY` and optional `LIMIT`, `INSERT`
with server-assigned ids, dynamic `UPDATE ... SET` writing only provided
fields, `DELETE` answering 404 on zero rows affected, and a multi-column
`LEFT JOIN` with aliased columns.

Schema DDL goes through `TableStore::exec` instead of wasm-only bindings,
so the handlers run unchanged against native mock providers (see
`src/schema.rs`).

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
| `/examples/agencies` | GET, POST | `list_agencies(ListAgenciesRequest)`, `create_agency(CreateAgencyRequest)` |
| `/examples/agencies/{id}` | GET, PATCH | `get_agency(GetAgencyRequest)`, `update_agency(UpdateAgencyRequest)` |
| `/examples/agencies/{agency_id}/feeds` | GET, POST | `list_agency_feeds(ListAgencyFeedsRequest)`, `create_feed(CreateFeedRequest)` |
| `/examples/feeds` | GET | `list_all_feeds(ListAllFeedsRequest)` |
| `/examples/feeds/{id}` | DELETE | `delete_feed(DeleteFeedRequest)` |

## Features demonstrated

- **Typed row extraction (`common::RowExt`)** — `Agency::from_row`,
  `Feed::from_row`, and `FeedWithAgency::from_row` read columns through
  `row.get_i64`, `row.get_str`, and `row.get_opt_str`
- **JOINs** — `FeedWithAgency` selects `agency.name AS agency_name`,
  `agency.url AS agency_url`, and `agency.timezone AS agency_timezone`
  through a `LEFT JOIN`
- **Server-assigned ids** — a max-id probe (`ORDER BY ... DESC LIMIT $1`)
  then max + 1
- **Existence checks** — get/update answer 404 for a missing agency, and a
  feed for a missing agency is rejected
- **Partial updates** — dynamic `SET` assignments for each provided field,
  an empty patch rejected as 400, and a fetch-after-update reply
- **Delete semantics** — `DELETE` answering 404 when zero rows are affected
- **Parameterized SQL** — every statement binds values as `$1, $2, ...`
  placeholders over `omnia_sdk::sql::DataType`

## Testing

[`tests/operations.rs`](tests/operations.rs) drives each handler through
`Client::call` over `omnia_test::guest::ScriptedTables`, asserting the
recorded SQL statements and bound parameters for id assignment, partial
update, the referential check, the JOIN listing, and delete-with-404.

```shell
cargo nextest run -p sql
```
