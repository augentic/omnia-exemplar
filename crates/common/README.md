# Common

What every stem in an Omnia guest shares (`crates/common`):

- `Capabilities` — the trait bound over all ten `omnia-sdk` capabilities (`BlobStore`, `Broadcast`, `Config`, `DocumentStore`, `HttpRequest`, `Identity`, `Publish`, `StateStore`, `TableStore`, `Vault`) that every stem's `routes()` and `topics()` take.
- `config` — `ENV` plus `env()`, `topic()`, and `topic_for()` for qualifying topic suffixes by deployment environment: a publisher qualifies the topic it sends to, and the root's `topic_router` qualifies every consumed topic once through `Routes::map`. Every domain configuration key belongs in the stem part that reads it.
- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.
- `RowExt` — typed column reads over `omnia_sdk::sql::Row` (`get_i64`, `get_opt_i64`, `get_f64`, `get_opt_f64`, `get_bool`, `get_opt_bool`, `get_str`, `get_opt_str`).
