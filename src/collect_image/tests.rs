//! Unit tests for [`super`], with the fake runtimes in
//! `tests/fake-runtime/` standing in for Docker.

use std::fs;
use std::io;

use super::*;
use crate::collect_runtime::detect;
use crate::tests::support::{TempDir, runtime_path};

const IMAGE: Image = Image {
    name: "analyzers/ruff",
    version: "0.17.1-1",
    digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    bytes: [11_570_831, 10_938_905],
};

struct Setup {
    root: TempDir,
    runtime: Runtime,
}

fn setup(mode: &str) -> Setup {
    let root = TempDir::new();
    fs::write(root.0.join(".fake-versions"), "ruff 0.17.1-1\n").unwrap();
    let runtime = detect(Some(&runtime_path(mode)), None, &root.0).unwrap();
    Setup { root, runtime }
}

fn ensure_in(setup: &Setup, offline: bool) -> (Result<String, String>, String) {
    let mut log = Vec::new();
    let result = ensure(
        &setup.runtime,
        "reg.example",
        &IMAGE,
        "MIT",
        offline,
        &setup.root.0,
        &mut log,
    );
    (result, String::from_utf8(log).unwrap())
}

const REFERENCE: &str = "reg.example/analyzers/ruff@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn sizes_read_as_megabytes() {
    assert_eq!(megabytes(11_570_831), "11.6 MB");
    assert_eq!(megabytes(49_999), "0.0 MB");
    assert_eq!(megabytes(50_000), "0.1 MB");
    assert_eq!(megabytes(0), "0.0 MB");
    assert_eq!(megabytes(352_298_027), "352.3 MB");
}

#[test]
fn a_missing_image_is_announced_then_pulled_by_digest() {
    let setup = setup("docker");
    let (result, log) = ensure_in(&setup, false);
    assert_eq!(result, Ok(REFERENCE.to_owned()));
    let size = megabytes(IMAGE.pull_bytes(std::env::consts::ARCH));
    assert_eq!(
        log,
        format!("fetch    ruff             0.17.1-1         {size:>10}  {REFERENCE}  MIT\n")
    );
    let calls = fs::read_to_string(setup.root.0.join(".fake-runtime-calls")).unwrap();
    assert!(
        calls.contains(&format!("docker pull --quiet {REFERENCE}\n")),
        "{calls}"
    );
    // Present now: no second fetch.
    let (again, second_log) = ensure_in(&setup, false);
    assert_eq!(again, Ok(REFERENCE.to_owned()));
    assert_eq!(second_log, "", "nothing fetched");
}

#[test]
fn offline_never_pulls() {
    let setup = setup("docker");
    let (result, log) = ensure_in(&setup, true);
    assert_eq!(
        result,
        Err(format!(
            "image not in the local store, and --offline forbids pulling it: {REFERENCE}"
        ))
    );
    assert_eq!(log, "");
}

#[test]
fn an_image_loaded_from_an_archive_runs_by_its_id() {
    let setup = setup("loaded");
    // The fake's loaded images carry labels x/y: the label check still applies.
    let (result, _) = ensure_in(&setup, true);
    assert_eq!(
        result,
        Err(
            "image labels do not match the catalogue: tool \"x\", version \"y\" (expected \"ruff\", \"0.17.1-1\")"
                .to_owned()
        )
    );
}

#[test]
fn labels_must_match_the_catalogue() {
    let setup = setup("relabelled");
    let (result, _) = ensure_in(&setup, false);
    assert_eq!(
        result,
        Err(
            "image labels do not match the catalogue: tool \"other\", version \"9\" (expected \"ruff\", \"0.17.1-1\")"
                .to_owned()
        )
    );
}

#[test]
fn verification_accepts_a_matching_id() {
    let found = Found {
        repo_digests: Vec::new(),
        id: IMAGE.digest.to_owned(),
        labels: serde_json::json!({ "io.sloptopop.tool": "ruff", "org.opencontainers.image.version": "0.17.1-1" }),
    };
    assert_eq!(
        verify(&found, &IMAGE, REFERENCE),
        Ok(IMAGE.digest.to_owned())
    );
    let podman_style = Found {
        id: IMAGE.digest.trim_start_matches("sha256:").to_owned(),
        ..found.clone()
    };
    assert_eq!(
        verify(&podman_style, &IMAGE, REFERENCE),
        Ok(IMAGE.digest.to_owned())
    );
    let other = Found {
        id: "sha256:ff".to_owned(),
        repo_digests: vec!["reg.example/analyzers/ruff@sha256:ff".to_owned()],
        ..found
    };
    assert_eq!(
        verify(&other, &IMAGE, REFERENCE),
        Err(format!(
            "image digest mismatch: the store has no {REFERENCE}"
        ))
    );
}

#[test]
fn both_labels_must_match() {
    let found = |tool: &str, version: &str| Found {
        repo_digests: vec![REFERENCE.to_owned()],
        id: "sha256:ff".to_owned(),
        labels: serde_json::json!({ "io.sloptopop.tool": tool, "org.opencontainers.image.version": version }),
    };
    assert_eq!(
        verify(&found("ruff", "0.17.1-1"), &IMAGE, REFERENCE),
        Ok(REFERENCE.to_owned())
    );
    for (tool, version) in [("ruff", "0.17.0-1"), ("bandit", "0.17.1-1")] {
        assert!(
            verify(&found(tool, version), &IMAGE, REFERENCE)
                .unwrap_err()
                .starts_with("image labels do not match the catalogue"),
            "{tool} {version}"
        );
    }
}

#[test]
fn failed_pulls_are_reported() {
    let setup = setup("nopull");
    let (result, log) = ensure_in(&setup, false);
    assert_eq!(
        result,
        Err("pull failed: Error response from daemon: manifest unknown".to_owned())
    );
    assert!(log.starts_with("fetch"), "announced before the attempt");
    let lost = self::setup("lost");
    let (unlisted, _) = ensure_in(&lost, false);
    assert_eq!(
        unlisted,
        Err(format!("pulled, but the store does not list {REFERENCE}"))
    );
}

#[test]
fn a_runtime_that_cannot_start_is_reported() {
    let setup = setup("docker");
    let gone = Runtime {
        program: setup.root.0.join("no-such-docker"),
        ..setup.runtime.clone()
    };
    let mut log = Vec::new();
    let result = ensure(
        &gone,
        "reg.example",
        &IMAGE,
        "MIT",
        false,
        &setup.root.0,
        &mut log,
    );
    assert!(result.unwrap_err().starts_with("cannot run docker pull: "));
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_log_that_cannot_be_written_does_not_stop_the_fetch() {
    let setup = setup("docker");
    let result = ensure(
        &setup.runtime,
        "reg.example",
        &IMAGE,
        "MIT",
        false,
        &setup.root.0,
        &mut Broken,
    );
    assert_eq!(result, Ok(REFERENCE.to_owned()));
}
