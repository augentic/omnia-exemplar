# Common

What every stem in an Omnia guest shares (`crates/common`):

- `Capabilities` — the trait bound over all ten `omnia-sdk` capabilities (`BlobStore`, `Broadcast`, `Config`, `DocumentStore`, `HttpRequest`, `Identity`, `Publish`, `StateStore`, `TableStore`, `Vault`) that every stem's `handlers()` takes.
- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.
