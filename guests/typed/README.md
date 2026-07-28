# Typed guest (style A)

Wires the shared transit operations to WASI HTTP and WASI Messaging using the
typed `omnia_guest::api` routers. Each route binds an `Operation` directly;
the router handles transport decoding, invocation, and response projection.
Non-JSON payloads use `decode_with` (messaging) or a single hand-written Axum
route via `router.into_axum()` (HTTP).

Compare with [`guests/axum`](../axum/README.md) (style B), which serves the
same routes and topics with hand-written Axum handlers.

## Run

```shell
cargo build -p guest-typed --target wasm32-wasip2 --release

cp examples/.env.example .env   # fill in values, then:
set -a; source .env; set +a
cargo run -p guest-typed --example runner -- run target/wasm32-wasip2/release/guest_typed.wasm
```
