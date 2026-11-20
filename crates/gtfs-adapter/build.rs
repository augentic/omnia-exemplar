//! Compiles `guest.rs`, the stem's handlers as one component, to
//! `wasm32-wasip3` for the suites under `tests/`, and generates `gen.rs`
//! with its path constant (`GTFS_ADAPTER_GUEST`) for them to `include!`.
//!
//! The guest is built with the `god-mode` feature when this package is, so
//! `tests/god_mode.rs` reaches the override route. The nested build compiles
//! this package for `wasm32`, running this script again under that target,
//! where `Components` is a no-op.

use omnia_test::build::Components;

fn main() {
    let guest =
        Components::in_workspace("../..").package("gtfs-adapter").examples(["gtfs-adapter-guest"]);
    #[cfg(feature = "god-mode")]
    let guest = guest.features(["god-mode"]);

    guest
        // `Cargo.lock` is outside the dep-info the nested build emits.
        .track(["Cargo.lock"])
        .build()
        .write_gen("gen.rs");
}
