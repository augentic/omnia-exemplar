# Axum guest (style B)

Wires the shared transit operations to WASI HTTP and WASI Messaging with
hand-written Axum handlers served through `omnia_wasi_http::serve`, and a raw
`incoming-handler` messaging export that matches on exact topic. Each handler
decodes its own payload and invokes the shared operation through an `Invoker`.
Routes and topics come from the canonical tables in `acme_common::routes`.

**Prefer style A** ([`guests/typed`](../typed/README.md)) by default. Reach
for this style when you need transport-level control the typed router does
not give you:

- custom Axum extractors or middleware
- response shaping beyond JSON projection
- messaging dispatch that doesn't fit the typed router

Note the messaging export matches the exact `{env}-` qualified topic names —
the same names the typed guest registers — and forwards the domain error's
full display so structured error codes survive the string-only WIT contract.

The god-mode override route is only compiled with the `god-mode` cargo
feature (`--features god-mode`), and additionally requires the
`GOD_MODE_ENABLED` configuration key at runtime.

## Run

```shell
cargo build -p guest-axum --target wasm32-wasip2 --release

cp examples/.env.example .env   # fill in values, then:
set -a; source .env; set +a
cargo run -p guest-axum --example axum-runner -- run target/wasm32-wasip2/release/guest_axum.wasm
```
