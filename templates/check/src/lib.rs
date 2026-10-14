//! Producer gate for the guest template contract.
//!
//! Holds `omnia-template`'s table to this repository: every `exact` entry
//! is the repository-root file at its path, token-free; every `seed` body
//! is the file under `templates/guest/core/` at its path and carries only
//! declared tokens; every body that is TOML, JSON, or YAML parses as
//! written; the seed's dependency pins are the workspace's; nothing under
//! `core/` is left out of the table; and every toolkit reference names one
//! tag.
//!
//! The gate runs inside the standard test suite (`mise run test` /
//! `mise run ci`) via `tests/gate.rs`, and stand-alone through
//! `cargo run --package template-check` — the root `mise.toml` is itself
//! an `exact` entry, so no exemplar-only task may be added to it.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use omnia_template::{ENTRIES, Proof, Role, TOKENS};

const CORE: &str = "templates/guest/core";

/// The repository root this crate is checked out under.
///
/// # Panics
///
/// Panics when the crate directory cannot be canonicalized.
#[must_use]
pub fn repo_root() -> PathBuf {
    // templates/check/ -> repository root.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repository root")
}

/// Run every contract check against the repository at `root`.
///
/// # Errors
///
/// Returns `Err` when a required input cannot be read or parsed at all;
/// `Ok` carries the (possibly empty) list of contract violations.
pub fn run(root: &Path) -> Result<Vec<String>, String> {
    let mut failures = Vec::new();

    check_table(&mut failures);
    check_bodies(root, &mut failures);
    check_tokens(&mut failures);
    check_syntax(&mut failures);
    check_pins(root, &mut failures)?;
    check_orphans(root, &mut failures)?;
    check_toolkit(&mut failures);

    Ok(failures)
}

fn check_table(failures: &mut Vec<String>) {
    let paths: Vec<&str> = ENTRIES.iter().map(|entry| entry.path).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    if paths != sorted {
        failures.push("table: not sorted by path".to_string());
    }
    let mut seen = BTreeSet::new();
    for entry in ENTRIES {
        if !seen.insert(entry.path) {
            failures.push(format!("table: duplicate path `{}`", entry.path));
        }
        if is_unsafe(entry.path) {
            failures.push(format!("table: unsafe path `{}`", entry.path));
        }
        if entry.proof == Proof::Exact && entry.role != Role::Static {
            failures.push(format!("table: exact entry `{}` is not static", entry.path));
        }
    }
}

// Both proofs read the file the body was embedded from, so a moved file or
// a mis-spelled path surfaces here rather than in a consumer.
fn check_bodies(root: &Path, failures: &mut Vec<String>) {
    for entry in ENTRIES {
        let source = match entry.proof {
            Proof::Exact => root.join(entry.path),
            Proof::Seed => root.join(CORE).join(entry.path),
        };
        match fs::read_to_string(&source) {
            Ok(body) if body == entry.body => {}
            Ok(_) => failures.push(format!(
                "{}: the embedded body differs from `{}`",
                entry.path,
                source.display()
            )),
            Err(err) => failures.push(format!("{}: `{}`: {err}", entry.path, source.display())),
        }
    }
}

fn check_tokens(failures: &mut Vec<String>) {
    let declared: BTreeSet<&str> = TOKENS.iter().map(|token| token.name).collect();
    let mut used = BTreeSet::new();
    for entry in ENTRIES {
        let found = placeholders(entry.body);
        match entry.proof {
            Proof::Exact if !found.is_empty() => failures.push(format!(
                "{}: proof `exact` forbids tokens, found {}",
                entry.path,
                found.iter().map(|token| format!("`<{token}>`")).collect::<Vec<_>>().join(", ")
            )),
            Proof::Exact => {}
            Proof::Seed => {
                for token in found {
                    if !declared.contains(token.as_str()) {
                        failures.push(format!("{}: undeclared token `<{token}>`", entry.path));
                    }
                    used.insert(token);
                }
            }
        }
    }
    for token in declared {
        if !used.contains(token) {
            failures.push(format!("table: declared token `{token}` appears in no body"));
        }
    }
}

fn check_syntax(failures: &mut Vec<String>) {
    for entry in ENTRIES {
        if let Some(err) = syntax_error(entry.path, entry.body) {
            failures.push(format!("{}: invalid: {err}", entry.path));
        }
    }
}

// The seed pins every dependency it shares with the root workspace at the
// workspace's version, so a scaffold tracks the omnia the exemplar itself
// builds against.
fn check_pins(root: &Path, failures: &mut Vec<String>) -> Result<(), String> {
    let workspace: toml::Value = parse_toml(&root.join("Cargo.toml"))?;
    let Some(seed) = ENTRIES.iter().find(|entry| entry.path == "Cargo.toml") else {
        failures.push("table: no `Cargo.toml` entry".to_string());
        return Ok(());
    };
    let Ok(seed) = toml::from_str::<toml::Value>(seed.body) else {
        return Ok(());
    };
    let workspace_deps = workspace.get("workspace").and_then(|w| w.get("dependencies"));

    for (name, seed_dep) in dependency_tables(&seed) {
        let Some(actual) = version(seed_dep) else {
            continue;
        };
        let Some(expected) = workspace_deps.and_then(|deps| deps.get(&name)).and_then(version)
        else {
            continue;
        };
        if actual != expected {
            failures.push(format!(
                "Cargo.toml: `{name}` pins {actual:?}, the workspace pins {expected:?}"
            ));
        }
    }
    Ok(())
}

fn check_orphans(root: &Path, failures: &mut Vec<String>) -> Result<(), String> {
    let core = root.join(CORE);
    let seeds: BTreeSet<&str> =
        ENTRIES.iter().filter(|entry| entry.proof == Proof::Seed).map(|entry| entry.path).collect();
    let mut files = Vec::new();
    walk(&core, &mut files)?;
    for file in files {
        let relative = file.strip_prefix(&core).map_err(|err| err.to_string())?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        if !seeds.contains(relative.as_str()) {
            failures.push(format!("{CORE}/{relative}: not in the table (orphan)"));
        }
    }
    Ok(())
}

fn check_toolkit(failures: &mut Vec<String>) {
    let tags: BTreeSet<String> =
        ENTRIES.iter().flat_map(|entry| entry.body.lines()).filter_map(toolkit_tag).collect();
    if tags.len() != 1 {
        failures.push(format!(
            "toolkit: the bodies name {} tags: {}",
            tags.len(),
            tags.iter().map(|tag| format!("`{tag}`")).collect::<Vec<_>>().join(", ")
        ));
    }
}

fn parse_toml(path: &Path) -> Result<toml::Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

// Every `[dependencies]`-shaped table of a manifest, the workspace's and
// the target-gated ones included, flattened to `(name, spec)`.
fn dependency_tables(manifest: &toml::Value) -> Vec<(String, &toml::Value)> {
    const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut tables = Vec::new();
    if let Some(table) = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(toml::Value::as_table)
    {
        tables.push(table);
    }
    for kind in KINDS {
        if let Some(table) = manifest.get(kind).and_then(toml::Value::as_table) {
            tables.push(table);
        }
    }
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for target in targets.values() {
            for kind in KINDS {
                if let Some(table) = target.get(kind).and_then(toml::Value::as_table) {
                    tables.push(table);
                }
            }
        }
    }
    tables
        .into_iter()
        .flat_map(|table| table.iter().map(|(name, spec)| (name.clone(), spec)))
        .collect()
}

fn version(spec: &toml::Value) -> Option<String> {
    match spec {
        toml::Value::String(version) => Some(version.clone()),
        toml::Value::Table(table) => table.get("version")?.as_str().map(str::to_owned),
        _ => None,
    }
}

fn is_unsafe(path: &str) -> bool {
    Path::new(path).is_absolute() || path.split('/').any(|segment| segment == "..")
}

// Every file beneath `dir`, hidden directories included.
fn walk(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("{}: {err}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

// `<UPPER_SNAKE>` tokens in first-appearance order, deduplicated. A lone
// uppercase letter (`<P>`, `<T>`) is a Rust generic in a seed source, not
// a token.
fn placeholders(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut index = 0;
    while let Some(open) = body[index..].find('<').map(|offset| index + offset) {
        let rest = &body[open + 1..];
        if let Some(end) = rest.find('>') {
            let candidate = &rest[..end];
            let is_token = candidate.len() >= 2
                && candidate.as_bytes()[0].is_ascii_uppercase()
                && candidate
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            if is_token && !found.iter().any(|existing| existing == candidate) {
                found.push(candidate.to_string());
            }
        }
        index = open + 1;
    }
    found
}

// The tag after `@v` or `?ref=v` of a toolkit reference on `line`.
fn toolkit_tag(line: &str) -> Option<String> {
    let (_, rest) = line.split_once("augentic/toolkit")?;
    let tag = rest.split_once("@v").or_else(|| rest.split_once("?ref=v"))?.1;
    Some(tag.trim_end_matches(['"', ']']).to_owned())
}

// Parses a body by its path's extension; `None` is valid, or not a checked
// format.
fn syntax_error(path: &str, body: &str) -> Option<String> {
    let extension = Path::new(path).extension().and_then(|ext| ext.to_str());
    match extension {
        Some("toml") => toml::from_str::<toml::Value>(body).err().map(|err| err.to_string()),
        Some("json") => {
            serde_json::from_str::<serde_json::Value>(body).err().map(|err| err.to_string())
        }
        Some("yaml" | "yml") => {
            serde_saphyr::from_str::<serde_json::Value>(body).err().map(|err| err.to_string())
        }
        _ => None,
    }
}
