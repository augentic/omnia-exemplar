//! The guest template every Omnia guest workspace starts from, as data a consumer lays and holds a tree to.
//!
//! [`ENTRIES`] is the template: every file of the tree, its body embedded
//! at compile time. An [`Exact`](Proof::Exact) entry is this repository's
//! own root file read in place, so the tooling a guest starts with is the
//! tooling this repository's CI runs under, with no copy to keep in step;
//! a [`Seed`](Proof::Seed) entry is a project-start baseline under
//! `templates/guest/core/`, which the project evolves. The two
//! [`TOKENS`] are the placeholders a seed body carries for the operator.
//!
//! A tree is laid with [`lay`] and held with [`drift`] and [`restore`]:
//! every [`Static`](Role::Static) file byte for byte, while a
//! [`Listed`](Role::Listed) file, which holds a list a slice appends its
//! lines to, is left to its consumer. The root `Cargo.toml` is the one
//! listed file held outside its lists, through [`Manifest`], since the
//! text around them is the pins every crate of the tree resolves under.
//!
//! The crate is consumed from a checkout of this repository, pinned to a
//! commit, and is never packaged: an exact body is read from two
//! directories above the package.

mod entries;
mod hold;
mod manifest;
mod tree;

pub use self::entries::{ENTRIES, Entry, Proof, Role, TOKENS, Token, entry};
pub use self::hold::{Drift, Drifted, Error};
pub use self::manifest::Manifest;
pub use self::tree::{drift, lay, restore};
