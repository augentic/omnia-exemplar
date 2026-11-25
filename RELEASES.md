## 0.1.0

Unreleased

### Added

- Reference WASI 0.3 (`wasm32-wasip3`, Rust `1.100`) guest workspace (`guest`) built on [omnia](https://github.com/augentic/omnia) `0.38.1`, exporting HTTP and messaging from `src/lib.rs` with a local `examples/runtime.rs` host.
- Transit pipeline stems (`crates/tally-connector`, `crates/pulse-connector`, `crates/pulse-adapter`, `crates/gtfs-adapter`, and `crates/common`) demonstrating HTTP and SOAP/XML ingress, upstream enrichment, outbound HTTP caching (`HttpCache`), and portable state keys.
- Capability and composition stems (`crates/capability`, `crates/docstore`, `crates/sql`, and `crates/pattern`) covering `blobstore`, `config`, `docstore`, `http`, `identity`, `keyvalue`, `messaging`, `sql`, and `websocket`.
- Three-rung test suite over `omnia_test::host` scripted doubles (`MatchedHttp`, `ScriptedTables`, `Sink`, `FixedIdentity`), native router checks (`tests/routes.rs`, `tests/messaging.rs`), and the assembled component suite (`tests/component.rs`).
- Guest template contract (`templates/guest/manifest.yaml`), embedded `omnia-template` crate (`templates/guest`), and `template-check` gate and scaffold proof (`templates/check`), backed by host-boundary lints, `cargo-vet`, `cargo-deny`, and `augentic/toolkit` `v0.7.0` CI and release workflows.

**Full Changelog**: https://github.com/augentic/omnia-exemplar/commits/v0.1.0

---

Release notes for previous releases can be found on the respective release
branches of the repository.

<!-- ARCHIVE_START -->
