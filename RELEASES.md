## 0.1.0

Unreleased

### Added

- Reference WASI 0.3 (`wasm32-wasip3`, Rust `1.100`) guest workspace (`guest`) built on [omnia](https://github.com/augentic/omnia) `0.38.1`, exporting HTTP and messaging from `src/lib.rs` with a local `examples/runtime.rs` host.
- Transit pipeline stems (`crates/tally-connector`, `crates/pulse-connector`, `crates/pulse-adapter`, `crates/gtfs-adapter`, and `crates/common`) demonstrating HTTP and SOAP/XML ingress, upstream enrichment, outbound HTTP caching (`HttpCache`), and portable state keys.
- Capability and composition stems (`crates/capability`, `crates/docstore`, `crates/sql`, and `crates/pattern`) covering `blobstore`, `config`, `docstore`, `http`, `identity`, `keyvalue`, `messaging`, `sql`, and `websocket`.
- Three-rung test suite over `omnia_test::host` scripted doubles (`MatchedHttp`, `ScriptedTables`, `Sink`, `FixedIdentity`), native router checks (`tests/routes.rs`, `tests/messaging.rs`), and the assembled component suite (`tests/component.rs`).
- Guest template contract (`templates/guest/manifest.yaml`), embedded `omnia-template` crate (`templates/guest`), and `template-check` gate and scaffold proof (`templates/check`), backed by host-boundary lints, `cargo-vet`, `cargo-deny`, and `augentic/toolkit` `v0.7.0` CI and release workflows.

<!-- Release notes generated using configuration in .github/release.yaml at 19e28b529ddbba0e6b3a4b93651f48f423501979 -->

## What's Changed
* Token tree by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/1
* Simplify template by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/2
* remove macro guest by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/3
* Extend example patterns by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/4
* update deps by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/5
* Bump Omnia pin to d239f5c by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/6
* Update deps by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/7
* Update docs by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/8
* Update deps by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/9
* Use operator macro by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/10
* Tidy error types by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/11
* Omnia error type update by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/12
* Update guest api by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/13
* workflow improvement by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/14
* Codec by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/15
* update omnia guest API with generic http handler by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/16
* Omnia updates by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/17
* Add SQL and Docstore comprehensive examples by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/18
* Higher test coverage by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/19
* Migrate to omnia-test crate by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/20
* Omnia update 2026 09 07 by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/21
* Implement Function-bound Handlers by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/22
* Shorten test names by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/23
* Automate PR summary by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/24
* Test tidy by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/25
* Sdk by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/27
* Cache ext by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/28
* ORM moved from omnia to extensions by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/29
* update deps to public crates by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/30
* cut over to mise and versioned workflows by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/31
* Adopt the shared conventions from augentic/toolkit v0.3.0 by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/44
* Bump the shared conventions to augentic/toolkit v0.4.0 by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/45
* Update conventions by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/46
* Minor lint issues by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/47
* Adopt cfg_select! and bump MSRV to 1.99 by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/48
* Bump toolkit version by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/49
* Bump toolkit version by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/50
* Template generation crate by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/54
* Avoid process ids in omnia-template atomic writes by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/55
* Replace multiple #[cfg(...)] with cfg_select! by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/75
* Update dependencies by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/76
* Remove Renovate config by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/77
* Ignore augentic/toolkit in Dependabot by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/78
* Simplify message routing by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/79
* Scaffolding by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/80
* Update wasmtime 50.0.0 by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/89
* Add Dapr support by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/95
* Simplify codegen by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/96
* Emery eval fixes by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/97
* Lift ban on include_xxx! macros by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/98
* Update deps by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/99
* Code hygine by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/100
* Simplify capabilities by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/101
* Migrate to wasip3 by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/102
* Update omnia-ext's omnia-http-cache by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/103
* Omnia WIT upgrade by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/106
* Reshape SDK around WIT changes by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/107
* Move http-cache to omnia-sdk by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/108
* Post-SDK refactor improvements by @andrewweston in https://github.com/augentic/omnia-exemplar/pull/109
* Omnia pin by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/111
* Chrono replaced with  jiff by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/112
* Release prep by @andrew-goldie in https://github.com/augentic/omnia-exemplar/pull/113

## New Contributors
* @andrewweston made their first contribution in https://github.com/augentic/omnia-exemplar/pull/1
* @andrew-goldie made their first contribution in https://github.com/augentic/omnia-exemplar/pull/11

**Full Changelog**: https://github.com/augentic/omnia-exemplar/commits/v0.1.0

---

Release notes for previous releases can be found on the respective release
branches of the repository.

<!-- ARCHIVE_START -->
