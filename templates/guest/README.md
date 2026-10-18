# Guest template contract

The base-repo tooling contract every Omnia guest workspace starts from.
The Emery omnia target adapter compiles it in as the `omnia-template`
crate this directory is: `ENTRIES` (`src/entries.rs`) is the manifest's
entries as Rust, each body embedded through `include_str!`, depended on
at a pinned commit of this repository and laid into a fresh working copy,
fill-only, before the first slice builds. Nothing here is copied into the
adapter by hand — the crate is the single physical source, and the gate
below holds its table to the manifest entry for entry. What the crate
does with a tree — `lay`, `drift`, `restore`, and the root `Cargo.toml`
held around its lists — is its own documentation
(`cargo doc -p omnia-template`).

[`manifest.yaml`](manifest.yaml) is the sole source-to-target map.
Every `source` is repository-relative, and each entry declares a proof
mode:

- **exact** — the source *is* the repository-root file at its `target`
  path (`source == target`), token-free, with no second copy anywhere.
  This repository's green CI — including the workflows that actually
  execute at the root — is the template's proof. The `template-check`
  gate (inside `mise run test`, or `cargo run -p template-check`)
  enforces the shape.
- **seed** — a project-start baseline the consuming project immediately
  evolves: the root package and the lists a build appends its lines to,
  cargo-vet state, the lint configuration, deploy parameters. Seed
  bodies live under [`core/`](core/) at their target paths, are rendered
  with [`values.yaml`](values.yaml) and syntax-checked, and are never
  diffed against the root, because the root's copies legitimately
  diverge over time.

Tokens are `<UPPER_SNAKE>` placeholders declared in the manifest's
`tokens` map. Only seed templates may carry them; consumers receive the
tokens verbatim and the omnia adapter's prompts leave them to the
operator. No entry carries one: the deploy parameters of
`.github/workflows/publish.yaml` are read from the consuming
repository's GitHub variables, `STORAGE_ACCOUNT` and `RESOURCE_GROUP`,
set at repository or organisation level since a job calling a reusable
workflow names no environment, and the component deploys under the
repository's name, or `APP_NAME` where that name does not fit a
container app's. The package is `guest`, as this
repository's own root is, so no token names it. A lone uppercase letter
(`<P>`) is a Rust generic in a `.rs` seed, not a token.

The seed also carries the guest's starting shape: the root `Cargo.toml`
as both the workspace and the deployable package, `crate-type =
["cdylib", "rlib"]`, every pin under `[workspace.dependencies]`, the
`wasm32`-excluded `omnia-test` dev-dependency (`features = ["guest"]`)
and the `host` feature; `src/lib.rs` with the `wasm32`-gated WASI HTTP
and WASI Messaging exports over provider-generic routers composed by
the lists `src/routes.rs` and `src/messaging.rs`, one statement per
stem; `tests/routes.rs` and `tests/messaging.rs`, one native rung each
that builds the composed router over a seeded
`omnia_test::guest::Provider`, so two stems serving one route or
consuming one topic fail there; `crates/common`, the `Capabilities`
bound a stem's routers take, the topic table a publisher and its
consumer agree on, and the environment helpers; and
`examples/runtime.rs`, the local host behind `host`. Its shared
dependency pins must match the root workspace's, and
`templates/check/tests/scaffold.rs` renders the whole manifest into
`target/template-scaffold/` and proves the result lints, tests, and
builds for `wasm32-wasip2` with no hand edits, bare and then with the
fixture stem under `templates/check/fixtures/stem/` laid in as a slice
lays one: its crate whole and its line in each list.

A stem is one crate under `crates/<stem>/`, and the fixture stem
`orders` is its proof and the shape a generated stem copies. Its
`src/lib.rs` is the crate doc, one `pub mod <part>;` line per part, and
the two functions the root lists compose, laid whichever parts the stem
has: `pub fn router<P: Capabilities>() -> axum::Router<Client<P>>`,
`common::routers([..])` over one `<part>::routes(),` entry per part
with routes, and `pub fn topics<P: Capabilities>(router:
messaging::Router<P>, env: &str) -> messaging::Router<P>`,
`common::topics(router, env).parts([..])` over one `<part>::topics,`
entry per part with topics. Each array is a list under a comment that
anchors it, empty for a stem with no route or no topic yet, so every
line of the entry outside its three lists is the same whichever slice
writes it and two slices laying one stem at once merge line by line. A
part is one module: its paths, its settings' keys, and its state keys
as `pub(crate) const`, its handlers `pub(crate)`, and `pub(crate) fn
routes()` and `pub(crate) fn topics(router, env)` registering them, so
a part the stem's lists do not name is dead code. Its tests under
`tests/<part>.rs` drive the stem's `router()` with `oneshot` and its
`topics()` through a `messaging::Router` over a `Client` of the
doubles, so a route or a topic is asserted as the wire reaches it. The
root `Cargo.toml` names the crate in its path list and its
`[dependencies]`, `src/routes.rs` merges its `router()`,
`src/messaging.rs` registers its `topics()`, and a topic the stem
publishes or consumes is one constant appended to
`crates/common/src/topic.rs`; nothing else of the seed is edited.

The seed's `clippy.toml` and the `[workspace.lints]` of its `Cargo.toml`
carry the host boundary: `disallowed-methods`, `disallowed-types`, and
`disallowed-macros` name what a guest never reaches on its own — the
environment, the file system, the network, the clock, threads and
tasks, the process, state kept across calls, a file compiled in — and
each entry's `reason` names the capability to reach instead. The lints
are `forbid`, so no crate lifts them with an `allow` or an `expect`, and
`allow_attributes` is forbidden beside them. `clippy.toml` is a static
file: a project adds a `doc-valid-idents` entry by editing the seed here.

Project-specific root policy — concrete crate names, private
registries, supply-chain exemptions, release-branch details — stays
outside this contract. Root files with no manifest entry are simply not
scaffolded.

Authoring rules:

- An `exact` entry is a manifest line and the matching `exact!` row of
  the crate's table: editing the root file *is* editing the template.
  Adding one requires the root file to be token-free.
- A `seed` entry is a manifest line, its body under `core/` at the
  target's path, and the matching `seed!` row. The gate refuses a
  manifest entry without its row, a row without its entry, and a body
  under `core/` without either.
- New tokens require a manifest `tokens` entry, a `values.yaml` render
  value, and may appear only in `seed` templates. An operator's fill is
  an edit `drift` must not report, so a body carrying a token cannot be
  `Role::Static`.
- A merge to `main` is a release act: the omnia adapter takes it by moving
  the commit it pins the crate at.
- `template-check` pins an exact `schema-version` for this manifest. The
  adapter reads the crate rather than the manifest, so a bump here lands
  with the table change it describes and reaches a consumer with its pin.
