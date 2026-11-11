//! Scaffolding the template into a tree and holding the tree's static files to it.

mod support;

use std::fs;

use omnia_template::{ENTRIES, Error, Proof, Role, drift, restore, scaffold};

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
    assert!(
        ENTRIES
            .iter()
            .filter(|entry| entry.proof == Proof::Exact)
            .all(|entry| entry.role == Role::Static),
        "an exact entry is static"
    );
}

// `src/lib.rs` is listed, so of the two held files the hold reads the
// `Makefile` alone.
#[test]
fn scaffolded_once() {
    let tree = Scratch::new();
    tree.write("Makefile", "# held\n");
    tree.write("src/lib.rs", "// held\n");
    let held = ["Makefile", "src/lib.rs"];

    let scaffolded = scaffold(tree.root()).expect("the scaffold");

    let lacked: Vec<_> = ENTRIES.iter().filter(|entry| !held.contains(&entry.path)).collect();
    let paths: Vec<&str> = lacked.iter().map(|entry| entry.path).collect();
    assert_eq!(scaffolded.len(), ENTRIES.len() - 2, "a held file is not scaffolded");
    assert_eq!(scaffolded, paths, "every lacked file is scaffolded, in the table's order");
    for entry in lacked {
        assert_eq!(
            tree.read(entry.path).as_deref(),
            Some(entry.body),
            "`{}` is scaffolded",
            entry.path
        );
    }
    assert_eq!(tree.read("Makefile").as_deref(), Some("# held\n"), "a held file stands");
    assert_eq!(tree.read("src/lib.rs").as_deref(), Some("// held\n"));
    assert!(
        scaffold(tree.root()).expect("the second scaffold").is_empty(),
        "a scaffolded tree is scaffolded again by nothing"
    );

    let found = drift(tree.root()).expect("the drift");
    let [drifted] = found.files() else { panic!("the held `Makefile` alone drifts: {found}") };
    assert_eq!(drifted.path, "Makefile");
}

#[test]
fn edited_tree() {
    let tree = Scratch::new();
    scaffold(tree.root()).expect("the scaffold");
    fs::remove_file(tree.root().join("Makefile")).expect("the file removed");
    tree.write("mise.toml", &"# mine\n".repeat(30));
    tree.write("Cargo.toml", "// a list, left to its consumer\n");

    let found = drift(tree.root()).expect("the drift");

    let paths: Vec<&str> = found.files().iter().map(|drifted| drifted.path).collect();
    assert_eq!(
        paths,
        ["Makefile", "mise.toml"],
        "the static files that differ, a missing one among them"
    );
    let [makefile, mise] = found.files() else { unreachable!() };
    assert!(
        makefile.lines.iter().all(|line| line.starts_with("- ")),
        "a missing file lacks every line"
    );
    assert_eq!(makefile.lines.len(), body("Makefile").lines().count());
    assert!(
        mise.lines.iter().any(|line| line.starts_with("+ ")),
        "an edited file names the tree's line: {:?}",
        mise.lines
    );
    let shown = found.to_string();
    assert!(shown.starts_with("`Makefile`:\n- "), "the display heads each file: {shown}");
    assert!(shown.contains("\n… and "), "the display cuts a long file short: {shown}");
    assert!(shown.contains("\n`mise.toml`:\n"), "the display heads the second file: {shown}");

    let restored = restore(tree.root()).expect("the restore");

    assert_eq!(restored, found, "what was restored is what had drifted");
    assert_eq!(
        tree.read("Makefile").as_deref(),
        Some(body("Makefile")),
        "the missing file is written"
    );
    assert_eq!(
        tree.read("Cargo.toml").as_deref(),
        Some("// a list, left to its consumer\n"),
        "a listed file is not held"
    );
    assert!(drift(tree.root()).expect("the drift after").is_empty(), "the tree stands again");
}

#[test]
fn ending_differs() {
    let tree = Scratch::new();
    scaffold(tree.root()).expect("the scaffold");
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

    let err = scaffold(tree.root()).expect_err("the scaffold is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("Makefile")), "{err}");
    assert!(tree.read(".gitignore").is_none(), "nothing is scaffolded when a path is refused");
}

#[cfg(unix)]
#[test]
fn occupied_by_symlink() {
    let tree = Scratch::new();
    scaffold(tree.root()).expect("the scaffold");
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

    let err = scaffold(tree.root()).expect_err("the scaffold is refused");

    assert!(matches!(&err, Error::Occupied { path } if path.ends_with("src")), "{err}");
    assert!(
        fs::read_dir(tree.root().join("elsewhere")).expect("the directory").next().is_none(),
        "nothing is written through the link"
    );
    assert!(tree.read("Makefile").is_none(), "nothing is scaffolded when a path is refused");
}

// `Makefile` sorts before `crates/`, so a restore that held each file only as
// it reached it would put the Makefile back before refusing the link.
#[cfg(unix)]
#[test]
fn restore_symlink_parent() {
    let tree = Scratch::new();
    scaffold(tree.root()).expect("the scaffold");
    tree.write("Makefile", "# edited\n");
    let common_src = tree.root().join("crates/common/src");
    fs::create_dir_all(tree.root().join("elsewhere")).expect("a directory");
    fs::rename(&common_src, tree.root().join("elsewhere/common-src")).expect("the directory moved");
    std::os::unix::fs::symlink(tree.root().join("elsewhere/common-src"), &common_src)
        .expect("a link where the directory was");

    let err = restore(tree.root()).expect_err("the restore is refused");

    assert!(
        matches!(&err, Error::Occupied { path } if path.ends_with("crates/common/src")),
        "{err}"
    );
    assert_eq!(tree.read("Makefile").as_deref(), Some("# edited\n"), "nothing is put back");
}
