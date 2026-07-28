# capability-examples

Compiling proof for the Omnia guest capabilities the transit crates do not
otherwise exercise: `BlobStore`, `Broadcast`, `DocumentStore`, and
`TableStore`.

Each module carries one small, deliberately domain-free `Operation` over its
capability trait:

| Module | Capability | Operation |
| --- | --- | --- |
| `blob` | `BlobStore` | `ArchiveRequest` — store a payload and report its size |
| `broadcast` | `Broadcast` | `AlertRequest` — push an alert to WebSocket clients |
| `document` | `DocumentStore` | `NoteRequest` — upsert a JSON note and read it back |
| `table` | `TableStore` | `ReadingRequest` — insert a reading and count the sensor's rows |

The crate-level tests (`tests/`) drive every operation through an in-memory
mock provider, and `guests/typed` routes them under `/examples/*` so the
default WASM capability implementations are instantiated in a real guest.

`Broadcast::send` is client-side only — the guest connects out to the
broadcast channel — so serving it requires no WebSocket export.
