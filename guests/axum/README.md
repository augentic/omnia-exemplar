# Axum guest (style B)

Wires the shared transit operations to WASI HTTP and WASI Messaging with
hand-written Axum handlers served through `omnia_wasi_http::serve`, and a raw
`incoming-handler` messaging export that matches on topic. Each handler
decodes its own payload and invokes the shared operation through an `Invoker`.

Compare with [`guests/typed`](../typed/README.md) (style A), which serves the
same routes and topics through the typed `omnia_guest::api` routers.

## Run

```shell
cargo build -p guest-axum --target wasm32-wasip2 --release

cp examples/.env.example .env   # fill in values, then:
set -a; source .env; set +a
cargo run -p guest-axum --example runner -- run target/wasm32-wasip2/release/guest_axum.wasm
```
