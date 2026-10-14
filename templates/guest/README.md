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
  cargo-vet state, clippy duplicate-crate lists, deploy parameters. Seed
  bodies live under [`core/`](core/) at their target paths, are rendered
  with [`values.yaml`](values.yaml) and syntax-checked, and are never
  diffed against the root, because the root's copies legitimately
  diverge over time.

Tokens are `<UPPER_SNAKE>` placeholders declared in the manifest's
`tokens` map. Only seed templates may carry them; consumers receive the
tokens verbatim and the omnia adapter's prompts leave them to the
operator (`.github/workflows/publish.yaml` deploy parameters). The
package is `guest`, as this repository's own root is, so no token names
it. A lone uppercase letter (`<P>`) is a Rust generic in a `.rs` seed,
not a token.

The seed also carries the guest's starting shape: the root `Cargo.toml`
as both the workspace and the deployable package, `crate-type =
["cdylib", "rlib"]`, every pin under `[workspace.dependencies]`, the
`wasm32`-excluded `omnia-test` dev-dependency (`features = ["guest"]`)
and the `host` feature; `src/lib.rs` with the `wasm32`-gated WASI HTTP
and WASI Messaging exports over provider-generic routers built from the
lists `src/routes.rs` and `src/messaging.rs`; `tests/routes.rs` and
`tests/messaging.rs`, one native rung each over a seeded
`omnia_test::guest::Provider`; `crates/common`, the route and topic
tables and the configuration catalog every stem shares; and
`examples/runtime.rs`, the local host behind `host`. Its shared
dependency pins must match the root workspace's, and
`templates/check/tests/scaffold.rs` renders the whole manifest into
`target/template-scaffold/` and proves the result lints, tests, and
builds for `wasm32-wasip2` with no hand edits.

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
  value, and may appear only in `seed` templates.
- A merge to `main` is a release act: the omnia adapter takes it by moving
  the commit it pins the crate at.
- `template-check` pins an exact `schema-version` for this manifest. The
  adapter reads the crate rather than the manifest, so a bump here lands
  with the table change it describes and reaches a consumer with its pin.
