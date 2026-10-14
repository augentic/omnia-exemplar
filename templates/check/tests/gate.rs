//! The template contract gate, run as part of the standard test suite so
//! `mise run ci` proves the templates without any exemplar-only task in
//! the (itself templated) root `mise.toml`, and the refusals it owes,
//! over a replica of the files it reads.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use omnia_template::ENTRIES;

const SEED_MANIFEST: &str = "templates/guest/core/Cargo.toml";

#[test]
fn contract() {
    let root = template_check::repo_root();
    let failures = template_check::run(&root).expect("contract inputs readable");
    assert!(failures.is_empty(), "template contract violated:\n{}", failures.join("\n"));
}

#[test]
fn retargeted_pin() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    let pin = seed
        .lines()
        .find(|line| line.starts_with("omnia-sdk = "))
        .expect("the seed pins `omnia-sdk`");
    replica.write(
        SEED_MANIFEST,
        &seed.replace(pin, "omnia-sdk = { git = \"https://github.com/augentic/omnia\" }"),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [pin, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(
        pin,
        "templates/guest/core/Cargo.toml: `omnia-sdk` sets `git`, which a seed dependency never \
         does"
    );
    assert_eq!(
        body,
        "omnia-template: `templates/guest/core/Cargo.toml` -> `Cargo.toml` embeds a body other \
         than its source"
    );
}

// The version reads as the root's pin while `package` fetches another crate
// under the familiar name.
#[test]
fn renamed_package() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    let pin = seed
        .lines()
        .find(|line| line.starts_with("omnia-sdk = "))
        .expect("the seed pins `omnia-sdk`");
    replica.write(
        SEED_MANIFEST,
        &seed.replace(pin, "omnia-sdk = { version = \"0.36.0\", package = \"omnia-test\" }"),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [pin, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(
        pin,
        "templates/guest/core/Cargo.toml: `omnia-sdk` sets `package`, which a seed dependency \
         never does"
    );
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

// A build dependency runs on the consumer's build host, and the root never
// pinned this one.
#[test]
fn unpinned_crate() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    replica.write(SEED_MANIFEST, &format!("{seed}\n[build-dependencies]\nrand = \"0.9.0\"\n"));

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [pin, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(pin, "templates/guest/core/Cargo.toml: `rand` is not a pin of the root workspace");
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

#[test]
fn patched_crate() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    replica.write(
        SEED_MANIFEST,
        &format!(
            "{seed}\n[patch.crates-io]\nomnia-sdk = {{ git = \"https://github.com/augentic/omnia\" }}\n"
        ),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [patch, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(
        patch,
        "templates/guest/core/Cargo.toml: `[patch]` retargets crates behind the pins"
    );
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

#[test]
fn unshared_source() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    let edited = seed.replacen(
        "[dependencies]\n",
        "[dependencies]\nelsewhere = { path = \"../elsewhere\" }\n",
        1,
    );
    assert_ne!(edited, seed, "the seed has a `[dependencies]` table");
    replica.write(SEED_MANIFEST, &edited);

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [source, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(source, "templates/guest/core/Cargo.toml: `elsewhere` is a path outside the tree");
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

// Cargo reads `.cargo/config.toml` behind every manifest, so a source
// replacement there retargets crates the manifests pin honestly.
#[test]
fn cargo_config_laid() {
    let replica = Replica::of_repo();
    let config = "templates/guest/core/.cargo/config.toml";
    replica.write(
        config,
        "[source.crates-io]\nreplace-with = \"mirror\"\n\n[source.mirror]\nregistry = \
         \"https://example.invalid/index\"\n",
    );
    let manifest = replica.read("templates/guest/manifest.yaml");
    replica.write(
        "templates/guest/manifest.yaml",
        &format!("{manifest}      - source: {config}\n        target: .cargo/config.toml\n        proof: seed\n"),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [config, row] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(config, "manifest: target `.cargo/config.toml` configures cargo behind the pins");
    assert_eq!(
        row,
        "manifest: `templates/guest/core/.cargo/config.toml` -> `.cargo/config.toml` (seed) has \
         no row in the crate's table"
    );
}

#[test]
fn edited_source() {
    let replica = Replica::of_repo();
    let makefile = replica.read("Makefile");
    replica.write("Makefile", &format!("{makefile}\n# a line the crate does not embed\n"));

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    assert_eq!(
        failures,
        ["omnia-template: `Makefile` -> `Makefile` embeds a body other than its source"]
    );
}

// The files the gate reads, copied beneath a scratch root a scenario edits:
// the manifest, its render values, the root manifest the pins are held to,
// and every entry's source.
struct Replica(PathBuf);

impl Replica {
    fn of_repo() -> Self {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let root = template_check::repo_root();
        let replica = Self(env::temp_dir().join(format!(
            "template-check-{}-{}",
            process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        )));
        let gated = ["Cargo.toml", "templates/guest/manifest.yaml", "templates/guest/values.yaml"];
        for source in ENTRIES.iter().map(|entry| entry.source).chain(gated) {
            let text = fs::read_to_string(root.join(source))
                .unwrap_or_else(|err| panic!("`{source}`: {err}"));
            replica.write(source, &text);
        }
        replica
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap_or_else(|err| panic!("`{path}`: {err}"))
    }

    fn write(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().expect("a file beneath the root")).expect("the parents");
        fs::write(path, text).expect("the file");
    }
}

impl Drop for Replica {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
