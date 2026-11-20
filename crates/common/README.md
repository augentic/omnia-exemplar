# Common

What every stem in an Omnia guest shares (`crates/common`):

- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.

A stem reaches the host through `omnia-sdk`'s bindings, so this crate links nothing from omnia.
