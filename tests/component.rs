//! Component rung: the shipped `guest.wasm` through the example host's own
//! wiring.
//!
//! `build.rs` compiles this package to a `wasm32-wasip2` component and
//! `gen.rs` names it (`COMPONENT_GUEST`). `examples/runtime.rs` is included
//! as `production`, so the `Hooks` its `runtime!` generates — the exact host
//! rows the binary links — assemble the runtime over `omnia_test::host::
//! Backends`, the in-memory defaults. A host the guest imports but the
//! example does not declare fails here at link time, which is the drift the
//! native rungs cannot see.

use omnia::Runtime;
use omnia_test::host::{Backends, Deployment};

include!(concat!(env!("OUT_DIR"), "/gen.rs"));

// The production `runtime!` as the example binary compiles it, untouched:
// `Hooks` is the wiring under test, `manifest()` the (empty) deployment it
// compiles in.
#[path = "../examples/runtime.rs"]
mod production;

/// A closed loopback port: every upstream call is refused immediately.
const CLOSED_UPSTREAM: &str = "http://127.0.0.1:9";

/// The `wasi:config` the guest reads: real keys, dummy values. Upstreams
/// point at a closed port so handlers that call out fail fast. Host-side
/// settings the smoke host needed from its environment (identity
/// credentials, the websocket bind address) have no counterpart here:
/// `Backends::defaults()` connects nothing.
const GUEST_CONFIG: &[(&str, &str)] = &[
    (acme_common::config::ENV, "dev"),
    (acme_common::config::BLOCK_MGT_URL, CLOSED_UPSTREAM),
    (acme_common::config::FLEET_URL, CLOSED_UPSTREAM),
    (acme_common::config::TRIP_MANAGEMENT_URL, CLOSED_UPSTREAM),
    (acme_common::config::STATIC_API_URL, CLOSED_UPSTREAM),
    (acme_common::config::API_IDENTITY, "component-test"),
    (pattern::decode::DECODER_URL, CLOSED_UPSTREAM),
    (pattern::decode::CLIENT_CERT, "component-test"),
];

/// Assemble the shipped component over fresh in-memory backends through the
/// example host's `Hooks`, returning the bundle for reading state back.
async fn boot() -> (Backends, Runtime<Backends>) {
    let backends = Backends::defaults().await.config(GUEST_CONFIG.iter().copied());
    let runtime = Deployment::from(production::manifest())
        .guest("guest", COMPONENT_GUEST)
        .boot(backends.clone(), <production::Hooks as omnia::Wiring<Backends>>::link)
        .await
        .expect("the shipped component links through the example host's wiring");
    (backends, runtime)
}

/// The component's imports are all satisfied by the hosts the example
/// declares, and the runtime pre-instantiates it.
#[tokio::test]
async fn boots() {
    // The generated `main` and `run` stay untouched; only `Hooks` is driven.
    let _ = (production::main, production::run);
    let (_backends, runtime) = boot().await;
    runtime.shutdown();
}
