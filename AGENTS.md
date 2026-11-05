# Omnia Exemplar — Agent Instructions

The reference guest for [omnia](https://github.com/augentic/omnia) services, and the source of the guest template under `templates/guest`. [README.md](README.md) is the guide: the [architecture](README.md#architecture), the [crate map](README.md#crate-map), [how to add a handler](README.md#adding-a-new-handler), [what to copy and what not to](README.md#copy-this-not-that), the [testing rungs](README.md#testing), the [guest template contract](README.md#guest-template-contract), and the [development loop](README.md#development). The rules below are the organisation's shared conventions; where the README is more specific, it governs.

## A generated guest

For which crate shows which pattern, read the README's [crate map](README.md#crate-map) and [Copy this, not that](README.md#copy-this-not-that).

These rules hold for a guest laid from `templates/guest`. Inside this repository, the template's static files are edited under `templates/guest/core/` and embedded with `include_str!` by design.

- **Handler signature & stem entry**: `pub(crate) async fn name<P>(input: Input, context: Context<P>) -> omnia_sdk::Result<Output>` with narrow capability bounds on `P`; `crates/<stem>/src/lib.rs` holds only `pub mod <part>;`, `common::routers([..])`, and `common::topics(router, env, [..])`.
- **Direct SQL & DocStore types**: Write parameterized SQL directly over `TableStore::query` / `TableStore::exec`. Import `omnia_sdk::sql::{DataType, Field, Row}` (plus `common::RowExt`) and `omnia_sdk::docstore::*` from `omnia-sdk`; never add `omnia-wasi-sql` or `omnia-wasi-docstore` to a stem's `Cargo.toml`.
- **Body routes, query strings & cache-aside**: Decode a `handle_with` route with `omnia_sdk::api::http::query_input` or `body_input` (using `common::numeric_id` when a body route's path parameter is an integer), never `axum::extract::Query`, and use the 6-line `StateStore::get` / `StateStore::set` cache-aside pattern.
- **Upstream HTTP & errors**: Build requests with `axum::body::Body` and check `!response.status().is_success()` -> `Err(bad_gateway!(..))` (`502`). Where the specification names a `400` reason code, construct `omnia_sdk::Error::BadRequest { code: "<reason>".into(), description }` directly (`bad_request!` hardcodes `code: "bad_request"`, and the wire body `{"error": "<code>", "message": ".."}` reads `code`); use a module error type for a status outside `400`/`404`/`500`/`502` (such as `409`).
- **Template hold (`omnia-template`) & host boundary (`clippy.toml`)**: Never edit `Role::Static` files (`crates/common/src/config.rs` and `tests/routes.rs` included — declare per-part settings locally). `std::env`, `std::fs`, `std::net`, `std::thread`, `std::time::{SystemTime, Instant}`, `tokio::spawn`, `tokio::time`, `OnceLock`/`LazyLock`, and `include_str!` are `forbid`den; use `chrono::Utc::now()` for wall-clock timestamps and `uuid::Uuid::new_v4()` for ids.

## Git

Never `git commit`, `git push`, open or close a pull request, or delete a branch — in this repository or in any sibling checkout — unless the maintainer lifts this for the session, explicitly and for named work. Leave every change uncommitted in the working tree; the maintainer reviews and commits. No plan or to-do list carries a commit, push, or PR step, and an instruction to complete every step does not override this.

## Code style

clippy (`make lint`) and nightly rustfmt (`make fmt`) are the style gate; beyond them and the rules below, match the surrounding code.

- Suppress a lint with `#[expect(lint, reason = "…")]` at the smallest scope, never `#[allow]`.
- `<module>.rs` plus `<module>/<child>.rs`; `mod.rs` only under `tests/support/`.
- Order a module top-down. Primary `pub` entry-point fns and top-level types come first. A type's inherent `impl`, trait `impl`s, and child types follow it immediately before the next top-level item. A private helper sits immediately below its first caller, and `#[cfg(test)] mod tests` sits last.
- A fn over a type is that type's method, not a free fn taking it as its first argument, where the type's module declares the fn or the fn is a plain lookup or predicate on the type. A constructor is an associated fn. A policy `const` sits beside the type whose method reads it. Values several fns thread through every call become one struct whose methods they are. A fn stays free when it is pure over primitives and iterators, or when it is one module's rule applied to another module's type.

Comments follow the conventions `std`, `serde`, and `tokio` converge on: docs state the observable contract for the crate's user, never the body's mechanics.

- `///` goes on the public API only — the `pub` types, fns, fields, variants, and re-exports a user of the crate can reach — never on a private or `pub(crate)` item, an `impl` block, or a trait-impl method. A clap field's `///` is its `--help` text. A doc opens with one summary sentence (about fifteen words, full stop), then a blank line, then short sentences and bullet lists. `# Examples` holds compiled doctests, for non-obvious usage only; `# Errors` names each class the caller matches on, linked; `# Panics` the rest. Every item mentioned is an intra-doc link. No mechanics, history, or migration notes. A `//!` says what a module is for, in the same shape.
- A private item takes a `//` only for what a senior developer would not see from its name and signature: a constraint, a why, an invariant. Most carry nothing. No restatements, match-arm labels, or body paraphrases.
- Inside a body, a `//` is a section header: lowercase, no full stop, above a blank-line-separated block, naming what the block achieves, so the headers read together outline the fn. A fn readable at a glance carries none, and a header never narrates the line beneath it. The one in-body explanation is `// HACK: …`, for a trick a senior would not see through.
- A test fn takes `//`, never `///`, and only for rationale its scenario name and assertions do not expose.
- No commented-out code.
- Every sentence earns its place and reads once: short plain sentences, one idea each; three or more things are a bullet list, not a colon-and-dash clause; no chained em-dashes, nested parentheticals, or semicolon runs; each fact has one home across `//!`, `///`, and `//`. A comment is as long as its why takes and no longer — concise is not dense, and readable is not verbose.

## Testing

Tests drive the public boundary: a behaviour is asserted through what a user of the product or crate can reach, over scripted doubles rather than a live filesystem, network, or model, never through private internals. A suite below the root survives only for an independent library contract; a unit test only for a branch no public boundary reaches. A test fn names the scenario (`gen_spec`, `no_sources`), never the outcome. Scripted doubles are strict: script exactly the exchanges a run consumes.

## Commands

All from the repository root through `make` ([`Makefile`](Makefile) → mise). The tasks are the shared `mise/rust.toml` of [`augentic/toolkit`](https://github.com/augentic/toolkit), pinned in [`mise.toml`](mise.toml) to the tag every `uses:` under `.github/workflows/` names; bump both together in one pull request.

```bash
make ci # exactly the CI jobs: fmt-check + lint + test + test-docs + docs + vet + deny  — run before handing over
make check # local advisories: audit + fmt (rewrites) + lint + outdated + deps
make test # cargo nextest run --locked --workspace --all-features, under -Dwarnings
make lint # lint-host (cargo clippy --workspace --all-targets --all-features, then cargo hack --each-feature), then lint-wasm (the same over every lib, bin and example for wasm32-wasip2 — never tests)
make fmt # cargo +nightly fmt --all
make vet-regen # regenerate cargo-vet imports/exemptions/unpublished, then vet
make cov # cargo llvm-cov nextest --workspace --all-features --summary-only
make sweep # drop target/ artifacts untouched for a week
```

If `make ci` cannot run, say exactly why and which checks ran instead.

