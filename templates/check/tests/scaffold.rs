//! The scaffold proof: a project rendered from the manifest builds for
//! wasm32 and passes its native route test with no hand edits.
//!
//! The scaffold builds against the same omnia the exemplar does — the root
//! `[patch.crates-io]` block is carried over with its paths made absolute,
//! and the root lockfile seeds the scaffold's — and shares the exemplar's
//! target directory so dependencies compile once.

#![cfg(not(miri))]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

#[test]
fn scaffold_builds_and_tests() {
    let root = template_check::repo_root();
    let target_dir =
        env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let dest = target_dir.join("template-scaffold");
    if dest.exists() {
        fs::remove_dir_all(&dest).expect("stale scaffold removed");
    }

    template_check::scaffold(&root, &dest).expect("scaffold renders");
    carry_patches(&root, &dest);
    fs::copy(root.join("Cargo.lock"), dest.join("Cargo.lock")).expect("lockfile seeded");

    cargo(&dest, &target_dir, &["build", "--target", "wasm32-wasip2"]);
    cargo(&dest, &target_dir, &["test"]);
}

/// Append the root `[patch]` block so the scaffold resolves omnia exactly as
/// the exemplar does, with relative path patches made absolute.
fn carry_patches(root: &Path, dest: &Path) {
    let text = fs::read_to_string(root.join("Cargo.toml")).expect("root manifest");
    let mut manifest: toml::Value = toml::from_str(&text).expect("root manifest parses");
    let Some(patch) = manifest.get_mut("patch") else {
        return;
    };
    for registry in patch.as_table_mut().into_iter().flat_map(|t| t.iter_mut()) {
        for (_, spec) in registry.1.as_table_mut().into_iter().flat_map(|t| t.iter_mut()) {
            if let Some(path) = spec.get("path").and_then(toml::Value::as_str) {
                let absolute = root.join(path).display().to_string();
                spec["path"] = toml::Value::String(absolute);
            }
        }
    }

    let mut block = toml::Table::new();
    block.insert("patch".to_string(), patch.clone());
    let block = toml::to_string(&block).expect("patch block serializes");

    let manifest_path = dest.join("Cargo.toml");
    let mut seeded = fs::read_to_string(&manifest_path).expect("scaffold manifest");
    seeded.push('\n');
    seeded.push_str(&block);
    fs::write(&manifest_path, seeded).expect("scaffold manifest written");
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
