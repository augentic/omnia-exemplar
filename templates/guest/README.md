# Guest template contract

The tree every Omnia guest workspace starts from, as the `omnia-template`
crate: `ENTRIES` is the template, one `Entry` per file with its body
embedded at compile time, and `lay`, `drift`, and `restore` are what a
consumer does with a tree over it. The Emery omnia target adapter is the
consumer: it pins this repository at a commit as a git dependency, lays the
template into a fresh working copy before the first slice builds, and holds
the tree to it after every build and verify.

Each entry declares a proof mode:

- **exact** — the body is the repository-root file at the entry's path,
  read in place through `include_str!`, token-free, with no second copy
  anywhere. This repository's green CI — including the workflows that
  actually execute at the root — is the template's proof.
- **seed** — a project-start baseline the consuming project evolves: the
  root package, the routers and their rungs, the shared `common` crate,
  the local host, cargo-vet state, clippy duplicate-crate lists, deploy
  parameters. Seed bodies live under [`core/`](core/) at their target
  paths, are syntax-checked, and are never diffed against the root,
  because the root's copies legitimately diverge over time.

And a role, which is how a tree built from it holds the file:

- **static** — laid once and never edited. `drift` names the lines a tree
  changed and `restore` puts the file back byte for byte.
- **listed** — holds a list a build appends its lines to, the text around
  the list fixed: the root `Cargo.toml` and `clippy.toml`, the two router
  lists, the two root rungs, and the tables and catalog under
  `crates/common`. The file is never held whole; the root `Cargo.toml` alone
  is held outside its two stem lists, through `Manifest`, since the text
  around them is the pins every crate of the tree resolves under.

Tokens are `<UPPER_SNAKE>` placeholders declared in `TOKENS`. Only seed
bodies may carry them; consumers receive them verbatim and the omnia
adapter's prompts leave them for the operator (`.github/workflows/publish.yaml`
deploy parameters). The package is `guest`, as this repository's own root is,
so no token names it. A lone uppercase letter (`<P>`) is a Rust generic in a
`.rs` seed, not a token.

The `template-check` gate (inside `mise run test`, or `cargo run -p
template-check`) holds the table to the tree: every exact body is the root
file in place, every seed body is the file under `core/` at its path, every
TOML, JSON, and YAML body parses as written, the seed's dependency pins are
the workspace's, nothing under `core/` is left out of the table, and every
toolkit reference names one tag. Its scaffold proof lays the template into
`target/template-scaffold/` and runs the checks the adapter's verify runs
over an integrated tree — `cargo clippy --workspace --all-targets -- -D
warnings`, `cargo test --workspace`, `cargo build --target wasm32-wasip2` —
with no hand edits.

Project-specific root policy — concrete crate names, private registries,
supply-chain exemptions, release-branch details — stays outside this
contract. Root files with no entry are simply not laid.

Authoring rules:

- An `exact` entry is one `exact!` line of the table: editing the root file
  *is* editing the template. Adding one requires the root file to be
  token-free.
- A `seed` entry is one `seed!` line and its body under `core/` at the same
  path. A seed body that shares a dependency with the root pins it at the
  root's version.
- New tokens are a `TOKENS` entry and may appear only in seed bodies.
- The crate is consumed from a checkout of this repository, pinned to a
  commit, and is never packaged: an exact body is read from two directories
  above the package. A merge to `main` is a release the consumer takes by
  moving its pin; a change to the table's shape is a change to the crate's
  version.
