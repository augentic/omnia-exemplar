//! The template contract gate, run as part of the standard test suite so
//! `cargo make ci` proves the templates without any exemplar-only task in
//! the (itself templated) root `Makefile.toml`.

#[test]
fn contract() {
    let root = template_check::repo_root();
    let failures = template_check::run(&root).expect("contract inputs readable");
    assert!(failures.is_empty(), "template contract violated:\n{}", failures.join("\n"));
}
