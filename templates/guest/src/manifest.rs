//! Holds a tree's root manifest to the template outside its two stem lists.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::entries::ROOT_MANIFEST;
use crate::hold::{Drift, Drifted, Error};

// The template line each list follows: the path list runs from `common`'s
// line to the blank line after it, the guest's `[dependencies]` list from
// `tracing`'s line to the end of the file. An entry is kept by its shape
// wherever it stands, and put back after its anchor.
const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
const CRATE_LIST_ANCHOR: &str = "tracing.workspace = true";

/// The root `Cargo.toml` of one tree, held to the template around its two stem lists.
///
/// The manifest is the one listed file whose fixed text matters beyond the
/// list: the pins under `[workspace.dependencies]`, the lints, the `host`
/// feature, and the target tables. A line that is neither the template's
/// nor a stem's entry in one of the two lists is drift; a stem's entry,
/// `<stem> = { path = "crates/<dir>" }` in the path list or
/// `<stem>.workspace = true` in the crate list, is kept wherever it stands
/// and put back after its list's anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    path: PathBuf,
}

impl Manifest {
    /// The manifest's path, relative to the tree's root.
    pub const PATH: &'static str = "Cargo.toml";

    /// Returns the manifest of the tree at `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            path: root.join(Self::PATH),
        }
    }

    /// Returns how the manifest differs from the template outside its two lists.
    ///
    /// A manifest the tree lacks differs in every line.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the manifest cannot be read.
    pub fn drift(&self) -> Result<Drift, Error> {
        Ok(self.split()?.drift)
    }

    /// Puts every line outside the two lists back as the template spells it.
    ///
    /// Each list's entries are kept after its anchor, in the order the tree
    /// held them, and what [`drift`](Self::drift) would have found is
    /// returned. A manifest that stands is not written.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Io`] when the manifest cannot be read or written.
    pub fn restore(&self) -> Result<Drift, Error> {
        let split = self.split()?;
        if !split.drift.is_empty() {
            fs::write(&self.path, split.render()).map_err(|source| Error::Io {
                path: self.path.clone(),
                source,
            })?;
        }
        Ok(split.drift)
    }

    fn split(&self) -> Result<Split, Error> {
        match fs::read_to_string(&self.path) {
            Ok(held) => Ok(Split::of(&held)),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Split::of("")),
            Err(source) => Err(Error::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum List {
    Paths,
    Crates,
}

impl List {
    fn after(line: &str) -> Option<Self> {
        match line {
            PATH_LIST_ANCHOR => Some(Self::Paths),
            CRATE_LIST_ANCHOR => Some(Self::Crates),
            _ => None,
        }
    }

    // `<stem> = { path = "crates/<dir>" }` is the path list's entry,
    // `<stem>.workspace = true` the crate list's.
    fn entry(line: &str) -> Option<Self> {
        if let Some(stem) = line.strip_suffix(".workspace = true") {
            return is_stem(stem).then_some(Self::Crates);
        }
        let (stem, rest) = line.split_once(" = { path = \"crates/")?;
        let dir = rest.strip_suffix("\" }")?;
        (is_stem(stem) && is_stem(dir)).then_some(Self::Paths)
    }
}

#[derive(Default)]
struct Split {
    paths: Vec<String>,
    crates: Vec<String>,
    drift: Drift,
}

impl Split {
    // A held line shaped as an entry, and not a line of the template, is
    // its list's, once; the rest, a second copy of an entry included, is
    // diffed against the template.
    fn of(held: &str) -> Self {
        let mut split = Self::default();
        let mut rest = Vec::new();
        for line in held.lines() {
            let entry =
                if ROOT_MANIFEST.lines().any(|own| own == line) { None } else { List::entry(line) };
            let list = match entry {
                Some(List::Paths) => &mut split.paths,
                Some(List::Crates) => &mut split.crates,
                None => {
                    rest.push(line);
                    continue;
                }
            };
            if list.iter().any(|held| held == line) {
                rest.push(line);
            } else {
                list.push(line.to_owned());
            }
        }
        let template: Vec<&str> = ROOT_MANIFEST.lines().collect();
        if let Some(drifted) = Drifted::lines(Manifest::PATH, &template, &rest) {
            split.drift.push(drifted);
        }
        split
    }

    fn render(&self) -> String {
        let mut text = String::new();
        for line in ROOT_MANIFEST.lines() {
            text.push_str(line);
            text.push('\n');
            let entries = match List::after(line) {
                Some(List::Paths) => &self.paths,
                Some(List::Crates) => &self.crates,
                None => continue,
            };
            for entry in entries {
                text.push_str(entry);
                text.push('\n');
            }
        }
        text
    }
}

fn is_stem(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
