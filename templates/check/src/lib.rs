//! Producer gate for the guest template contract.
//!
//! Validates `exemplar.yaml` against `Cargo.toml` / `Cargo.lock`, strictly
//! parses `templates/guest/manifest.yaml`, enforces token discipline, renders
//! seed templates with `values.yaml`, syntax-checks the rendered output, and
//! byte-diffs every `exact` template against its repository-root counterpart.
//!
//! The gate runs inside the standard test suite (`cargo make test` /
//! `cargo make ci`) via `tests/gate.rs`, and stand-alone through
//! `cargo run --package template-check` — the root `Makefile.toml` is itself
//! an `exact` template, so no exemplar-only task may be added to it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Exemplar {
    schema_version: u32,
    omnia: OmniaPin,
    templates: Templates,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OmniaPin {
    version: String,
    repository: String,
    rev: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Templates {
    manifest: PathBuf,
}

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

    let exemplar: Exemplar = parse_yaml(&root.join("exemplar.yaml"))?;
    if exemplar.schema_version != 1 {
        failures.push(format!(
            "exemplar.yaml: unsupported schema-version {} (expected 1)",
            exemplar.schema_version
        ));
    }
    check_cargo_pins(root, &exemplar.omnia, &mut failures)?;

    let manifest_path = root.join(&exemplar.templates.manifest);
    let manifest: Manifest = parse_yaml(&manifest_path)?;
    if manifest.schema_version != 2 {
        failures.push(format!(
            "manifest: unsupported schema-version {} (expected 2)",
            manifest.schema_version
        ));
    }
    if manifest.assemblies.core.path_mode != "content-only" {
        failures.push(format!(
            "manifest: unsupported path-mode `{}`",
            manifest.assemblies.core.path_mode
        ));
    }

    let template_dir =
        manifest_path.parent().ok_or("manifest has no parent directory")?.to_path_buf();
    let values: BTreeMap<String, String> = parse_yaml(&template_dir.join("values.yaml"))?;

    check_token_declarations(&manifest.tokens, &values, &mut failures);
    check_entries(root, &template_dir, &manifest, &values, &mut failures);
    check_orphans(&template_dir, &manifest, &mut failures)?;

    Ok(failures)
}

fn parse_yaml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_saphyr::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
}

/// `Cargo.toml` patch entries for the Omnia repository must carry the
/// declared rev, and `Cargo.lock` must have resolved every Omnia package to
/// it — the manifest, not an incidental lock resolution, declares
/// compatibility.
fn check_cargo_pins(root: &Path, pin: &OmniaPin, failures: &mut Vec<String>) -> Result<(), String> {
    let cargo_toml: toml::Value = parse_toml(&root.join("Cargo.toml"))?;
    let patches = cargo_toml
        .get("patch")
        .and_then(|patch| patch.get("crates-io"))
        .and_then(toml::Value::as_table)
        .ok_or("Cargo.toml: no [patch.crates-io] table")?;

    let mut pinned_count = 0_u32;
    for (name, entry) in patches {
        let Some(git) = entry.get("git").and_then(toml::Value::as_str) else {
            continue;
        };
        if git.trim_end_matches(".git") != pin.repository {
            continue;
        }
        pinned_count += 1;
        match entry.get("rev").and_then(toml::Value::as_str) {
            Some(rev) if rev == pin.rev => {}
            Some(rev) => failures.push(format!(
                "Cargo.toml: patch `{name}` pins rev {rev}, exemplar.yaml declares {}",
                pin.rev
            )),
            None => failures.push(format!(
                "Cargo.toml: patch `{name}` has no rev; exemplar.yaml declares {}",
                pin.rev
            )),
        }
    }
    if pinned_count == 0 {
        failures.push(format!("Cargo.toml: no [patch.crates-io] entry for {}", pin.repository));
    }

    let cargo_lock: toml::Value = parse_toml(&root.join("Cargo.lock"))?;
    let packages = cargo_lock
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("Cargo.lock: no [[package]] entries")?;
    for package in packages {
        let name = package.get("name").and_then(toml::Value::as_str).unwrap_or_default();
        let Some(source) = package.get("source").and_then(toml::Value::as_str) else {
            continue;
        };
        if !source.starts_with("git+") || !source.contains(&pin.repository) {
            continue;
        }
        if !source.contains(&pin.rev) {
            failures.push(format!(
                "Cargo.lock: `{name}` resolved from `{source}`, not rev {}",
                pin.rev
            ));
        }
        let version = package.get("version").and_then(toml::Value::as_str).unwrap_or_default();
        if name == "omnia" && version != pin.version {
            failures.push(format!(
                "Cargo.lock: omnia is {version}, exemplar.yaml declares {}",
                pin.version
            ));
        }
    }
    Ok(())
}

fn parse_toml(path: &Path) -> Result<toml::Value, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    toml::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
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
    root: &Path, template_dir: &Path, manifest: &Manifest, values: &BTreeMap<String, String>,
    failures: &mut Vec<String>,
) {
    let mut targets = BTreeSet::new();
    let mut used_tokens = BTreeSet::new();

    for entry in &manifest.assemblies.core.files {
        if !targets.insert(entry.target.as_str()) {
            failures.push(format!("manifest: duplicate target `{}`", entry.target));
        }
        if entry.target.starts_with('/') || entry.target.split('/').any(|seg| seg == "..") {
            failures.push(format!("manifest: unsafe target path `{}`", entry.target));
        }

        let source_path = template_dir.join("core").join(&entry.source);
        let Ok(body) = fs::read_to_string(&source_path) else {
            failures.push(format!("manifest: missing source `core/{}`", entry.source));
            continue;
        };

        let found = placeholders(&body);
        for token in &found {
            used_tokens.insert(token.clone());
            if !manifest.tokens.contains_key(token) {
                failures.push(format!("core/{}: undeclared token `<{token}>`", entry.source));
            }
        }
        if entry.proof == Proof::Exact && !found.is_empty() {
            failures.push(format!(
                "core/{}: proof `exact` forbids tokens, found {}",
                entry.source,
                found.iter().map(|token| format!("`<{token}>`")).collect::<Vec<_>>().join(", ")
            ));
        }

        let rendered = render(&body, values);
        if let Some(unresolved) = placeholders(&rendered).first() {
            failures.push(format!(
                "core/{}: token `<{unresolved}>` unresolved after rendering",
                entry.source
            ));
        }
        if let Some(err) = syntax_error(&entry.target, &rendered) {
            failures.push(format!("core/{}: rendered output is invalid: {err}", entry.source));
        }

        if entry.proof == Proof::Exact {
            let root_path = root.join(&entry.target);
            match fs::read_to_string(&root_path) {
                Ok(root_body) if root_body == body => {}
                Ok(_) => failures.push(format!(
                    "core/{}: differs from `{}` — exact templates must byte-match the \
                     repository root (authorship flows templates -> root)",
                    entry.source, entry.target
                )),
                Err(err) => failures.push(format!(
                    "core/{}: proof `exact` but `{}` is unreadable: {err}",
                    entry.source, entry.target
                )),
            }
        }
    }

    for token in manifest.tokens.keys() {
        if !used_tokens.contains(token) {
            failures.push(format!("manifest: declared token `{token}` appears in no template"));
        }
    }
}

/// Files under `core/` that the manifest does not map are dead weight the
/// adapter's `build.rs` would also reject.
fn check_orphans(
    template_dir: &Path, manifest: &Manifest, failures: &mut Vec<String>,
) -> Result<(), String> {
    let listed: BTreeSet<&str> =
        manifest.assemblies.core.files.iter().map(|entry| entry.source.as_str()).collect();
    let core_dir = template_dir.join("core");
    let entries =
        fs::read_dir(&core_dir).map_err(|err| format!("{}: {err}", core_dir.display()))?;
    for dir_entry in entries {
        let dir_entry = dir_entry.map_err(|err| format!("{}: {err}", core_dir.display()))?;
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        if !listed.contains(name.as_str()) {
            failures.push(format!("core/{name}: not listed in the manifest (orphan)"));
        }
    }
    Ok(())
}

/// `<UPPER_SNAKE>` tokens in first-appearance order, deduplicated.
fn placeholders(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0;
    while let Some(open) = body[index..].find('<').map(|offset| index + offset) {
        let rest = &body[open + 1..];
        let end = rest.find('>');
        if let Some(end) = end {
            let candidate = &rest[..end];
            let is_token = !candidate.is_empty()
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
