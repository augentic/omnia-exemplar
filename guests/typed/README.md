# Typed guest (style A)

Wires the shared transit operations to WASI HTTP and WASI Messaging using the
typed `omnia_guest::api` routers. Each route binds an `Operation` directly;
the router handles transport decoding, invocation, and response projection.
Routes and topics come from the canonical tables in `acme_common::routes`.

**This is the default style** — the least code and the hardest to get wrong.
Compare with [`guests/axum`](../axum/README.md) (style B), the escape hatch
for when you need transport-level control.

Non-JSON payloads stay inside this style:

- **Messaging** — `consume::<Op>().decode_with(fn)` swaps the JSON decoder
  for a custom one (see `decode_pulse_xml` in `src/lib.rs`).
- **HTTP** — `router.into_axum()` exposes the underlying Axum router for a
  hand-written route (the SOAP/XML `POST /inbound/xml` ingress).

The god-mode override route is only compiled with the `god-mode` cargo
feature (`--features god-mode`), and additionally requires the
`GOD_MODE_ENABLED` configuration key at runtime.

## Run

```shell
cargo build -p guest-typed --target wasm32-wasip2 --release

cp examples/.env.example .env   # fill in values, then:
set -a; source .env; set +a
cargo run -p guest-typed --example typed-runner -- run target/wasm32-wasip2/release/guest_typed.wasm
```
