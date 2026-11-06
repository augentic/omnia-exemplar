//! Holds a tree's root manifest to the template outside its stem list.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::entries::ROOT_MANIFEST;
use crate::hold::{Drift, Drifted, Error};
use crate::tree::replace;

/// The template line the root manifest's `[dependencies]` stem list follows.
pub const CRATE_LIST_ANCHOR: &str = "omnia-sdk.workspace = true";

/// Insert `line` after the first occurrence of `anchor` in `text`.
///
/// When `anchor` opens a bracketed list (`anchor` ends with `'['`), `line`
/// is appended to the list before its closing `']'` whether the list is
/// empty, single-line, or multi-line. Otherwise `line` is placed on a new
/// line immediately following `anchor`'s line. Returns `None` when `anchor`
/// (or its closing `']'`) does not appear in `text`.
#[must_use]
pub fn insert_after(text: &str, anchor: &str, line: &str) -> Option<String> {
    let (before, after) = text.split_once(anchor)?;
    let line = line.trim_end_matches('\n');
    if anchor.ends_with('[') {
        let (inner, rest) = after.split_once(']')?;
        let line_start = before.rfind('\n').map_or(0, |index| index + 1);
        let indent: String =
            before[line_start..].chars().take_while(|ch| ch.is_whitespace()).collect();
        let item_indent = format!("{indent}    ");
        let entry = format!("{item_indent}{},", line.trim().trim_end_matches(','));
        if inner.trim().is_empty() {
            Some(format!("{before}{anchor}\n{entry}\n{indent}]{rest}"))
        } else if !inner.contains('\n') {
            let mut items = String::new();
            for existing in inner.split(',').map(str::trim).filter(|item| !item.is_empty()) {
                items.push_str(&item_indent);
                items.push_str(existing);
                items.push_str(",\n");
            }
            Some(format!("{before}{anchor}\n{items}{entry}\n{indent}]{rest}"))
        } else {
            let trimmed = inner.trim_end();
            let comma = if trimmed.ends_with(',') { "" } else { "," };
            Some(format!("{before}{anchor}{trimmed}{comma}\n{entry}\n{indent}]{rest}"))
        }
    } else {
        let (tail, rest) = after.split_once('\n').unwrap_or((after, ""));
        if rest.is_empty() && !after.ends_with('\n') {
            Some(format!("{before}{anchor}{tail}\n{line}"))
        } else {
            Some(format!("{before}{anchor}{tail}\n{line}\n{rest}"))
        }
    }
}

/// The root `Cargo.toml` of one tree, held to the template around its stem list.
///
/// The manifest is the one listed file whose fixed text matters beyond the
/// list: the pins under `[workspace.dependencies]`, the lints, the `host`
/// feature, and the target tables. A line that is neither the template's
/// nor a stem's `<stem> = { path = "crates/<dir>" }` entry is drift; a
/// stem's entry is kept wherever it stands and put back after
/// [`CRATE_LIST_ANCHOR`].
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

    /// Returns how the manifest differs from the template outside its stem list.
    ///
    /// A manifest the tree lacks differs in every line.
    ///
    /// # Errors
    ///
    /// - [`Error::Occupied`] when a directory or a symbolic link stands at
    ///   the manifest's path.
    /// - [`Error::Io`] when the manifest cannot be read.
    pub fn drift(&self) -> Result<Drift, Error> {
        Ok(self.split()?.drift)
    }

    /// Puts every line outside the stem list back as the template spells it.
    ///
    /// The list's entries are kept after [`CRATE_LIST_ANCHOR`], in the
    /// order the tree held them, and what [`drift`](Self::drift) would have
    /// found is returned. A manifest that stands is not written; one that
    /// differs is written whole or left as it stood.
    ///
    /// # Errors
    ///
    /// - [`Error::Occupied`] when a directory or a symbolic link stands at
    ///   the manifest's path.
    /// - [`Error::Io`] when the manifest cannot be read or written.
    pub fn restore(&self) -> Result<Drift, Error> {
        let split = self.split()?;
        if !split.drift.is_empty() {
            replace(&self.path, split.render().as_bytes())?;
        }
        Ok(split.drift)
    }

    fn split(&self) -> Result<Split, Error> {
        let io = |source| Error::Io {
            path: self.path.clone(),
            source,
        };
        match fs::symlink_metadata(&self.path) {
            Ok(meta) if meta.is_file() => {
                fs::read_to_string(&self.path).map(|held| Split::of(&held)).map_err(io)
            }
            Ok(_) => Err(Error::Occupied {
                path: self.path.clone(),
            }),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Split::of("")),
            Err(source) => Err(io(source)),
        }
    }
}

#[derive(Default)]
struct Split {
    crates: Vec<String>,
    drift: Drift,
}

impl Split {
    // A held line shaped as `<stem> = { path = "crates/<dir>" }`, and not a
    // line of the template, is the stem list's, once; the rest, a second
    // copy of an entry included, is diffed against the template.
    fn of(held: &str) -> Self {
        let own = |line: &str| ROOT_MANIFEST.lines().any(|own| own == line);
        let mut split = Self::default();
        let mut rest = Vec::new();
        for line in held.lines() {
            if !own(line) && is_entry(line) && !split.crates.iter().any(|held| held == line) {
                split.crates.push(line.to_owned());
            } else {
                rest.push(line);
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
            if line == CRATE_LIST_ANCHOR {
                for entry in &self.crates {
                    text.push_str(entry);
                    text.push('\n');
                }
            }
        }
        text
    }
}

fn is_entry(line: &str) -> bool {
    let Some((stem, rest)) = line.split_once(" = { path = \"crates/") else {
        return false;
    };
    rest.strip_suffix("\" }").is_some_and(|dir| is_stem(stem) && is_stem(dir))
}

fn is_stem(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
