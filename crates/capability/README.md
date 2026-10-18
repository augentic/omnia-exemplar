# capability-examples

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

The crate-level tests (`tests/`) drive every handler through
`omnia_test::guest::Provider` doubles, and `tests/dispatch.rs` drives the
crate's `router()`. Route constants for mounting the handlers under
`/examples/*` live in `src/routes.rs`, and the workspace-root guest merges
the router (there is no `guests/` tree — see the repository README).

`Broadcast::send` is client-side only — the guest connects out to the
broadcast channel — so serving it requires no WebSocket export.
