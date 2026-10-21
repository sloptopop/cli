//! Unit tests for [`super`], with the fake runtimes in
//! `tests/fake-runtime/` standing in for Docker.

use std::fs;

use super::*;
use crate::collect_runtime::detect;
use crate::tests::support::{TempDir, runtime_path};

const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

const PYPI: Snapshot = Snapshot {
    name: "db-osv-pypi",
    path: "osv-scalibr/PyPI",
};

struct Setup {
    root: TempDir,
    cache: TempDir,
    runtime: Runtime,
}

fn setup(mode: &str) -> Setup {
    let root = TempDir::new();
    let runtime = detect(Some(&runtime_path(mode)), None, &root.0).unwrap();
    Setup {
        root,
        cache: TempDir::new(),
        runtime,
    }
}

fn resolve_in(
    setup: &Setup,
    offline: bool,
    date: Option<&str>,
) -> (Result<Resolved, String>, String) {
    let context = Context {
        runtime: &setup.runtime,
        registry: "reg.example",
        cache: &setup.cache.0,
        offline,
        date,
        cwd: &setup.root.0,
        nonce: "7",
    };
    let mut log = Vec::new();
    let result = resolve(&context, PYPI, &mut log);
    (result, String::from_utf8(log).unwrap())
}

#[test]
fn each_ecosystem_has_its_snapshot() {
    let all = BTreeSet::from([
        Ecosystem::Cpp,
        Ecosystem::Dotnet,
        Ecosystem::Go,
        Ecosystem::Jvm,
        Ecosystem::Node,
        Ecosystem::Php,
        Ecosystem::Powershell,
        Ecosystem::Python,
        Ecosystem::Rust,
        Ecosystem::Shell,
        Ecosystem::Swift,
    ]);
    let names: Vec<&str> = snapshots(Data::Osv, &all).iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        [
            "db-osv-nuget",
            "db-osv-go",
            "db-osv-maven",
            "db-osv-npm",
            "db-osv-packagist",
            "db-osv-pypi",
            "db-osv-crates-io",
            "db-osv-swifturl"
        ]
    );
    let paths: Vec<&str> = snapshots(Data::Osv, &all).iter().map(|s| s.path).collect();
    assert!(
        paths.iter().all(|p| p.starts_with("osv-scalibr/")),
        "{paths:?}"
    );
    assert_eq!(
        snapshots(Data::Osv, &BTreeSet::from([Ecosystem::Shell])),
        []
    );
    assert_eq!(
        snapshots(Data::GoVuln, &BTreeSet::new()),
        [Snapshot {
            name: "db-govulndb",
            path: "govulndb"
        }]
    );
}

#[test]
fn the_newest_snapshot_is_fetched_recorded_and_unpacked_once() {
    let setup = setup("docker");
    let (resolved, log) = resolve_in(&setup, false, None);
    let resolved = resolved.unwrap();
    assert_eq!(
        log,
        "fetch    db-osv-pypi      reg.example/analyzers/db-osv-pypi:latest  CC-BY-4.0 (vulnerability data)\n"
    );
    assert_eq!(
        resolved.reference,
        format!("reg.example/analyzers/db-osv-pypi@sha256:{HEX}")
    );
    assert_eq!(resolved.version, "20261020");
    let store = setup.cache.0.join("db/db-osv-pypi").join(HEX);
    assert_eq!(resolved.dir, store.join("osv-scalibr/PyPI"));
    assert_eq!(
        fs::read_to_string(store.join(".version")).unwrap(),
        "20261020"
    );
    assert!(resolved.dir.join("all.zip").is_file());
    let calls = fs::read_to_string(setup.root.0.join(".fake-runtime-calls")).unwrap();
    assert!(calls.contains(&format!("docker create --name sloptopop-db-7 reg.example/analyzers/db-osv-pypi@sha256:{HEX} none\n")), "{calls}");
    assert!(
        calls.contains("docker rm --force sloptopop-db-7\n"),
        "{calls}"
    );
    // Again: the cached copy is used, nothing is created.
    fs::remove_file(setup.root.0.join(".fake-runtime-calls")).unwrap();
    let (again, _) = resolve_in(&setup, false, None);
    assert_eq!(again.unwrap(), resolved);
    let later = fs::read_to_string(setup.root.0.join(".fake-runtime-calls")).unwrap();
    assert!(!later.contains("create"), "{later}");
}

#[test]
fn a_date_selects_that_snapshot() {
    let setup = setup("docker");
    let (resolved, log) = resolve_in(&setup, false, Some("20261012"));
    assert!(resolved.is_ok());
    assert!(log.contains("db-osv-pypi:20261012"), "{log}");
}

#[test]
fn offline_uses_the_newest_cached_snapshot() {
    let setup = setup("docker");
    let (result, log) = resolve_in(&setup, true, None);
    assert_eq!(
        result,
        Err("no cached db-osv-pypi snapshot, and --offline forbids fetching one".to_owned())
    );
    assert_eq!(log, "");
    for (hex, version) in [("aa", "20261019"), ("bb", "20261021"), ("cc", "20261020")] {
        let dir = setup.cache.0.join("db/db-osv-pypi").join(hex);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(".version"), format!("{version}\n")).unwrap();
    }
    fs::create_dir_all(setup.cache.0.join("db/db-osv-pypi/incomplete.partial")).unwrap();
    let (resolved, _) = resolve_in(&setup, true, None);
    let resolved = resolved.unwrap();
    assert_eq!(resolved.version, "20261021");
    assert_eq!(
        resolved.reference,
        "reg.example/analyzers/db-osv-pypi@sha256:bb"
    );
    assert_eq!(
        resolved.dir,
        setup.cache.0.join("db/db-osv-pypi/bb/osv-scalibr/PyPI")
    );
}

#[test]
fn failures_say_what_failed() {
    let (result, _) = resolve_in(&setup("nodata"), false, None);
    assert_eq!(
        result,
        Err("cannot fetch reg.example/analyzers/db-osv-pypi:latest: Error: not found".to_owned())
    );
    let (uncopied, _) = resolve_in(&setup("nocopy"), false, None);
    assert_eq!(
        uncopied,
        Err(format!(
            "cannot unpack reg.example/analyzers/db-osv-pypi@sha256:{HEX}: Error: no such container"
        ))
    );
    let (uncreated, _) = resolve_in(&setup("nocreate"), false, None);
    assert_eq!(
        uncreated,
        Err(format!(
            "cannot unpack reg.example/analyzers/db-osv-pypi@sha256:{HEX}: exit status: 1"
        ))
    );
    let (unmarked, _) = resolve_in(&setup("badcopy"), false, None);
    let unmarked = unmarked.unwrap_err();
    assert!(
        unmarked.starts_with(&format!(
            "cannot unpack reg.example/analyzers/db-osv-pypi@sha256:{HEX}: "
        )),
        "{unmarked}"
    );
    let (uninspected, _) = resolve_in(&setup("lost"), false, None);
    assert_eq!(
        uninspected,
        Err("cannot inspect reg.example/analyzers/db-osv-pypi:latest: exit status: 1".to_owned())
    );
}

#[test]
fn an_unwritable_cache_is_reported() {
    let setup = setup("docker");
    let file = setup.cache.0.join("file");
    fs::write(&file, "").unwrap();
    let context = Context {
        runtime: &setup.runtime,
        registry: "reg.example",
        cache: &file,
        offline: false,
        date: None,
        cwd: &setup.root.0,
        nonce: "7",
    };
    let error = resolve(&context, PYPI, &mut Vec::new()).unwrap_err();
    assert!(
        error.starts_with(&format!(
            "cannot unpack reg.example/analyzers/db-osv-pypi@sha256:{HEX}: "
        )),
        "{error}"
    );
}

#[test]
fn a_digest_of_another_repository_is_not_used() {
    let setup = setup("docker");
    let context = Context {
        runtime: &setup.runtime,
        registry: "reg.example",
        cache: &setup.cache.0,
        offline: false,
        date: None,
        cwd: &setup.root.0,
        nonce: "7",
    };
    // The fake lists this one's tag under another repository only.
    let odd = Snapshot {
        name: "db-foreign",
        path: "x",
    };
    let mut log = Vec::new();
    let result = resolve(&context, odd, &mut log);
    assert_eq!(
        result,
        Err("the store lists no digest of reg.example/analyzers/db-foreign for reg.example/analyzers/db-foreign:latest".to_owned())
    );
}

#[test]
fn a_runtime_that_cannot_start_is_reported() {
    let setup = setup("docker");
    let gone = Runtime {
        program: setup.root.0.join("no-such-docker"),
        ..setup.runtime.clone()
    };
    let context = Context {
        runtime: &gone,
        registry: "reg.example",
        cache: &setup.cache.0,
        offline: false,
        date: None,
        cwd: &setup.root.0,
        nonce: "7",
    };
    let error = resolve(&context, PYPI, &mut Vec::new()).unwrap_err();
    assert!(
        error.starts_with(
            "cannot fetch reg.example/analyzers/db-osv-pypi:latest: cannot run docker: "
        ),
        "{error}"
    );
}

#[test]
fn ages_count_whole_days() {
    // 2026-10-21 is day 20_747 since 1970-01-01.
    assert_eq!(age("20261021", 20_747), Some(0));
    assert_eq!(age("20261012", 20_747), Some(9));
    assert_eq!(age("19700101", 0), Some(0));
    assert_eq!(age("20000301", 11_017), Some(0), "after a leap day");
    assert_eq!(age("20000229", 11_017), Some(1));
    assert_eq!(age("20260101", 20_454), Some(0), "January");
    for bad in [
        "2026",
        "2026102",
        "202610211",
        "2026x021",
        "20261321",
        "20261000",
        "20261032",
        "abcd1021",
        "202610ab",
        "",
    ] {
        assert_eq!(age(bad, 20_747), None, "{bad}");
    }
}
