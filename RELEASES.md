## 0.1.0

Unreleased

### Added

- Initial exemplar seeded from the train realtime services, ported to omnia 0.35.

### Changed

- Tests moved to `omnia-test`: every hand-written `tests/provider.rs` mock is
  now one `omnia_test::provider!` declaration seeded with the `omnia_test::guest`
  doubles, and `acme-test` (`crates/test`) is gone — its `Fetch` matcher is
  `MatchedHttp`, and the pulse-adapter fixture loader lives in
  `crates/pulse-adapter/tests/fixture/`.
- The root guest gains a route rung and a messaging rung: `router<P>` and
  `messaging_router<P>` are public and provider-generic, the root crate builds
  as `["cdylib", "rlib"]`, and `tests/routes.rs` / `tests/messaging.rs` drive
  the production routing tables natively.
- `templates/guest` scaffolds `Cargo.toml`, `src/lib.rs`, and `tests/routes.rs`
  (new `CRATE_NAME` token); the template gate holds the seed's dependency pins
  equal to the workspace's, and a scaffold test builds the rendered project for
  `wasm32-wasip2` and runs its route test.

### R4 findings

Recorded for the Phase 4 review, not decided here:

- **`rlib` vs `crates/router` (4.3).** `rlib` on the root cost one
  `crate-type` entry and a narrowed `cfg` over the export items; nothing else
  moved. Both rungs link the root crate directly and the built guest still
  exports both handlers. The `rlib` doubles the host-side compile of the root
  crate's dependency tree for tests, which the exemplar already paid for its
  handler crates. Nothing observed pushes toward `crates/router`.
- **`ScriptedTables` survived `sql-examples`, with a shape change.** The
  stateful in-memory mock let one test walk create → update → list → delete;
  `ScriptedTables` is a responder, so that became one scenario per handler,
  each scripting the rows its query should see and asserting the recorded
  statement and parameters. The tests are longer per handler and more honest
  about the contract (they now pin the SQL text and bound `DataType`s), and no
  test needed the double to become a database. The `sql-examples` README now
  says plainly that `ScriptedTables` is not a store, since the old tests read
  as if one existed.
- **`acme-test` deleted.** Its `Fetch` matcher was `MatchedHttp` with a
  prefix match; the fixtures gained explicit `request` query strings so the
  exact-URL match holds, and the `TestDef`/`Fixture` loader shrank to a
  190-line test module in pulse-adapter (`tests/fixture/`). Nothing in it
  was general enough for `omnia-test`.
- **The scaffold test is a real build.** It renders the whole manifest into
  `target/template-scaffold/`, carries the root `[patch.crates-io]` over
  with absolute paths, seeds the lockfile, and runs `cargo build --target
  wasm32-wasip2` and `cargo test` sharing the exemplar's target directory.
  ~20–30 s warm; the first cold run compiles the guest dependency tree for
  wasm32.

---

Release notes for previous releases can be found on the respective release
branches of the repository.

<!-- ARCHIVE_START -->
