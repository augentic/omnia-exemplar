# capability-examples

Compiling proof for the Omnia guest capabilities the transit crates do not
otherwise exercise: `BlobStore`, `Broadcast`, `DocumentStore`, and
`TableStore`.

Each module carries one small, deliberately domain-free `Handler` over its
capability trait:

| Module | Capability | Handler |
| --- | --- | --- |
| `blob` | `BlobStore` | `ArchiveRequest` — store a payload and report its size |
| `broadcast` | `Broadcast` | `AlertRequest` — push an alert to WebSocket clients |
| `document` | `DocumentStore` | `NoteRequest` — upsert a JSON note and read it back |
| `table` | `TableStore` | `ReadingRequest` — insert a reading and count the sensor's rows |

The crate-level tests (`tests/`) drive every handler through an in-memory
mock provider. Route constants for mounting the handlers under `/examples/*`
live in `src/routes.rs`; the workspace-root guest does not wire them by
default (there is no `guests/` tree — see the repository README).

`Broadcast::send` is client-side only — the guest connects out to the
broadcast channel — so serving it requires no WebSocket export.
