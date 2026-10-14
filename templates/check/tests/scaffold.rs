//! The scaffold proof: a project rendered from the manifest passes the
//! checks a build is held to, with no hand edits.
//!
//! The scaffold builds against the same published omnia the exemplar does:
//! the root lockfile seeds the scaffold's, so every crates.io dependency
//! resolves to the exemplar's exact version. The native checks share the
//! exemplar's target directory so dependencies compile once; both packages
//! are `guest`, so the `libguest` the exemplar uplifts to `target/debug` is
//! the scaffold's after a run, which nothing reads. The component is built
//! under a target directory of its own, since the exemplar's holds no
//! `wasm32-wasip2` build to share and the one artifact asserted must be
//! this render's.

#![cfg(not(miri))]

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

// The checks are the omnia target adapter's verify over an integrated tree,
// so a template that passes here passes a wave whose slices wrote nothing.
// Every feature is on, as CI has it, so the `host` example compiles too.
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

    let clippy =
        ["clippy", "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"];
    cargo(&dest, &target_dir, &clippy);
    cargo(&dest, &target_dir, &["test", "--workspace", "--all-features"]);

    // the component, from this build alone
    let wasm_target_dir = target_dir.join("template-scaffold-wasm32");
    let component = wasm_target_dir.join("wasm32-wasip2/debug/guest.wasm");
    if let Err(err) = fs::remove_file(&component)
        && err.kind() != ErrorKind::NotFound
    {
        panic!("stale component removed: {err}");
    }
    cargo(&dest, &wasm_target_dir, &["build", "--target", "wasm32-wasip2"]);
    assert!(component.is_file(), "the guest component is built");
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
