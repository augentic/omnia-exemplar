# Guest template contract

The tokenized base-repo tooling files every Omnia guest workspace starts
from. The Emery omnia target adapter fetches this subtree at adapter
build time (`targets/omnia/build.rs` in `augentic/emery-adapters`),
bakes it into the component, and its deterministic scaffold prelude
writes the listed targets, fill-only, at the start of every consumer
build. There is no committed second copy in the adapter repository.

[`manifest.yaml`](manifest.yaml) is the sole source-to-target map. It
declares the closed token set and a per-file proof mode:

- **exact** — the template carries no tokens and must byte-match the
  file at its `target` path in this repository's root. Authorship flows
  templates → root, never root → templates: the root files are the
  rendered output of this subtree, so this repository's green CI —
  including the workflows that actually execute at the root — is the
  templates' proof. `cargo make template-check` enforces the equality.
- **seed** — a project-start baseline the consuming project immediately
  evolves: cargo-vet state, clippy duplicate-crate lists, deploy
  parameters. Seeds are rendered with [`values.yaml`](values.yaml) and
  syntax-checked; they are never diffed against the root, because the
  root's copies legitimately diverge over time.

Tokens are `<UPPER_SNAKE>` placeholders declared in the manifest's
`tokens` map. Only seed templates may carry them; consumers receive the
tokens verbatim and the omnia adapter's build prompts direct the guest
writer to fill them (`.github/workflows/publish.yaml` deploy
parameters).

Project-specific root policy — concrete crate names, private
registries, supply-chain exemptions, release-branch details — stays
outside this subtree and outside the render-diff set. Root files with
no template counterpart are exempt from the proof.

Authoring rules:

- Change a tooling convention by editing the template *and* the root
  file in the same commit; `template-check` fails on any divergence of
  an `exact` pair.
- New tokens require a manifest `tokens` entry, a `values.yaml` render
  value, and may appear only in `seed` templates.
- Merges to `main` are release acts: the omnia adapter and its consumer
  builds track `main` unpinned.
