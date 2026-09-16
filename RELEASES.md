## 0.1.0

Unreleased

### Added

- Initial exemplar seeded from the train realtime services, ported to omnia 0.35.

### Changed

- Tests moved to `omnia-test`: every hand-written `tests/provider.rs` mock is
  now an `omnia_test::guest::Provider` seeded with the `omnia_test::guest`
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
- Handlers are plain fns bound at the route. `#[omnia_guest::handler]` is
  gone: every handler is now a `pub async fn name<P>(input, Context<P>)`
  named for its operation (`tally`, `pulse`, `motion`, `create_stop`, …),
  documented and re-exported from its crate root, and reads capabilities
  through `context.provider()`. The root routers bind them directly —
  `post(tally)`, `consume(motion)`, `handle_with(filter, handler, decode,
  encode)` — and tests call `client.call(handler, input, &metadata)`. Wire
  shapes, paths, and topics are unchanged.
- Pinned omnia to `83f0273` (omnia #280, which lets `omnia_test::build`
  compile several sources — shipped packages and test programs — into one
  `gen.rs`). The `omnia` facade now re-exports everything a host needs, so
  `[patch.crates-io]` names only the crates this workspace depends on
  directly. Omnia's macro trim removed `omnia_guest::provider!` and
  `omnia_test::provider!`: the production `Provider` is a unit struct with
  one empty `impl` per capability trait, and tests use
  `omnia_test::guest::Provider` directly.
- Pinned omnia to `fd17357` (omnia #282–#284). The guest SDK crate is now
  `omnia-sdk`: every `omnia-guest` dependency line and `omnia_guest::…` path
  in the workspace, the tests, and the `templates/guest` seed reads
  `omnia-sdk` / `omnia_sdk::…`; features and `omnia-test`'s `guest` rung are
  unchanged. Omnia dropped wRPC in favour of in-memory link dispatch, so the
  `wrpc-transport` / `wrpc-wasmtime` `[patch.crates-io]` git override, the
  `deny.toml` `allow-git` entry for `bytecodealliance/wrpc` (root and seed),
  and the `cargo vet` policies for `wrpc-introspect` / `wrpc-transport` are
  gone with it.
- The smoke tier is gone (`tests/smoke.rs`, the `smoke` task, and CI's
  `cargo make smoke` step); its coverage moved under `cargo make test`. The
  root route rung absorbed the dispatch checks (`routes::dispatch` over every
  registered `(method, path)`, plus the pulse codec, nearby body, and
  feature-gated `set_trip` cases). A component rung runs the shipped
  component through the example host's `Hooks`: the root `build.rs`
  (`omnia_test::build::Components`) compiles the guest fixture for
  `wasm32-wasip2` into `OUT_DIR`, and `tests/component.rs` boots it through
  `omnia_test::host::Deployment` over in-memory `Backends`, driving the
  `wasi:http` and `wasi:messaging` exports in-process (`HttpHandler`,
  `MessagingHandler`). `tests/examples.rs` gates `cargo build --examples`,
  since the server host never exits. Nothing is `#[ignore]`d. CI's wasm job
  is lint-only (`cargo make lint-wasm`) and the shared job gains
  `targets: wasm32-wasip2` for the nested fixture build.

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
