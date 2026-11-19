# Guest template contract

The base-repo tooling contract every Omnia guest workspace starts from.
The Emery omnia target adapter compiles it in as the `omnia-template`
crate this directory is: `ENTRIES` (`src/entries.rs`) is the manifest's
entries as Rust, each body embedded through `include_str!`, depended on
at a pinned commit of this repository and scaffolded into a fresh working
copy, fill-only, before the first slice builds. Nothing here is copied
into the adapter by hand — the crate is the single physical source, and
the gate below holds its table to the manifest entry for entry. What the
crate does with a tree — `scaffold`, `drift`, `restore`, and the root
`Cargo.toml` held around its list — is its own documentation
(`cargo doc -p omnia-template`).

[`manifest.yaml`](manifest.yaml) is the sole source-to-target map.
Every `source` is repository-relative, and each entry declares a proof
mode:

- **exact** — the source *is* the repository-root file at its `target`
  path (`source == target`), with no second copy anywhere.
  This repository's green CI — including the workflows that actually
  execute at the root — is the template's proof. The `template-check`
  gate (inside `mise run test`, or `cargo run -p template-check`)
  enforces the shape.
- **seed** — a project-start baseline the consuming project immediately
  evolves: the root package and the lists a build appends its lines to,
  cargo-vet state, the lint configuration, deploy parameters. Seed
  bodies live under [`core/`](core/) at their target paths, are
  syntax-checked, and are never diffed against the root, because the
  root's copies legitimately diverge over time.

No template file carries a placeholder for the operator to fill: the
deploy parameters of `.github/workflows/publish.yaml` are read from the
consuming repository's GitHub variables, `STORAGE_ACCOUNT` and
`RESOURCE_GROUP`, set at repository or organisation level since a job
calling a reusable workflow names no environment, and the component
deploys under the repository's name, or `APP_NAME` where that name does
not fit a container app's. The package is `guest`, as this repository's
own root is.

The seed also carries the guest's starting shape:

- the root `Cargo.toml`, both the workspace and the deployable package,
  with `crate-type = ["cdylib", "rlib"]`, every pin under
  `[workspace.dependencies]`, the `wasm32`-excluded dev-dependencies the
  rungs build with, the `host` feature, and a dev profile that optimizes
  the Cranelift crates a stem's suites JIT-compile its guest with;
- `src/lib.rs` with `router`, whose handler list composes one entry per
  stem after the anchor `HANDLER_LIST_ANCHOR` names, laid one entry per
  line under the comment that heads the array, so two stems listed in one
  merge land as two lines; `http::export!(router)` and
  `messaging::export!(router)` bind it as the guest's exports;
- `tests/routes.rs` and `tests/messaging.rs`, one native rung each that
  builds the composed router, so two stems serving one route or consuming
  one topic fail there;
- `crates/common`, the topic table a publisher and its consumer agree on;
- `examples/runtime.rs`, the local host behind `host`.

Its shared dependency pins and patches must match the root workspace's,
and `templates/check/tests/scaffold.rs` renders the whole manifest into
`target/template-scaffold/` and proves the result lints, tests, and
builds for `wasm32-wasip3` with no hand edits, bare and then with the
fixture stem under `templates/check/fixtures/stem/` written in as a slice
writes one: its crate whole and its line in each list.

A stem is one crate under `crates/<stem>/`, and the fixture stem
`orders` (`placing`, `lookup`, `catalogue`, `dispatch`) is its proof and
the shape a generated stem copies:

- Its `src/lib.rs` is the crate doc, one `pub mod <part>;` line per part,
  and the function the root list composes, written whatever parts the
  stem has: `pub fn handlers() -> Handlers`, `Handlers::from([..])` over
  one `<part>::handlers(),` entry per part. The array is a list opened by
  `HANDLER_LIST_ANCHOR`, empty for a stem with no part yet. Every line of
  the entry outside its list is the same whichever slice writes it, so
  two slices writing one stem at once merge line by line, and the root
  lists every stem once.
- A part is one module: its paths, its settings' keys, and its portable
  state keys as `pub(crate) const`, its handlers `pub(crate)`, and
  `pub(crate) fn handlers()` registering them, so a part the stem's list
  does not name is dead code. A handler reaches the host through
  `omnia-sdk`'s bindings, so it takes no state.
- `guest.rs` is the stem's handlers as one component, an `[[example]]`
  that `build.rs` compiles to `wasm32-wasip3` through
  `omnia_test::build::Components`. The tests under `tests/<part>.rs` boot
  that component over `omnia_test::host` backends, through the boot in
  `tests/support/`, so a route or a topic is asserted as the wire reaches
  it and a host effect as its backend records it.
- The root `Cargo.toml` names the crate in its `[dependencies]`
  (`<stem> = { path = "crates/<stem>" }`) after the anchor
  `CRATE_LIST_ANCHOR` names, and `src/lib.rs` lists its `handlers()`, one
  line after `HANDLER_LIST_ANCHOR`. A topic the stem publishes or
  consumes is one constant appended to `crates/common/src/topic.rs`.
  Nothing else of the seed is edited.

The seed's `clippy.toml` and the `[workspace.lints]` of its `Cargo.toml`
carry the host boundary: `disallowed-methods` and `disallowed-types`
name what a guest never reaches on its own — the environment, the file
system, the network, the clock, threads and tasks, the process, state
kept across calls — and each entry's `reason` names the `omnia-sdk`
interface to reach instead. The lints are `forbid`, so no crate lifts them with an
`allow` or an `expect`, and `allow_attributes` is forbidden beside them.
`clippy.toml` is a static file: a project adds a `doc-valid-idents`
entry by editing the seed here.

Project-specific root policy — concrete crate names, private
registries, supply-chain exemptions, release-branch details — stays
outside this contract. Root files with no manifest entry are simply not
scaffolded.

Authoring rules:

- An `exact` entry is a manifest line and the matching `exact!` row of
  the crate's table: editing the root file *is* editing the template.
- A `seed` entry is a manifest line, its body under `core/` at the
  target's path, and the matching `seed!` row. The gate refuses a
  manifest entry without its row, a row without its entry, and a body
  under `core/` without either.
- A merge to `main` is a release act: the omnia adapter takes it by moving
  the commit it pins the crate at.
- `template-check` pins an exact `schema-version` for this manifest. The
  adapter reads the crate rather than the manifest, so a bump here lands
  with the table change it describes and reaches a consumer with its pin.
