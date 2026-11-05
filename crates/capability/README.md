# capability

Compiling proof for the Omnia guest capabilities the transit crates do not
otherwise exercise: `BlobStore`, `Broadcast`, `DocumentStore`, and
`TableStore`.

Each module carries one small, deliberately domain-free `Handler` over its
capability trait:

| Module | Capability | Handler |
| --- | --- | --- |
| `blob` | `BlobStore` | `archive(ArchiveRequest)` — store a payload and report its size |
| `broadcast` | `Broadcast` | `alert(AlertRequest)` — push an alert to WebSocket clients |
| `document` | `DocumentStore` | `note(NoteRequest)` — upsert a JSON note and read it back |
| `table` | `TableStore` | `reading(ReadingRequest)` — insert a reading and count the sensor's rows |

Each part declares its `pub(crate)` path constant (`/examples/*`), handler,
and `routes()`; `src/lib.rs` folds them with `common::routers`, and the
workspace-root `src/routes.rs` merges `capability::router()`.
[`tests/operations.rs`](tests/operations.rs) drives `capability::router()`
over `omnia_test::guest::Provider` doubles.

`Broadcast::send` is client-side only — the guest connects out to the
broadcast channel — so serving it requires no WebSocket export.
