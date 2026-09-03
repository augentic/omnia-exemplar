//! Producer gate for the guest template contract.
//!
//! Strictly parses `templates/guest/manifest.yaml`, enforces token
//! discipline and path safety, requires every `exact` entry to reference
//! its repository-root file in place (`source == target`, token-free —
//! no second copy), renders `seed` templates with `values.yaml`, and
//! syntax-checks the rendered output.
//!
//! The gate runs inside the standard test suite (`cargo make test` /
//! `cargo make ci`) via `tests/gate.rs`, and stand-alone through
//! `cargo run --package template-check` — the root `Makefile.toml` is itself
//! an `exact` template, so no exemplar-only task may be added to it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const MANIFEST: &str = "templates/guest/manifest.yaml";
const SUBTREE: &str = "templates/guest";

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Manifest {
    schema_version: u32,
    tokens: BTreeMap<String, String>,
    assemblies: Assemblies,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Assemblies {
    core: Assembly,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Assembly {
    path_mode: String,
    files: Vec<FileEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileEntry {
    source: String,
    target: String,
    proof: Proof,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Proof {
    Exact,
    Seed,
}

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

    let manifest: Manifest = parse_yaml(&root.join(MANIFEST))?;
    if manifest.schema_version != 3 {
        failures.push(format!(
            "manifest: unsupported schema-version {} (expected 3)",
            manifest.schema_version
        ));
    }
    if manifest.assemblies.core.path_mode != "content-only" {
        failures.push(format!(
            "manifest: unsupported path-mode `{}`",
            manifest.assemblies.core.path_mode
        ));
    }

    let subtree = PathBuf::from(SUBTREE);
    let values: BTreeMap<String, String> = parse_yaml(&root.join(&subtree).join("values.yaml"))?;

    check_token_declarations(&manifest.tokens, &values, &mut failures);
    check_entries(root, &subtree, &manifest, &values, &mut failures);
    check_orphans(root, &subtree, &manifest, &mut failures)?;

    Ok(failures)
}

fn parse_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_saphyr::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// Every declared token needs a render value; every value needs a
/// declaration.
fn check_token_declarations(
    tokens: &BTreeMap<String, String>, values: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    for token in tokens.keys() {
        if !values.contains_key(token) {
            failures.push(format!("values.yaml: no render value for declared token `{token}`"));
        }
    }
    for value in values.keys() {
        if !tokens.contains_key(value) {
            failures.push(format!("values.yaml: `{value}` is not a declared token"));
        }
    }
}

fn check_entries(
    root: &Path, subtree: &Path, manifest: &Manifest, values: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    let mut targets = BTreeSet::new();
    let mut used_tokens = BTreeSet::new();

    for entry in &manifest.assemblies.core.files {
        if !targets.insert(entry.target.as_str()) {
            failures.push(format!("manifest: duplicate target `{}`", entry.target));
        }
        for (label, path) in [("source", &entry.source), ("target", &entry.target)] {
            if is_unsafe(path) {
                failures.push(format!("manifest: unsafe {label} path `{path}`"));
            }
        }

        let Ok(body) = fs::read_to_string(root.join(&entry.source)) else {
            failures.push(format!("manifest: missing source `{}`", entry.source));
            continue;
        };

        let found = placeholders(&body);
        for token in &found {
            used_tokens.insert(token.clone());
            if !manifest.tokens.contains_key(token) {
                failures.push(format!("{}: undeclared token `<{token}>`", entry.source));
            }
        }

        match entry.proof {
            Proof::Exact => {
                if entry.source != entry.target {
                    failures.push(format!(
                        "manifest: proof `exact` requires source == target, got `{}` -> `{}` — \
                         exact entries reference the repository-root file in place",
                        entry.source, entry.target
                    ));
                }
                if !found.is_empty() {
                    failures.push(format!(
                        "{}: proof `exact` forbids tokens, found {}",
                        entry.source,
                        found
                            .iter()
                            .map(|token| format!("`<{token}>`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            Proof::Seed => {
                if !Path::new(&entry.source).starts_with(subtree) {
                    failures.push(format!(
                        "manifest: seed source `{}` lives outside `{}`",
                        entry.source,
                        subtree.display()
                    ));
                }
                let rendered = render(&body, values);
                if let Some(unresolved) = placeholders(&rendered).first() {
                    failures.push(format!(
                        "{}: token `<{unresolved}>` unresolved after rendering",
                        entry.source
                    ));
                }
                if let Some(err) = syntax_error(&entry.target, &rendered) {
                    failures.push(format!("{}: rendered output is invalid: {err}", entry.source));
                }
                if entry.target == "Cargo.toml" {
                    check_seed_versions(root, &entry.source, &rendered, failures);
                }
            }
        }
    }

    for token in manifest.tokens.keys() {
        if !used_tokens.contains(token) {
            failures.push(format!("manifest: declared token `{token}` appears in no template"));
        }
    }
}

/// A seed `Cargo.toml` pins every dependency it shares with the root
/// workspace at the workspace's version, so the scaffold tracks the omnia
/// rev the exemplar itself builds against.
fn check_seed_versions(root: &Path, source: &str, rendered: &str, failures: &mut Vec<String>) {
    let Ok(workspace) = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|err| err.to_string())
        .and_then(|text| toml::from_str::<toml::Value>(&text).map_err(|err| err.to_string()))
    else {
        failures.push("Cargo.toml: root manifest unreadable".to_string());
        return;
    };
    let Ok(seed) = toml::from_str::<toml::Value>(rendered) else {
        return;
    };
    let workspace_deps = workspace.get("workspace").and_then(|w| w.get("dependencies"));

    for (name, seed_dep) in dependency_tables(&seed) {
        let Some(expected) = workspace_deps.and_then(|deps| deps.get(&name)).and_then(version)
        else {
            continue;
        };
        let actual = version(seed_dep).unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "{source}: `{name}` pins {actual:?}, the workspace pins {expected:?}"
            ));
        }
    }
}

/// Every `[dependencies]`-shaped table in a package manifest, including the
/// target-gated ones, flattened to `(name, spec)`.
fn dependency_tables(manifest: &toml::Value) -> Vec<(String, &toml::Value)> {
    const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut tables = Vec::new();
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

/// Write every manifest entry, rendered with `values.yaml`, under `dest`:
/// the project a consumer build starts from.
///
/// # Errors
///
/// Returns `Err` when the manifest, the values, or a source cannot be read,
/// or when a target cannot be written.
pub fn scaffold(root: &Path, dest: &Path) -> Result<(), String> {
    let manifest: Manifest = parse_yaml(&root.join(MANIFEST))?;
    let values: BTreeMap<String, String> = parse_yaml(&root.join(SUBTREE).join("values.yaml"))?;

    for entry in &manifest.assemblies.core.files {
        let source = root.join(&entry.source);
        let body =
            fs::read_to_string(&source).map_err(|err| format!("{}: {err}", source.display()))?;
        let target = dest.join(&entry.target);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        fs::write(&target, render(&body, &values))
            .map_err(|err| format!("{}: {err}", target.display()))?;
    }
    Ok(())
}

/// Absolute or parent-traversing paths never belong in the manifest.
fn is_unsafe(path: &str) -> bool {
    Path::new(path).is_absolute() || path.split('/').any(|segment| segment == "..")
}

/// Files under the seed directory that the manifest does not map are dead
/// weight the adapter's runtime scaffold would also reject.
fn check_orphans(
    root: &Path, subtree: &Path, manifest: &Manifest, failures: &mut Vec<String>,
) -> Result<(), String> {
    let listed: BTreeSet<PathBuf> =
        manifest.assemblies.core.files.iter().map(|entry| PathBuf::from(&entry.source)).collect();
    let core_dir = root.join(subtree).join("core");
    let entries =
        fs::read_dir(&core_dir).map_err(|err| format!("{}: {err}", core_dir.display()))?;
    for dir_entry in entries {
        let dir_entry = dir_entry.map_err(|err| format!("{}: {err}", core_dir.display()))?;
        let relative = subtree.join("core").join(dir_entry.file_name());
        if !listed.contains(&relative) {
            failures.push(format!("{}: not listed in the manifest (orphan)", relative.display()));
        }
    }
    Ok(())
}

/// `<UPPER_SNAKE>` tokens in first-appearance order, deduplicated.
///
/// A lone uppercase letter (`<P>`, `<T>`) is a Rust generic in a seed
/// source, not a token.
fn placeholders(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while let Some(open) = body[index..].find('<').map(|offset| index + offset) {
        let rest = &body[open + 1..];
        let end = rest.find('>');
        if let Some(end) = end {
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
        debug_assert!(index <= bytes.len());
    }
    found
}

fn render(body: &str, values: &BTreeMap<String, String>) -> String {
    let mut rendered = body.to_string();
    for (token, value) in values {
        rendered = rendered.replace(&format!("<{token}>"), value);
    }
    rendered
}

/// Parse rendered output by target extension; `None` means valid or not a
/// checked format.
fn syntax_error(target: &str, rendered: &str) -> Option<String> {
    let extension = Path::new(target).extension().and_then(|ext| ext.to_str());
    match extension {
        Some("toml") => toml::from_str::<toml::Value>(rendered).err().map(|err| err.to_string()),
        Some("json") => {
            serde_json::from_str::<serde_json::Value>(rendered).err().map(|err| err.to_string())
        }
        Some("yaml" | "yml") => {
            serde_saphyr::from_str::<serde_json::Value>(rendered).err().map(|err| err.to_string())
        }
        _ => None,
    }
}
