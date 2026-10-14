//! Laying the template into a tree and holding the tree's static files to it.

mod support;

use std::fs;

use omnia_template::{ENTRIES, Error, Proof, Role, drift, lay, restore};

use self::support::Scratch;

fn body(path: &str) -> &'static str {
    omnia_template::entry(path).unwrap_or_else(|| panic!("the template holds `{path}`")).body
}

#[test]
fn table() {
    let paths: Vec<&str> = ENTRIES.iter().map(|entry| entry.path).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(paths, sorted, "the table is sorted by path, each path once");
    assert_eq!(paths.len(), 31, "the layout is thirty-one files");

    for entry in ENTRIES {
        assert!(!entry.path.starts_with('/'), "`{}` is relative", entry.path);
        assert!(
            !entry.path.split('/').any(|segment| segment == ".."),
            "`{}` stays beneath",
            entry.path
        );
        assert!(!entry.body.is_empty(), "`{}` has a body", entry.path);
    }
    let exact = ENTRIES.iter().filter(|entry| entry.proof == Proof::Exact).count();
    let listed = ENTRIES.iter().filter(|entry| entry.role == Role::Listed).count();
    assert_eq!(exact, 13, "the exact entries are the repository's tooling");
    assert_eq!(listed, 9, "the listed entries are the nine lists a slice appends to");
    assert!(
        ENTRIES
            .iter()
            .filter(|entry| entry.proof == Proof::Exact)
            .all(|entry| entry.role == Role::Static),
        "an exact entry is static"
    );
}

#[test]
fn laid_once() {
    let tree = Scratch::new();

    let laid = lay(tree.root()).expect("the lay");

    let paths: Vec<&str> = ENTRIES.iter().map(|entry| entry.path).collect();
    assert_eq!(laid, paths, "every file is laid, in the table's order");
    for entry in ENTRIES {
        assert_eq!(tree.read(entry.path).as_deref(), Some(entry.body), "`{}` is laid", entry.path);
    }
    assert!(
        lay(tree.root()).expect("the second lay").is_empty(),
        "a laid tree is laid again by nothing"
    );
    assert!(drift(tree.root()).expect("the drift").is_empty(), "a laid tree stands");
}

#[test]
fn held_file_kept() {
    let tree = Scratch::new();
    tree.write("Makefile", "# held\n");
    tree.write("src/routes.rs", "// held\n");

    let laid = lay(tree.root()).expect("the lay");

    assert!(
        !laid.contains(&"Makefile") && !laid.contains(&"src/routes.rs"),
        "a held file is not laid"
    );
    assert_eq!(laid.len(), ENTRIES.len() - 2);
    assert_eq!(tree.read("Makefile").as_deref(), Some("# held\n"), "a held file stands");
    assert_eq!(tree.read("src/routes.rs").as_deref(), Some("// held\n"));
}

#[test]
fn static_files_restored() {
    let tree = Scratch::new();
    lay(tree.root()).expect("the lay");
    tree.write("src/lib.rs", "//! mine\n");
    fs::remove_file(tree.root().join("Makefile")).expect("the file removed");
    tree.write("src/routes.rs", "// a list, left to its consumer\n");

    let found = drift(tree.root()).expect("the drift");

    let paths: Vec<&str> = found.files().iter().map(|drifted| drifted.path).collect();
    assert_eq!(
        paths,
        ["Makefile", "src/lib.rs"],
        "the static files that differ, a missing one among them"
    );
    let [makefile, lib] = found.files() else { unreachable!() };
    assert!(
        makefile.lines.iter().all(|line| line.starts_with("- ")),
        "a missing file lacks every line"
    );
    assert_eq!(makefile.lines.len(), body("Makefile").lines().count());
    assert!(
        lib.lines.contains(&"+ //! mine".to_owned()),
        "the tree's line is named: {:?}",
        lib.lines
    );
    assert!(
        lib.lines.contains(&"- //! # guest".to_owned()),
        "the template's line is named: {:?}",
        lib.lines
    );
    let shown = found.to_string();
    assert!(shown.starts_with("`Makefile`:\n- "), "the display heads each file: {shown}");
    assert!(shown.contains("\n… and "), "the display cuts a long file short: {shown}");
    assert!(shown.contains("\n`src/lib.rs`:\n"), "the display heads the second file: {shown}");

    let restored = restore(tree.root()).expect("the restore");

    assert_eq!(restored, found, "what was restored is what had drifted");
    assert_eq!(
        tree.read("Makefile").as_deref(),
        Some(body("Makefile")),
        "the missing file is laid"
    );
    assert_eq!(
        tree.read("src/lib.rs").as_deref(),
        Some(body("src/lib.rs")),
        "the changed file is put back"
    );
    assert_eq!(
        tree.read("src/routes.rs").as_deref(),
        Some("// a list, left to its consumer\n"),
        "a listed file is not held"
    );
    assert!(drift(tree.root()).expect("the drift after").is_empty(), "the tree stands again");
}

#[test]
fn ending_differs() {
    let tree = Scratch::new();
    lay(tree.root()).expect("the lay");
    tree.write("rustfmt.toml", body("rustfmt.toml").trim_end());

    let found = drift(tree.root()).expect("the drift");

    let [drifted] = found.files() else { panic!("one file differs: {found}") };
    assert_eq!(drifted.path, "rustfmt.toml");
    assert_eq!(drifted.lines, ["~ the same lines, ended differently"]);
}

#[test]
fn occupied_by_directory() {
    let tree = Scratch::new();
    fs::create_dir(tree.root().join("Makefile")).expect("a directory at a file's path");

    let err = lay(tree.root()).expect_err("the lay is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("Makefile")), "{err}");
    assert!(tree.read(".gitignore").is_none(), "nothing is laid when a path is refused");
}

#[cfg(unix)]
#[test]
fn occupied_by_symlink() {
    let tree = Scratch::new();
    lay(tree.root()).expect("the lay");
    fs::remove_file(tree.root().join("rustfmt.toml")).expect("the file removed");
    std::os::unix::fs::symlink(tree.root().join("taplo.toml"), tree.root().join("rustfmt.toml"))
        .expect("a link at a file's path");

    let err = drift(tree.root()).expect_err("the hold is refused");
    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("rustfmt.toml")), "{err}");

    let err = restore(tree.root()).expect_err("the restore is refused");
    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("rustfmt.toml")), "{err}");
    assert_eq!(
        tree.read("taplo.toml").as_deref(),
        Some(body("taplo.toml")),
        "the link's target stands"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_parent() {
    let tree = Scratch::new();
    fs::create_dir(tree.root().join("elsewhere")).expect("a directory");
    std::os::unix::fs::symlink(tree.root().join("elsewhere"), tree.root().join("src"))
        .expect("a link where a directory goes");

    let err = lay(tree.root()).expect_err("the lay is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("src")), "{err}");
    assert!(
        fs::read_dir(tree.root().join("elsewhere")).expect("the directory").next().is_none(),
        "nothing is written through the link"
    );
    assert!(tree.read("Makefile").is_none(), "nothing is laid when a path is refused");
}

#[test]
fn file_as_parent() {
    let tree = Scratch::new();
    tree.write("src", "");

    let err = lay(tree.root()).expect_err("the lay is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("src")), "{err}");
    assert!(tree.read("Makefile").is_none(), "nothing is laid when a path is refused");
}

// `Makefile` sorts before `src/`, so a restore that held each file only as
// it reached it would put the Makefile back before refusing the link.
#[cfg(unix)]
#[test]
fn restore_with_symlinked_parent() {
    let tree = Scratch::new();
    lay(tree.root()).expect("the lay");
    tree.write("Makefile", "# edited\n");
    fs::rename(tree.root().join("src"), tree.root().join("elsewhere"))
        .expect("the directory moved");
    std::os::unix::fs::symlink(tree.root().join("elsewhere"), tree.root().join("src"))
        .expect("a link where the directory was");

    let err = restore(tree.root()).expect_err("the restore is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("src")), "{err}");
    assert_eq!(tree.read("Makefile").as_deref(), Some("# edited\n"), "nothing is put back");
}
