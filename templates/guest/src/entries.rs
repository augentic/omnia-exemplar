//! The table of the template's files, each embedded with how it is proven and how a tree holds it.

/// How a file of the template is proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proof {
    /// The repository-root file at the entry's path, read in place and
    /// carrying no token, so this repository's green CI is its proof.
    Exact,
    /// A project-start baseline under `templates/guest/core/` at the
    /// entry's path, which the consuming project evolves and the gate
    /// syntax-checks.
    Seed,
}

/// How a tree built from the template holds a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Laid once and never edited: [`drift`](crate::drift) and
    /// [`restore`](crate::restore) hold the file to its body byte for byte.
    Static,
    /// Holds a list a slice appends its lines to, the text around the list
    /// fixed. The file is never held whole; the root `Cargo.toml` alone is
    /// held outside its lists, by [`Manifest`](crate::Manifest).
    Listed,
}

/// One file of the template: its root-relative path, its body, its proof, and its role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// The `/`-separated path of the file, relative to the tree's root.
    pub path: &'static str,
    /// The whole content the file holds when the template lays it.
    pub body: &'static str,
    /// How the file is proven.
    pub proof: Proof,
    /// How a tree holds the file.
    pub role: Role,
}

/// A placeholder a [`Seed`](Proof::Seed) body carries for the operator to fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// The name between the angle brackets, `<UPPER_SNAKE>`.
    pub name: &'static str,
    /// What the operator fills it with.
    pub description: &'static str,
}

/// The closed set of placeholders a body may carry, each spelled `<NAME>`.
///
/// Both stand in `.github/workflows/publish.yaml`, the deploy parameters
/// no template can know.
pub static TOKENS: &[Token] = &[
    Token {
        name: "STORAGE_ACCOUNT",
        description: "Azure storage account receiving the built component.",
    },
    Token {
        name: "RESOURCE_GROUP",
        description: "Azure resource group owning the storage account.",
    },
];

// The root manifest is the one body a rule of the crate reads on its own,
// so it is embedded by name and the table carries the same string.
pub const ROOT_MANIFEST: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/core/Cargo.toml"));

// The two macros spell an entry's proof by where its body is read from:
// an `exact` body is the repository-root file two directories up from
// this package, a `seed` body sits under `core/` at the entry's path.
macro_rules! exact {
    ($path:literal, $role:expr) => {
        Entry {
            path: $path,
            body: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../", $path)),
            proof: Proof::Exact,
            role: $role,
        }
    };
}

macro_rules! seed {
    ($path:literal, $role:expr) => {
        Entry {
            path: $path,
            body: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/core/", $path)),
            proof: Proof::Seed,
            role: $role,
        }
    };
}

/// Every file of the template, sorted by path.
///
/// The tree is the root-package guest this repository teaches:
///
/// - the root package `guest` is the component, exporting WASI HTTP and
///   WASI Messaging over one provider and building both routers from the
///   two lists `src/routes.rs` and `src/messaging.rs`, each driven natively
///   by a rung under `tests/`;
/// - `crates/common` holds the route and topic tables and the
///   configuration catalog every stem shares;
/// - `examples/runtime.rs` is the local host behind the `host` feature;
/// - the rest is the tooling this repository runs under, the toolchain,
///   the formatter, lint, deny, and vet configuration, the task runner,
///   and the workflows.
pub static ENTRIES: &[Entry] = &[
    exact!(".github/dependabot.yml", Role::Static),
    exact!(".github/workflows/audit.yaml", Role::Static),
    exact!(".github/workflows/ci.yaml", Role::Static),
    exact!(".github/workflows/patch.yaml", Role::Static),
    seed!(".github/workflows/publish.yaml", Role::Static),
    exact!(".github/workflows/release.yaml", Role::Static),
    exact!(".gitignore", Role::Static),
    exact!(".vscode/settings.json", Role::Static),
    Entry {
        path: "Cargo.toml",
        body: ROOT_MANIFEST,
        proof: Proof::Seed,
        role: Role::Listed,
    },
    exact!("Makefile", Role::Static),
    seed!("clippy.toml", Role::Listed),
    seed!("crates/common/Cargo.toml", Role::Static),
    seed!("crates/common/src/config.rs", Role::Listed),
    seed!("crates/common/src/lib.rs", Role::Static),
    seed!("crates/common/src/routes.rs", Role::Static),
    seed!("crates/common/src/routes/http.rs", Role::Listed),
    seed!("crates/common/src/routes/topic.rs", Role::Listed),
    seed!("deny.toml", Role::Static),
    seed!("examples/runtime.rs", Role::Static),
    exact!("mise.toml", Role::Static),
    exact!("rust-toolchain.toml", Role::Static),
    exact!("rustfmt.toml", Role::Static),
    seed!("src/lib.rs", Role::Static),
    seed!("src/messaging.rs", Role::Listed),
    seed!("src/routes.rs", Role::Listed),
    exact!("supply-chain/README.md", Role::Static),
    seed!("supply-chain/audits.toml", Role::Static),
    seed!("supply-chain/config.toml", Role::Static),
    exact!("taplo.toml", Role::Static),
    seed!("tests/messaging.rs", Role::Listed),
    seed!("tests/routes.rs", Role::Listed),
];

/// Returns the entry at `path`, a `/`-separated path relative to the tree's root.
#[must_use]
pub fn entry(path: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|entry| entry.path == path)
}
