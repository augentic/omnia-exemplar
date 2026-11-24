//! Compiles `guest.rs`, the stem's handlers as one component, to
//! `wasm32-wasip3` for the suites under `tests/`, and generates `gen.rs`
//! with its path constant (`GTFS_ADAPTER_GUEST`) for them to `include!`.
//!
//! The fixture always enables `god-mode` because `target/wasm32-fixtures` is
//! shared across every outer feature set (`cargo hack --each-feature`
//! included) and `tests/god_mode.rs` needs the override route compiled in.
//! The nested build compiles this package for `wasm32`, running this script
//! again under that target, where `Components` is a no-op.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .package("gtfs-adapter")
        .features(["god-mode"])
        .examples(["gtfs-adapter-guest"])
        // `Cargo.lock` is outside the dep-info the nested build emits.
        .track(["Cargo.lock"])
        .build()
        .write_gen("gen.rs");
}
