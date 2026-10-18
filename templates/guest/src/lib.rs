//! The guest template every Omnia guest workspace starts from, as data a consumer scaffolds a tree from and holds it to.
//!
//! The contract is `templates/guest/manifest.yaml`, beside this crate.
//! [`ENTRIES`] is its entries as data, each body embedded at compile time,
//! which the `template-check` gate holds to the manifest entry for entry.
//! An [`Exact`](Proof::Exact) entry is this repository's own root file
//! read in place, so the tooling a guest starts with is the tooling this
//! repository's CI runs under, with no copy to keep in step; a
//! [`Seed`](Proof::Seed) entry is a project-start baseline under
//! `templates/guest/core/`, which the project evolves.
//!
//! A tree is scaffolded with [`scaffold`] and held with [`drift`] and [`restore`]:
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

pub use self::entries::{ENTRIES, Entry, Proof, Role, entry};
pub use self::hold::{Drift, Drifted, Error};
pub use self::manifest::Manifest;
pub use self::tree::{drift, restore, scaffold};
