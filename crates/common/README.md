# Common

What every stem in an Omnia guest shares (`crates/common`):

- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.

The capability bound a stem's `handlers()` takes is `omnia_sdk::Capabilities`, every capability trait at once.
