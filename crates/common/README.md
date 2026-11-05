# Common

What every stem in an Omnia guest shares (`crates/common`):

- `Capabilities` — the trait bound over all ten `omnia-sdk` capabilities (`BlobStore`, `Broadcast`, `Config`, `DocumentStore`, `HttpRequest`, `Identity`, `Publish`, `StateStore`, `TableStore`, `Vault`) that every stem's `router()` and `topics()` take.
- `routers([..])` and `topics(router, env).parts([..])` — the two folds a stem's `src/lib.rs` uses to compose its parts' routes and topic consumers.
- `config` — `ENV` plus `env()`, `topic()`, and `topic_for()` for qualifying topic suffixes by deployment environment. Every domain configuration key belongs in the stem part that reads it.
- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.
- `RowExt` and `numeric_id` — typed column reads over `omnia_sdk::sql::Row` (`get_i64`, `get_f64`, `get_str`, `get_opt_str`) and serde deserialization for integer ids that arrive either as JSON numbers or path-parameter strings.
