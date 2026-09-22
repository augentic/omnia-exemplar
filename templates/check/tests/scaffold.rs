//! The scaffold proof: a project rendered from the manifest builds for
//! wasm32 and passes its native route test with no hand edits.
//!
//! The scaffold builds against the same published omnia the exemplar does —
//! the root lockfile seeds the scaffold's, so every crates.io dependency
//! resolves to the exemplar's exact version — and shares the exemplar's
//! target directory so dependencies compile once.

#![cfg(not(miri))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// A project freshly rendered from the manifest, with no hand edits.
#[test]
fn rendered_scaffold() {
    let root = template_check::repo_root();
    let target_dir =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let dest = target_dir.join("template-scaffold");
    if dest.exists() {
        fs::remove_dir_all(&dest).expect("stale scaffold removed");
    }

    template_check::scaffold(&root, &dest).expect("scaffold renders");
    fs::copy(root.join("Cargo.lock"), dest.join("Cargo.lock")).expect("lockfile seeded");

    cargo(&dest, &target_dir, &["build", "--target", "wasm32-wasip2"]);
    cargo(&dest, &target_dir, &["test"]);
}

fn cargo(dir: &Path, target_dir: &Path, args: &[&str]) {
    let cargo = env::var_os("CARGO").map_or_else(|| PathBuf::from("cargo"), PathBuf::from);
    let output = Command::new(cargo)
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "`cargo {}` failed in {}:\n{}",
        args.join(" "),
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}
