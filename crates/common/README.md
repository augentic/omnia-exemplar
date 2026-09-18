# Acme Common

Shared logic for the (fictional) Acme transit domain:

- `config` — the configuration key catalog plus `env()` / `topic()` helpers
  with a single documented resolution policy.
- `routes` — the canonical HTTP path and messaging topic tables consumed by
  both guests and the domain crates.
- `block_mgt` / `fleet` — clients for the Block Management and Fleet APIs,
  retrieving vehicle allocations and vehicle metadata respectively.

The API clients are written against the `omnia-sdk` capability traits
(`Config`, `HttpRequest`, `Identity`, `StateStore`) so the same code runs
inside the WASM guest and against native `omnia_test::guest::Provider`
doubles in tests.

## Caching

Omnia's guest-side HTTP client does not cache. The two hot lookups —
`fleet::vehicle` (five minutes) and `block_mgt::cached_allocation` (twenty
seconds) — opt in explicitly by wrapping the provider in
`omnia_http_cache::HttpCache` (from
[omnia-extensions](https://github.com/augentic/omnia-extensions)), which
stores responses through the provider's own `StateStore`:

```rust,ignore
let response = HttpCache::new(provider, provider).fetch(request).await?;
```

The request carries `Cache-Control: max-age=<secs>` for the TTL and
`If-None-Match` for the cache key. `HttpCache` requires a **quoted strong
etag** and refuses bare tokens before any request leaves, and the etag is
used verbatim as the `StateStore` key, so both clients namespace it to keep
the entry out of the guest's own state:

| Client | Etag |
| --- | --- |
| `fleet::vehicle` | `"fleet:<query>"` |
| `block_mgt::cached_allocation` | `"allocation:<vehicle_id>"` |

The other `block_mgt` functions are uncached and call `HttpRequest::fetch`
directly. Natively, `omnia_test::guest::Provider` supplies a `Memory`
`StateStore`, and `MatchedHttp` matches on method, URL and body — not
headers — so the wrapper is transparent to the handler rung.
