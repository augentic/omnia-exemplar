//! Holding a tree's root manifest to the template around its stem list, and
//! inserting a stem's line into a list after its anchor.

mod support;

use omnia_template::{
    CRATE_LIST_ANCHOR, Error, HANDLER_LIST_ANCHOR, Manifest, REQUIRES_LIST_ANCHOR, insert_after,
};

use self::support::Scratch;

const CRATE_LINE: &str = "orders = { path = \"crates/orders\" }\n";
const PIN: &str = "omnia-sdk = \"0.38.0\"";
const PIN_EDITED: &str = "omnia-sdk = \"0.39.0\"";

fn template() -> &'static str {
    omnia_template::entry(Manifest::PATH).expect("the template holds the manifest").body
}

// The template's manifest with the `orders` stem's line in the list.
fn with_orders() -> String {
    insert_after(template(), CRATE_LIST_ANCHOR, CRATE_LINE).expect("crate anchor")
}

// An entry is known by its shape wherever it stands: out of place it is no
// drift and the file is left, and a restore the rest of the file asks for
// puts it after its anchor.
#[test]
fn misplaced_entry() {
    let tree = Scratch::new();
    let misplaced = format!("{CRATE_LINE}{}", template());
    tree.write(Manifest::PATH, &misplaced);
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");
    assert!(drift.is_empty(), "an entry out of place is no drift: {drift}");
    assert_eq!(tree.read(Manifest::PATH), Some(misplaced.clone()), "a standing manifest is left");

    tree.write(Manifest::PATH, &misplaced.replace(PIN, PIN_EDITED));
    let drift = manifest.restore().expect("the restore");
    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.path, Manifest::PATH);
    assert_eq!(drifted.lines, [format!("- {PIN}"), format!("+ {PIN_EDITED}")]);
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the entry follows its anchor");
    assert!(manifest.drift().expect("the drift after").is_empty());
}

#[test]
fn duplicate_entry() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &(with_orders() + CRATE_LINE));
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.lines, [format!("+ {}", CRATE_LINE.trim_end())]);
    assert_eq!(tree.read(Manifest::PATH), Some(with_orders()), "the entry is kept once");
}

// A dependency line that is not a stem under `crates/` is drift and the
// restore removes it.
#[test]
fn non_stem_dependency() {
    let tree = Scratch::new();
    tree.write(Manifest::PATH, &(template().to_owned() + "serde.workspace = true\n"));
    let manifest = Manifest::new(tree.root());

    let drift = manifest.restore().expect("the restore");

    let [drifted] = drift.files() else { panic!("the manifest alone drifts: {drift}") };
    assert_eq!(drifted.lines, ["+ serde.workspace = true"]);
    assert_eq!(tree.read(Manifest::PATH).as_deref(), Some(template()));
}

#[test]
fn missing() {
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

// The seed's array is headed by a comment, so an entry lands under it with
// no comma on the comment, the array stays one entry per line however few
// it holds, and a one-line array laid by hand is opened out the same way.
#[test]
fn handler_list_insertion() {
    const HEAD: &str = "    Handlers::from([\n        // one `<stem>::handlers(),` per stem, each \
                        added at the end\n";
    let seed = omnia_template::entry("src/lib.rs").expect("the template holds `src/lib.rs`").body;
    assert!(
        seed.contains(&format!("{HEAD}    ])\n    .with_state(Client::new(provider))")),
        "{seed}"
    );

    let one = insert_after(seed, HANDLER_LIST_ANCHOR, "        orders::handlers(),").expect("one");
    assert!(one.contains(&format!("{HEAD}        orders::handlers(),\n    ])")), "{one}");

    let two = insert_after(&one, HANDLER_LIST_ANCHOR, "billing::handlers()").expect("two");
    assert!(
        two.contains(&format!(
            "{HEAD}        orders::handlers(),\n        billing::handlers(),\n    ])"
        )),
        "{two}"
    );

    let single_line =
        "    Handlers::from([orders::handlers()]).with_state(Client::new(provider))\n";
    let after_fmt =
        insert_after(single_line, HANDLER_LIST_ANCHOR, "billing::handlers()").expect("formatted");
    assert_eq!(
        after_fmt,
        "    Handlers::from([\n        orders::handlers(),\n        billing::handlers(),\n    \
         ]).with_state(Client::new(provider))\n"
    );
}

// Each predicate lands on the line under the standing bound, so a later
// stem's precedes an earlier one's; a `where` list's order is immaterial.
#[test]
fn requires_list_insertion() {
    const HEAD: &str = "where\n    P: Send + Sync + 'static,\n";
    let seed = omnia_template::entry("src/lib.rs").expect("the template holds `src/lib.rs`").body;
    assert!(seed.contains(&format!("{HEAD}{{\n")), "{seed}");

    let one = insert_after(seed, REQUIRES_LIST_ANCHOR, "    P: orders::Requires,").expect("one");
    assert!(one.contains(&format!("{HEAD}    P: orders::Requires,\n{{\n")), "{one}");

    let two = insert_after(&one, REQUIRES_LIST_ANCHOR, "    P: billing::Requires,").expect("two");
    assert!(
        two.contains(&format!("{HEAD}    P: billing::Requires,\n    P: orders::Requires,\n{{\n")),
        "{two}"
    );
}
