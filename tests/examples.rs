//! Examples gate: the example hosts must keep compiling.
//!
//! `examples/runtime.rs` is a server host that never exits, so it is
//! **build-only** here: `cargo build --locked --examples` from the package
//! root, and exit status 0 is the whole assertion. Behaviour is not tested
//! here: the routes and messaging rungs drive the guest natively, and each
//! crate's own suite covers its logic. Anything the host links against (the
//! omnia runtime, the `Hooks` wiring, the manifest) drifting far enough to
//! break the example fails this test before it fails a manual
//! `cargo run --example runtime`.
//!
//! Nextest runs the test in its own process; concurrent invocations
//! serialise on cargo's build lock, so the nested build is safe to repeat.

use std::process::Command;

/// Run `cargo --locked <args>` from the package root and require success.
fn cargo(args: &[&str]) {
    let status = Command::new(env!("CARGO"))
        .arg("--locked")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "cargo --locked {} failed", args.join(" "));
}

/// Every `[[example]]` in the root manifest compiles.
#[test]
fn build() {
    cargo(&["build", "--examples"]);
}
