//! Compiles this package to a `wasm32-wasip2` component for the component
//! rung (`tests/component.rs`) and generates `gen.rs` with its path constant
//! (`COMPONENT_GUEST`) for the native side to `include!`.
//!
//! The nested build compiles this same package for `wasm32`, running this
//! script again; `Components` is a no-op under that target, so the recursion
//! stops there. The fixture uses the dev profile and default features
//! (`god-mode` off), and lands under `OUT_DIR/fixtures`, so plain
//! `cargo test` is self-contained.

fn main() {
    omnia_test::build::Components::in_workspace(".")
        .packages(["guest"])
        .group("component")
        // `Cargo.lock` is outside the dep-info the nested build emits.
        .track(["Cargo.lock"])
        .build()
        .write_gen("gen.rs");
}
