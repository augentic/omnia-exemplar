//! The scaffold proof: a project laid from the template passes the checks
//! a build is held to, with no hand edits.
//!
//! The scaffold builds against the same published omnia the exemplar does —
//! the root lockfile seeds the scaffold's, so every crates.io dependency
//! resolves to the exemplar's exact version — and shares the exemplar's
//! target directory so dependencies compile once.

#![cfg(not(miri))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

// The checks are the omnia target adapter's verify over an integrated tree,
// so a template that passes here passes a wave whose slices wrote nothing.
#[test]
fn laid_scaffold() {
    let root = template_check::repo_root();
    let target_dir =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let dest = target_dir.join("template-scaffold");
    if dest.exists() {
        fs::remove_dir_all(&dest).expect("stale scaffold removed");
    }
    fs::create_dir_all(&dest).expect("scaffold root");

    let laid = omnia_template::lay(&dest).expect("the template lays");
    assert_eq!(laid.len(), omnia_template::ENTRIES.len(), "every file is laid");
    fs::copy(root.join("Cargo.lock"), dest.join("Cargo.lock")).expect("lockfile seeded");

    cargo(&dest, &target_dir, &["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
    cargo(&dest, &target_dir, &["test", "--workspace"]);
    cargo(&dest, &target_dir, &["build", "--target", "wasm32-wasip2"]);
    assert!(
        target_dir.join("wasm32-wasip2/debug/guest.wasm").is_file(),
        "the guest component is built"
    );
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
