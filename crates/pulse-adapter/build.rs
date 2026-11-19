//! Compiles `guest.rs`, the stem's handlers as one component, to
//! `wasm32-wasip3` for the suites under `tests/`, and generates `gen.rs`
//! with its path constant (`PULSE_ADAPTER_GUEST`) for them to `include!`.
//!
//! The nested build compiles this package for `wasm32`, running this script
//! again under that target, where `Components` is a no-op.

fn main() {
    omnia_test::build::Components::in_workspace("../..")
        .package("pulse-adapter")
        .examples(["pulse-adapter-guest"])
        // `Cargo.lock` is outside the dep-info the nested build emits.
        .track(["Cargo.lock"])
        .build()
        .write_gen("gen.rs");
}
