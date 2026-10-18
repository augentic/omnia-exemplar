//! What holding a tree to the template finds: the lines that drifted, and what stopped the hold.

use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::{error, io};

/// The files of a tree that differ from the template, each with the lines that differ.
///
/// Empty when the tree stands as the template spells it. [`Display`] lists
/// each file under its path, `- ` before a line of the template the tree
/// lacks and `+ ` before a line of the tree the template lacks, the longest
/// files cut short with a count.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Drift(Vec<Drifted>);

impl Drift {
    // The most lines of one file `Display` spells out before counting the rest.
    const LISTED: usize = 24;

    /// Returns whether no file differs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns each file that differs, in the template's order.
    #[must_use]
    pub fn files(&self) -> &[Drifted] {
        &self.0
    }

    pub(crate) fn push(&mut self, drifted: Drifted) {
        self.0.push(drifted);
    }
}

impl Display for Drift {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for (i, drifted) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            write!(f, "`{}`:", drifted.path)?;
            let listed = drifted.lines.len().min(Self::LISTED);
            for line in &drifted.lines[..listed] {
                write!(f, "\n{line}")?;
            }
            match drifted.lines.len() - listed {
                0 => {}
                more => write!(f, "\n… and {more} more")?,
            }
        }
        Ok(())
    }
}

/// One file of a [`Drift`]: its path and the lines that differ.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drifted {
    /// The `/`-separated path of the file, relative to the tree's root.
    pub path: &'static str,
    /// The lines that differ, `- ` before the template's and `+ ` before the tree's.
    pub lines: Vec<String>,
}

impl Drifted {
    // Past this many lines a tree's file is reported whole rather than
    // diffed, since the diff's table grows with the product of the two
    // lengths.
    const DIFFED: usize = 2_000;

    // The file as the tree holds it, or nothing where the tree lacks it,
    // against the template's body: `None` when they agree byte for byte.
    pub(crate) fn of(path: &'static str, template: &str, held: Option<&[u8]>) -> Option<Self> {
        if held == Some(template.as_bytes()) {
            return None;
        }
        let held = String::from_utf8_lossy(held.unwrap_or_default());
        let template: Vec<&str> = template.lines().collect();
        let held: Vec<&str> = held.lines().collect();
        let mut lines = Self::diffed(&template, &held);
        if lines.is_empty() {
            lines.push("~ the same lines, ended differently".to_owned());
        }
        Some(Self { path, lines })
    }

    // The template's lines against the tree's, both already cut to lines:
    // `None` when they agree.
    pub(crate) fn lines(path: &'static str, template: &[&str], held: &[&str]) -> Option<Self> {
        let lines = Self::diffed(template, held);
        (!lines.is_empty()).then_some(Self { path, lines })
    }

    fn diffed(template: &[&str], held: &[&str]) -> Vec<String> {
        if held.len() > Self::DIFFED {
            return vec![format!("+ {} lines in place of the template's", held.len())];
        }
        diff(template, held)
    }
}

/// Why a tree could not be scaffolded, read, or written.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A directory or a symbolic link stands where a file of the template
    /// goes, so nothing is written over it.
    Occupied {
        /// The path beneath the tree's root where the file goes.
        path: PathBuf,
    },
    /// Reading or writing a path beneath the tree's root failed.
    Io {
        /// The path read or written.
        path: PathBuf,
        /// What the filesystem answered.
        source: io::Error,
    },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Occupied { path } => {
                write!(f, "a directory or symbolic link stands at `{}`", path.display())
            }
            Self::Io { path, source } => write!(f, "`{}`: {source}", path.display()),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Occupied { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

// The lines of `template` and `held` outside their longest common
// subsequence, in order, `- ` before the template's and `+ ` before the
// tree's.
fn diff(template: &[&str], held: &[&str]) -> Vec<String> {
    let (n, m) = (template.len(), held.len());
    let mut common = vec![vec![0_usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            common[i][j] = if template[i] == held[j] {
                common[i + 1][j + 1] + 1
            } else {
                common[i + 1][j].max(common[i][j + 1])
            };
        }
    }

    let mut drift = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if template[i] == held[j] {
            i += 1;
            j += 1;
        } else if common[i + 1][j] >= common[i][j + 1] {
            drift.push(format!("- {}", template[i]));
            i += 1;
        } else {
            drift.push(format!("+ {}", held[j]));
            j += 1;
        }
    }
    drift.extend(template[i..].iter().map(|line| format!("- {line}")));
    drift.extend(held[j..].iter().map(|line| format!("+ {line}")));
    drift
}
