//! Scaffolds the template into a tree and holds the tree's static files to it.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

use crate::entries::{ENTRIES, Entry, Role};
use crate::hold::{Drift, Drifted, Error};

/// Scaffolds every file of the template the tree at `root` lacks and returns their paths.
///
/// A file the tree holds is left as it stands, whatever it holds, so a tree
/// scaffolded once is scaffolded again by nothing. A file whose write fails
/// is removed, so the next scaffold writes it again. The directories above a
/// file are made as needed. Every path is held before any is written: a
/// directory or a symbolic link standing where a file goes, or a file or a
/// symbolic link on the way to one, refuses the whole scaffold.
///
/// # Errors
///
/// - [`Error::Occupied`] when a directory or a symbolic link stands at a
///   file's path, or on the way to one.
/// - [`Error::Io`] when a path cannot be read or written.
pub fn scaffold(root: &Path) -> Result<Vec<&'static str>, Error> {
    let missing: Vec<&Entry> = ENTRIES
        .iter()
        .map(|entry| Ok(read(root, entry)?.is_none().then_some(entry)))
        .collect::<Result<Vec<_>, Error>>()?
        .into_iter()
        .flatten()
        .collect();

    let mut scaffolded = Vec::with_capacity(missing.len());
    for entry in missing {
        if create(root, entry)? {
            scaffolded.push(entry.path);
        }
    }
    Ok(scaffolded)
}

/// Returns how the tree at `root` differs from the template in its static files.
///
/// Every entry of [`Role::Static`] is held byte for byte, a file the tree
/// lacks among them; a [`Role::Listed`] entry is not read.
///
/// # Errors
///
/// - [`Error::Occupied`] when a directory or a symbolic link stands at a
///   static file's path.
/// - [`Error::Io`] when a static file cannot be read.
pub fn drift(root: &Path) -> Result<Drift, Error> {
    let mut drift = Drift::default();
    for drifted in drifted(root)?.into_iter().filter_map(|(_, drifted)| drifted) {
        drift.push(drifted);
    }
    Ok(drift)
}

/// Puts every static file of the tree at `root` back as the template spells it.
///
/// What [`drift`] would have found is written over and returned, a file
/// the tree lacks written among them. Every file is read before any is
/// written, so a path the hold refuses leaves the tree as it was, and each
/// file is put back whole or left as it stood.
///
/// # Errors
///
/// - [`Error::Occupied`] when a directory or a symbolic link stands at a
///   static file's path, or on the way to one.
/// - [`Error::Io`] when a static file cannot be read or written.
pub fn restore(root: &Path) -> Result<Drift, Error> {
    let mut drift = Drift::default();
    for (entry, drifted) in drifted(root)? {
        if let Some(drifted) = drifted {
            write(root, entry)?;
            drift.push(drifted);
        }
    }
    Ok(drift)
}

// Every static entry beside how the tree differs from it, every file read
// before the caller writes any.
fn drifted(root: &Path) -> Result<Vec<(&'static Entry, Option<Drifted>)>, Error> {
    ENTRIES
        .iter()
        .filter(|entry| entry.role == Role::Static)
        .map(|entry| {
            let held = read(root, entry)?;
            Ok((entry, Drifted::of(entry.path, entry.body, held.as_deref())))
        })
        .collect()
}

// The file at the entry's path as the tree holds it, nothing where the tree
// lacks it, and a refusal where no file could be written, the directories
// above it held first.
fn read(root: &Path, entry: &Entry) -> Result<Option<Vec<u8>>, Error> {
    for dir in above(root, entry) {
        if !standing(&dir)? {
            return Ok(None);
        }
    }
    let path = root.join(entry.path);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {
            fs::read(&path).map(Some).map_err(|source| Error::Io { path, source })
        }
        Ok(_) => Err(Error::Occupied { path }),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io { path, source }),
    }
}

// Creates the entry's file where the tree holds none, and answers whether it did:
// a file that appeared since the tree was read is kept.
fn create(root: &Path, entry: &Entry) -> Result<bool, Error> {
    let path = parents(root, entry)?;
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::AlreadyExists => return Ok(false),
        Err(source) => return Err(Error::Io { path, source }),
    };
    if let Err(source) = file.write_all(entry.body.as_bytes()) {
        // a body cut short would read as held by the next scaffold, so the file
        // goes with the error that cut it; the write's error is the cause
        // to report whether or not the removal succeeds
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(Error::Io { path, source });
    }
    Ok(true)
}

fn write(root: &Path, entry: &Entry) -> Result<(), Error> {
    let path = parents(root, entry)?;
    replace(&path, entry.body.as_bytes())
}

// Puts the body at the path whole or not at all, over whatever stands
// there: it is written beside the path under a name of the crate's own
// and moved over it, so a write that fails leaves the file as it was, and
// a link planted at the path since it was read is replaced rather than
// written through. The name carries no process id, since a `wasm32` guest
// has none, so a staged file a write that stopped short left behind is
// removed before the next write stages there: one process holds a tree at
// a time.
pub fn replace(path: &Path, body: &[u8]) -> Result<(), Error> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let staged = path.with_file_name(format!(".{name}.omnia-template~"));
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| Error::Io { path, source }
    };
    let _ = fs::remove_file(&staged);
    let moved = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staged)
        .and_then(|mut file| file.write_all(body))
        .map_err(io(&staged))
        .and_then(|()| fs::rename(&staged, path).map_err(io(path)));
    if moved.is_err() {
        let _ = fs::remove_file(&staged);
    }
    moved
}

// Makes the directories above the entry that are missing and answers the
// entry's path.
fn parents(root: &Path, entry: &Entry) -> Result<PathBuf, Error> {
    for dir in above(root, entry) {
        if !standing(&dir)? {
            fs::create_dir(&dir).map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
        }
    }
    Ok(root.join(entry.path))
}

// Each directory above the entry, the nearest the root first.
fn above(root: &Path, entry: &Entry) -> Vec<PathBuf> {
    let mut dir = root.to_path_buf();
    Path::new(entry.path)
        .parent()
        .into_iter()
        .flat_map(Path::components)
        .map(|component| {
            dir.push(component);
            dir.clone()
        })
        .collect()
}

// Whether a directory above an entry stands: a file or a symbolic link in
// its place is refused rather than followed.
fn standing(dir: &Path) -> Result<bool, Error> {
    match fs::symlink_metadata(dir) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err(Error::Occupied {
            path: dir.to_path_buf(),
        }),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            path: dir.to_path_buf(),
            source,
        }),
    }
}
