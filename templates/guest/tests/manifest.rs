//! Holding a tree's root manifest to the template around its two stem lists.

mod support;

use omnia_template::{Error, Manifest};

use self::support::Scratch;

const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }\n";
const PATH_LINE: &str = "orders = { path = \"crates/orders\" }\n";
const CRATE_LINE: &str = "orders.workspace = true\n";
const PIN: &str = "omnia-sdk = \"0.36.0\"";
const PIN_EDITED: &str = "omnia-sdk = \"0.37.0\"";

fn template() -> &'static str {
    omnia_template::entry(Manifest::PATH).expect("the template holds the manifest").body
}

// The template's manifest with the `orders` stem's line in each list.
fn with_orders() -> String {
    template().replace(PATH_LIST_ANCHOR, &format!("{PATH_LIST_ANCHOR}{PATH_LINE}")) + CRATE_LINE
}

#[test]
fn standing_with_entries() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &with_orders());
    let manifest = Manifest::new(tree.root());

    assert!(manifest.drift().expect("the drift").is_empty(), "a stem's lines are no drift");
    assert!(manifest.restore().expect("the restore").is_empty());
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the manifest stands");
}

#[test]
fn pin_restored_with_entries_kept() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &with_orders().replace(PIN, PIN_EDITED));
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.path, Manifest::PATH);
    assert_eq!(drifted.lines, [format!("- {PIN}"), format!("+ {PIN_EDITED}")]);
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the pin is the template's again");
    assert!(manifest.drift().expect("the drift after").is_empty());
}

// An entry is known by its shape wherever it stands: out of place it is no
// drift and the file is left, and a restore the rest of the file asks for
// puts it after its anchor.
#[test]
fn entries_anywhere_kept() {
    let tree = Scratch::new();
    let misplaced = format!("{PATH_LINE}{CRATE_LINE}{}", template());
    tree.write(Manifest::PATH, &misplaced);
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");
    assert!(drift.is_empty(), "an entry out of place is no drift: {drift}");
    assert_eq!(tree.read(Manifest::PATH), Some(misplaced.clone()), "a standing manifest is left");

    tree.write(Manifest::PATH, &misplaced.replace(PIN, PIN_EDITED));
    let drift = manifest.restore().expect("the restore");
    assert_eq!(drift.files().len(), 1, "the pin alone drifts: {drift}");
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "each entry follows its anchor");
}

#[test]
fn duplicate_entry_is_drift() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &(with_orders() + CRATE_LINE));
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.lines, [format!("+ {}", CRATE_LINE.trim_end())]);
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the entry is kept once");
}

// `authors.workspace = true` has a crate line's shape and is a `[package]`
// key, so it is drift rather than an entry put back under `[dependencies]`.
#[test]
fn package_key_inherited() {
    let tree = Scratch::new();
    let inherited = with_orders().replace(
        "\nversion.workspace = true\n",
        "\nversion.workspace = true\nauthors.workspace = true\n",
    );
    assert_ne!(inherited, with_orders(), "the `[package]` table inherits `version`");
    tree.write(Manifest::PATH, &inherited);
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.lines, ["+ authors.workspace = true"]);
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the key is not kept as a stem's");
}

// A crate line whose stem the path list does not name would be left
// without its pin by a restore, so it is drift and the restore removes it.
#[test]
fn crate_line_without_stem() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &(template().to_owned() + "serde.workspace = true\n"));
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.lines, ["+ serde.workspace = true"]);
    assert_eq!(tree.read(Manifest::PATH).as_deref(), Some(template()));
}

#[test]
fn missing_restored() {
    let tree = Scratch::new();
    let manifest = Manifest::new(tree.root());

    let drift = manifest.drift().expect("the drift");
    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert!(drifted.lines.iter().all(|line| line.starts_with("- ")), "every line is lacked");

    let restored = manifest.restore().expect("the restore");
    assert_eq!(restored, drift);
    assert_eq!(
        tree.read(Manifest::PATH).as_deref(),
        Some(template()),
        "the template's manifest is written"
    );
}

#[cfg(unix)]
#[test]
fn occupied_by_symlink() {
    let tree = Scratch::new();
    tree.write("elsewhere.toml", "[package]\nname = \"other\"\n");
    std::os::unix::fs::symlink(
        tree.root().join("elsewhere.toml"),
        tree.root().join(Manifest::PATH),
    )
    .expect("a link at the manifest's path");
    let manifest = Manifest::new(tree.root());

    let err = manifest.drift().expect_err("the hold is refused");
    assert!(matches!(&err, Error::Occupied { path } if path.ends_with(Manifest::PATH)), "{err}");

    let err = manifest.restore().expect_err("the restore is refused");
    assert!(matches!(&err, Error::Occupied { path } if path.ends_with(Manifest::PATH)), "{err}");
    assert_eq!(
        tree.read("elsewhere.toml").as_deref(),
        Some("[package]\nname = \"other\"\n"),
        "the link's target stands"
    );
}

#[test]
fn whole_file_replaced() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, "[package]\nname = \"other\"\n");
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert!(drifted.lines.contains(&"+ name = \"other\"".to_owned()), "{drift}");
    assert_eq!(tree.read(Manifest::PATH).as_deref(), Some(template()));
}
