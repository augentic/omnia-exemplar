# Acme Common

Shared logic for the (fictional) Acme transit domain:

- `config` — the configuration key catalog plus `env()` / `topic()` helpers
  with a single documented resolution policy.
- `routes` — the canonical HTTP path and messaging topic tables consumed by
  both guests and the domain crates.
- `block_mgt` / `fleet` — clients for the Block Management and Fleet APIs,
  retrieving vehicle allocations and vehicle metadata respectively.

The API clients are written against the `omnia-guest` capability traits
(`Config`, `HttpRequest`, `Identity`) so the same code runs inside the WASM
guest and against native mock providers in tests.
