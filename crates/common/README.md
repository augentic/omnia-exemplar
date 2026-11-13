# Common

What every stem in an Omnia guest shares (`crates/common`):

- `topic` — the messaging topic table a publishing stem and its consumer both name, so the two agree on the surface by construction.

Each stem declares the capability bound its `handlers()` takes, a `Requires` trait over the `omnia_sdk` capability traits its handlers reach.
