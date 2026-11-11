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

// The pin keeps the root's version, so the key alone retargets the crate.
#[test]
fn retargeted_pin() {
    for (key, value) in [("git", "https://github.com/augentic/omnia"), ("package", "omnia-test")] {
        let replica = Replica::of_repo();
        let seed = replica.read(SEED_MANIFEST);
        let pin = seed
            .lines()
            .find(|line| line.starts_with("omnia-sdk = "))
            .expect("the seed pins `omnia-sdk`");
        let version = pin.trim_start_matches("omnia-sdk = ");
        replica.write(
            SEED_MANIFEST,
            &seed.replace(
                pin,
                &format!("omnia-sdk = {{ version = {version}, {key} = \"{value}\" }}"),
            ),
        );

        let failures = template_check::run(replica.root()).expect("contract inputs readable");

        let [retarget, body] = failures.as_slice() else {
            panic!("two failures for `{key}`: {failures:#?}")
        };
        assert_eq!(
            retarget,
            &format!(
                "templates/guest/core/Cargo.toml: `omnia-sdk` sets `{key}`, which a seed \
                 dependency never does"
            )
        );
        assert!(body.ends_with("embeds a body other than its source"), "{body}");
    }
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
fn seed_features() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    let pin =
        seed.lines().find(|line| line.starts_with("serde = ")).expect("the seed pins `serde`");
    replica.write(
        SEED_MANIFEST,
        &seed.replace(pin, "serde = { version = \"1.0.229\", features = [\"derive\", \"rc\"] }"),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [features, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(
        features,
        "templates/guest/core/Cargo.toml: `serde` enables [\"derive\", \"rc\"], the root pins \
         [\"derive\"]"
    );
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

#[test]
fn patched_crate() {
    let replica = Replica::of_repo();
    let seed = replica.read(SEED_MANIFEST);
    let patched = seed.replacen(
        "[patch.crates-io]\n",
        "[patch.crates-io]\ntokio = { git = \"https://github.com/tokio-rs/tokio\" }\n",
        1,
    );
    assert_ne!(patched, seed, "the seed has a `[patch.crates-io]` table");
    replica.write(SEED_MANIFEST, &patched);

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    let [patch, body] = failures.as_slice() else { panic!("two failures: {failures:#?}") };
    assert_eq!(
        patch,
        "templates/guest/core/Cargo.toml: `[patch.crates-io]` retargets `tokio` behind the pins"
    );
    assert!(body.ends_with("embeds a body other than its source"), "{body}");
}

#[test]
fn uncarried_root_patch() {
    let replica = Replica::of_repo();
    let root = replica.read("Cargo.toml");
    let patched = root.replacen(
        "[patch.crates-io]\n",
        "[patch.crates-io]\ntokio = { git = \"https://github.com/tokio-rs/tokio\" }\n",
        1,
    );
    assert_ne!(patched, root, "the root has a `[patch.crates-io]` table");
    replica.write("Cargo.toml", &patched);

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    assert_eq!(
        failures,
        ["templates/guest/core/Cargo.toml: `tokio` is patched in the root workspace \
             (`[patch.crates-io]`), not in the seed"]
    );
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
// replacement there retargets crates the manifests pin honestly, and a
// case-insensitive volume reads it under any spelling. Each spelling has a
// replica of its own, since on such a volume two would share one directory
// and the orphan walk would see the other's file.
#[test]
fn cargo_config_scaffolded() {
    for dir in [".cargo", ".Cargo"] {
        let replica = Replica::of_repo();
        let source = format!("templates/guest/core/{dir}/config.toml");
        let target = format!("{dir}/config.toml");
        replica.write(
            &source,
            "[source.crates-io]\nreplace-with = \"mirror\"\n\n[source.mirror]\nregistry = \
             \"https://example.invalid/index\"\n",
        );
        let manifest = replica.read("templates/guest/manifest.yaml");
        replica.write(
            "templates/guest/manifest.yaml",
            &format!(
                "{manifest}      - source: {source}\n        target: {target}\n        proof: \
                 seed\n"
            ),
        );

        let failures = template_check::run(replica.root()).expect("contract inputs readable");

        let [config, row] = failures.as_slice() else {
            panic!("two failures for `{target}`: {failures:#?}")
        };
        assert_eq!(
            config,
            &format!("manifest: target `{target}` configures cargo behind the pins")
        );
        assert_eq!(
            row,
            &format!("manifest: `{source}` -> `{target}` (seed) has no row in the crate's table")
        );
    }
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

#[test]
fn clippy_paths() {
    let replica = Replica::of_repo();
    let crates_clippy = replica.read("crates/clippy.toml");
    replica.write(
        "crates/clippy.toml",
        &crates_clippy.replacen(
            "disallowed-methods = [\n",
            "disallowed-methods = [\n  { path = \"std::mem::forget\", reason = \"no leaks\" },\n",
            1,
        ),
    );

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    assert_eq!(
        failures,
        ["templates/guest/core/clippy.toml: `disallowed-methods` lacks `std::mem::forget`, \
             which `crates/clippy.toml` bans"]
    );
}

#[test]
fn clippy_config_missing() {
    let replica = Replica::of_repo();
    fs::remove_file(replica.root().join("crates/clippy.toml")).expect("removed");

    let failures = template_check::run(replica.root()).expect("contract inputs readable");

    assert_eq!(failures, ["crates/clippy.toml: clippy config unreadable"]);
}

// The files the gate reads, copied beneath a scratch root a scenario edits:
// the manifest, the root manifest the pins are held to, `crates/clippy.toml`,
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
        let gated = ["Cargo.toml", "crates/clippy.toml", "templates/guest/manifest.yaml"];
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
