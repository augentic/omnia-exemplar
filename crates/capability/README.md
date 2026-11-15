# capability

Compiling proof for the Omnia guest capabilities the transit crates do not
otherwise exercise: `BlobStore`, `WebSockets`, `DocumentDb`, and
`SqlDb`.

Each module carries one small, deliberately domain-free handler over its
capability trait:

| Module | Capability | Handler |
| --- | --- | --- |
| `blob` | `BlobStore` | `archive(ArchiveRequest)` — store a payload and report its size |
| `broadcast` | `WebSockets` | `alert(AlertRequest)` — push an alert to every WebSocket client, or to the named sockets |
| `document` | `DocumentDb` | `note(NoteRequest)` — upsert a JSON note and read it back |
| `table` | `SqlDb` | `reading(ReadingRequest)` — insert a reading into the named database and count the sensor's rows |

Each part declares its `pub(crate)` path constant (`/examples/*`), handler,
and `handlers()`; `src/lib.rs` declares the stem's `Requires` over the four
traits and merges the parts with `Handlers::from`, and the workspace-root
[`handlers`](../../src/lib.rs) merges `capability::handlers()`.
[`tests/operations.rs`](tests/operations.rs) drives `capability::handlers()`
over `omnia_test::guest::Provider` doubles.

A `WebSockets` send is outbound only, so serving it requires no WebSocket
export.
