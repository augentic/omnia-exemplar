//! Producer gate for the guest template contract.
//!
//! Strictly parses `templates/guest/manifest.yaml`, enforces token
//! discipline and path safety, requires every `exact` entry to reference
//! its repository-root file in place (`source == target`, token-free —
//! no second copy), renders `seed` templates with `values.yaml`,
//! syntax-checks the rendered output, and holds the `omnia-template`
//! crate's table to the manifest entry for entry, each embedded body the
//! bytes of its source.
//!
//! The gate runs inside the standard test suite (`mise run test` /
//! `mise run ci`) via `tests/gate.rs`, and stand-alone through
//! `cargo run --package template-check` — the root `mise.toml` is itself
//! an `exact` template, so no exemplar-only task may be added to it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use omnia_template::ENTRIES;
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Proof {
    Exact,
    Seed,
}

impl Proof {
    const fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Seed => "seed",
        }
    }
}

impl From<omnia_template::Proof> for Proof {
    fn from(proof: omnia_template::Proof) -> Self {
        match proof {
            omnia_template::Proof::Exact => Self::Exact,
            omnia_template::Proof::Seed => Self::Seed,
        }
    }
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
    check_crate(root, &manifest, &mut failures);

    Ok(failures)
}

// The crate's table is the manifest as data, so the two list the same
// entries, the same source read for the same target under the same proof,
// and a row's body is the bytes of the source it names: `lay` and
// `restore` write the body, while the checks above read the source. A
// source that cannot be read is reported with the manifest's entry.
fn check_crate(root: &Path, manifest: &Manifest, failures: &mut Vec<String>) {
    let declared: BTreeSet<(&str, &str, Proof)> = manifest
        .assemblies
        .core
        .files
        .iter()
        .map(|entry| (entry.source.as_str(), entry.target.as_str(), entry.proof))
        .collect();
    let embedded: BTreeSet<(&str, &str, Proof)> =
        ENTRIES.iter().map(|entry| (entry.source, entry.path, entry.proof.into())).collect();

    for (source, target, proof) in declared.difference(&embedded) {
        failures.push(format!(
            "manifest: `{source}` -> `{target}` ({}) has no row in the crate's table",
            proof.name()
        ));
    }
    for (source, target, proof) in embedded.difference(&declared) {
        failures.push(format!(
            "omnia-template: `{source}` -> `{target}` ({}) is not a manifest entry",
            proof.name()
        ));
    }

    for entry in ENTRIES {
        if let Ok(on_disk) = fs::read(root.join(entry.source))
            && on_disk != entry.body.as_bytes()
        {
            failures.push(format!(
                "omnia-template: `{}` -> `{}` embeds a body other than its source",
                entry.source, entry.path
            ));
        }
    }
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
        // cargo probes `.cargo` by name, which a case-insensitive volume
        // answers for any spelling
        let cargo_config =
            |part: std::path::Component<'_>| part.as_os_str().eq_ignore_ascii_case(".cargo");
        if Path::new(&entry.target).components().any(cargo_config) {
            failures.push(format!(
                "manifest: target `{}` configures cargo behind the pins",
                entry.target
            ));
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
                if Path::new(&entry.target).file_name().is_some_and(|name| name == "Cargo.toml") {
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

// A seed `Cargo.toml` resolves every crate as the root workspace does, so
// the scaffold compiles nothing the exemplar's own supply-chain gates have
// not seen.
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
    for failure in seed_dependency_failures(&workspace, &seed) {
        failures.push(format!("{source}: {failure}"));
    }
}

// Every dependency of the seed that does not resolve as the root workspace
// would have it. A dependency has one of three shapes, and anything else
// fails:
//
// - `workspace = true` inherits the seed's own table, checked in its turn
// - a `path` inside the tree is a crate of the tree, which the root does
//   not pin
// - a `version` is the root's pin for that name
//
// Beside its shape a dependency sets feature keys alone. The admitted keys
// are a closed list, so `package`, `git`, `registry`, `branch`, `rev`, and
// `tag` each fail whatever the version reads, as do a `[patch]` or
// `[replace]` table.
fn seed_dependency_failures(workspace: &toml::Value, seed: &toml::Value) -> Vec<String> {
    const SHAPE_KEYS: [&str; 3] = ["workspace", "path", "version"];
    const FEATURE_KEYS: [&str; 3] = ["features", "default-features", "optional"];
    let pins = workspace.get("workspace").and_then(|w| w.get("dependencies"));
    let mut failures = Vec::new();

    for table in ["patch", "replace"] {
        if seed.get(table).is_some() {
            failures.push(format!("`[{table}]` retargets crates behind the pins"));
        }
    }

    for (name, spec) in dependency_tables(seed) {
        let table = spec.as_table();
        let mut keys = table.into_iter().flat_map(toml::Table::keys).map(String::as_str);
        if let Some(key) = keys.find(|key| !SHAPE_KEYS.contains(key) && !FEATURE_KEYS.contains(key))
        {
            failures.push(format!("`{name}` sets `{key}`, which a seed dependency never does"));
            continue;
        }

        let key = |key: &str| table.and_then(|table| table.get(key));
        let inherits = key("workspace").and_then(toml::Value::as_bool) == Some(true);
        let path = key("path").and_then(toml::Value::as_str);
        let pinned = pins.and_then(|pins| pins.get(&name)).and_then(version);
        match (inherits, path, version(spec)) {
            (true, None, None) => {}
            (false, Some(path), None) if is_unsafe(path) => {
                failures.push(format!("`{name}` is a path outside the tree"));
            }
            (false, Some(_), None) => {
                if let Some(pinned) = pinned {
                    failures.push(format!("`{name}` is a path, the root pins {pinned:?}"));
                }
            }
            (false, None, Some(actual)) => match pinned {
                Some(expected) if expected == actual => {}
                Some(expected) => {
                    failures.push(format!("`{name}` pins {actual:?}, the root pins {expected:?}"));
                }
                None => failures.push(format!("`{name}` is not a pin of the root workspace")),
            },
            _ => failures.push(format!(
                "`{name}` is none of an inherited pin, a path in the tree, or a version"
            )),
        }
    }
    failures
}

/// Every `[dependencies]`-shaped table in a manifest, the workspace's and
/// the target-gated ones included, flattened to `(name, spec)`.
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

/// Write every manifest entry, rendered with `values.yaml`, under `dest`:
/// the project a consumer build starts from.
///
/// # Errors
///
/// Returns `Err` when the manifest, the values, or a source cannot be read,
/// when an entry's path is absolute or climbs out of its root, or when a
/// target cannot be written.
pub fn scaffold(root: &Path, dest: &Path) -> Result<(), String> {
    let manifest: Manifest = parse_yaml(&root.join(MANIFEST))?;
    let values: BTreeMap<String, String> = parse_yaml(&root.join(SUBTREE).join("values.yaml"))?;

    for entry in &manifest.assemblies.core.files {
        if is_unsafe(&entry.source) || is_unsafe(&entry.target) {
            return Err(format!("manifest: unsafe path `{}` -> `{}`", entry.source, entry.target));
        }
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
    let mut files = Vec::new();
    walk(&core_dir, &mut files)?;
    for file in files {
        let relative = file.strip_prefix(&core_dir).map_err(|err| err.to_string())?;
        let relative = subtree.join("core").join(relative);
        if !listed.contains(&relative) {
            failures.push(format!("{}: not listed in the manifest (orphan)", relative.display()));
        }
    }
    Ok(())
}

// Every file beneath `dir`, hidden directories included, since a seed body
// sits at its target's own path and `.github/` is one.
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
