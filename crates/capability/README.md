# capability

Compiling proof for the Omnia host interfaces the transit crates do not
otherwise exercise: `blobstore`, `websocket`, `docstore`, and `sql`.

Each module carries one small, deliberately domain-free handler over its
interface:

| Module | Interface | Handler |
| --- | --- | --- |
| `blob` | `blobstore` | `archive(ArchiveRequest)` — store a payload and report its size |
| `broadcast` | `websocket` | `alert(AlertRequest)` — push an alert to every WebSocket client, or to the named sockets |
| `document` | `docstore` | `note(NoteRequest)` — upsert a JSON note and read it back |
| `table` | `sql` | `reading(ReadingRequest)` — insert a reading into the named database and count the sensor's rows |

Each part declares its `pub(crate)` path constant (`/examples/*`), handler,
and `handlers()`; `src/lib.rs` merges the parts with `Handlers::from`, and
the workspace-root [`router`](../../src/lib.rs) merges
`capability::handlers()`. [`tests/operations.rs`](tests/operations.rs)
drives every handler through the stem's component (`guest.rs`, compiled by
`build.rs`) over `omnia_test::host` backends.

A `websocket` send is outbound only, so serving it requires no WebSocket
export.
