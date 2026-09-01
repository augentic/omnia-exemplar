# SQL examples

The rich `wasi-sql` showcase: a two-table agency/feed schema exercising the
full guest ORM surface — `SelectBuilder`, `InsertBuilder::from_entity`,
`UpdateBuilder` with conditional sets, `DeleteBuilder`, and `entity!` with
multi-column JOIN aliasing. This crate restores the full SQL example that
omnia's "Example tidy" trimmed, rewritten in this repository's typed-handler
style: `#[omnia_guest::handler]` functions over `P: TableStore`, mounted by
the root guest under `/examples/*`, and tested natively against a spy mock
that recognizes the ORM-generated SQL.

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

| Route | Methods | Handler input |
| --- | --- | --- |
| `/examples/agencies` | GET, POST | `ListAgenciesRequest`, `CreateAgencyRequest` |
| `/examples/agencies/{id}` | GET, PATCH | `GetAgencyRequest`, `UpdateAgencyRequest` |
| `/examples/agencies/{agency_id}/feeds` | GET, POST | `ListAgencyFeedsRequest`, `CreateFeedRequest` |
| `/examples/feeds` | GET | `ListAllFeedsRequest` |
| `/examples/feeds/{id}` | DELETE | `DeleteFeedRequest` |

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

# List all feeds with agency info (demonstrates the JOIN entity)
curl -s http://localhost:8080/examples/feeds

# Delete a feed (404 when zero rows are affected)
curl -s -X DELETE http://localhost:8080/examples/feeds/1
```

## Features demonstrated

- **ORM entity definition** — the `entity!` macro, including column
  aliasing for joined tables
- **JOINs** — `FeedWithAgency` selects `agency.name`, `agency.url`, and
  `agency.timezone` through `Join::left` + `Filter::col_eq`
- **Server-assigned ids** — a max-id probe (`order_by_desc` + `limit(1)`)
  then max + 1
- **Existence checks** — get/update answer 404 for a missing agency, and a
  feed for a missing agency is rejected
- **Partial updates** — `UpdateBuilder` with one conditional `.set()` per
  provided field, an empty patch rejected as 400, and a fetch-after-update
  reply
- **Delete semantics** — `DeleteBuilder` with 404 on zero rows affected
- **Parameterized SQL** — every statement binds values as `$1, $2, ...`
  placeholders
- **Query building** — `SelectBuilder`, `InsertBuilder`, `UpdateBuilder`,
  `DeleteBuilder`

## Testing

[`tests/provider.rs`](tests/provider.rs) is a spy `TableStore` mock in the
style of `pattern-examples`: it keeps in-memory agency/feed tables,
*recognizes* the ORM-generated SQL (parsing the quoted column lists and
checking the bound parameters), answers the JOIN select with aliased
columns, and records every statement. [`tests/operations.rs`](tests/operations.rs)
drives the full CRUD flow through `Client::call` — id assignment, partial
update, the referential check, the JOIN listing, delete-with-404 — and
asserts on the recorded statement shapes.

```shell
cargo nextest run -p sql-examples
```
