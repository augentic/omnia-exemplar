# Guest template contract

The base-repo tooling contract every Omnia guest workspace starts from.
The Emery omnia target adapter reads this contract at consumer-build
time from the build agent's checkout of this repository, and its
deterministic scaffold prelude writes the listed targets, fill-only,
before generation begins. Nothing here is copied into the adapter — the
checkout is the single physical source.

[`manifest.yaml`](manifest.yaml) is the sole source-to-target map.
Every `source` is repository-relative, and each entry declares a proof
mode:

- **exact** — the source *is* the repository-root file at its `target`
  path (`source == target`), token-free, with no second copy anywhere.
  This repository's green CI — including the workflows that actually
  execute at the root — is the template's proof. `cargo make
  template-check` enforces the shape.
- **seed** — a project-start baseline the consuming project immediately
  evolves: cargo-vet state, clippy duplicate-crate lists, deploy
  parameters. Seed bodies live under [`core/`](core/), are rendered
  with [`values.yaml`](values.yaml) and syntax-checked, and are never
  diffed against the root, because the root's copies legitimately
  diverge over time.

Tokens are `<UPPER_SNAKE>` placeholders declared in the manifest's
`tokens` map. Only seed templates may carry them; consumers receive the
tokens verbatim and the omnia adapter's build prompts direct the guest
writer to fill them (`.github/workflows/publish.yaml` deploy
parameters, the package and crate name). A lone uppercase letter
(`<P>`) is a Rust generic in a `.rs` seed, not a token.

The seed also carries the guest's starting shape: `Cargo.toml` with the
`omnia-guest` dependency, the `wasm32`-excluded `omnia-test`
dev-dependency (`features = ["guest"]`) and `crate-type = ["cdylib",
"rlib"]`; `src/lib.rs` with one `wasm32`-gated WASI HTTP export over a
provider-generic router; and `tests/routes.rs`, one
`omnia_test::provider!` line and one native route test. Its shared
dependency pins must match the root workspace's, and
`templates/check/tests/scaffold.rs` renders the whole manifest into
`target/template-scaffold/` and proves the result builds for
`wasm32-wasip2` and passes that test with no hand edits.

Project-specific root policy — concrete crate names, private
registries, supply-chain exemptions, release-branch details — stays
outside this contract. Root files with no manifest entry are simply not
scaffolded.

Authoring rules:

- An `exact` entry is just a manifest line: editing the root file *is*
  editing the template. Adding one requires the root file to be
  token-free.
- New tokens require a manifest `tokens` entry, a `values.yaml` render
  value, and may appear only in `seed` templates.
- Merges to `main` are release acts: the omnia adapter and its consumer
  builds track `main` unpinned.
- The adapter pins an exact `schema-version` for this manifest. Bumping
  that version here requires a coordinated adapter release, or consumer
  builds fail closed at the scaffold prelude.
