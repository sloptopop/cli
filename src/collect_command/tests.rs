//! Unit tests for [`super`], with the fake container runtimes in
//! `tests/fake-runtime/` (and, for `--host-tools`, the fake analyzers in
//! `tests/fake-tools/`) on the `PATH` handed to `collect`.

use std::ffi::OsString;
use std::io;

use serde_json::Value;

use super::*;
use crate::args::Source;
use crate::tests::support::{TempDir, fake_tools, fake_versions, runtime_path};

struct Ran {
    code: Result<u8, String>,
    stdout: String,
    stderr: String,
}

fn options(root: &Path) -> CollectOptions {
    CollectOptions {
        root: root.to_string_lossy().into_owned(),
        ..CollectOptions::default()
    }
}

/// An environment with `path` as `PATH`, a registry and a cache directory.
fn env(path: OsString, cache: &Path) -> Locator {
    Locator {
        path: Some(path),
        vars: vec![
            (
                "SLOPTOPOP_REGISTRY".to_owned(),
                OsString::from("reg.example"),
            ),
            (
                "SLOPTOPOP_CACHE_DIR".to_owned(),
                cache.as_os_str().to_owned(),
            ),
        ],
        ..Locator::default()
    }
}

fn run_with(options: &CollectOptions, env: &Locator) -> Ran {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = collect(options, env, &mut stdout, &mut stderr);
    Ran {
        code,
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    }
}

struct Repo {
    root: TempDir,
    cache: TempDir,
}

impl Repo {
    fn new(manifests: &[&str]) -> Self {
        let root = TempDir::new();
        for manifest in manifests {
            fs::write(root.0.join(manifest), "").unwrap();
        }
        fs::create_dir_all(root.0.join(".git")).unwrap();
        fake_versions(&root.0);
        Self {
            root,
            cache: TempDir::new(),
        }
    }

    fn run(&self, mode: &str) -> Ran {
        run_with(
            &options(&self.root.0),
            &env(runtime_path(mode), &self.cache.0),
        )
    }

    fn record(&self) -> Value {
        serde_json::from_str(
            &fs::read_to_string(self.root.0.join(".sloptopop/reports/.collect.json")).unwrap(),
        )
        .unwrap()
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.root.0.join(".fake-runtime-calls")).unwrap_or_default()
    }
}

fn statuses(record: &Value) -> Vec<(String, String)> {
    record["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| {
            (
                tool["name"].as_str().unwrap().to_owned(),
                format!(
                    "{} {}",
                    tool["status"].as_str().unwrap(),
                    tool.get("reason").and_then(Value::as_str).unwrap_or("")
                )
                .trim()
                .to_owned(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
        .collect()
}

#[test]
fn tools_run_from_their_verified_images() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = repo.run("docker");
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    assert_eq!(ran.stderr, "");
    assert!(
        ran.stdout.starts_with("fetch    gitleaks "),
        "{}",
        ran.stdout
    );
    assert!(
        ran.stdout
            .contains("fetch    db-osv-pypi      reg.example/analyzers/db-osv-pypi:latest"),
        "{}",
        ran.stdout
    );
    assert!(
        ran.stdout.contains(
            "sloptopop collect: 9 ran, 0 failed, 0 missing, 1 skipped (ecosystems: python)"
        ),
        "{}",
        ran.stdout
    );
    let record = repo.record();
    assert_eq!(record["schemaVersion"], 2);
    assert_eq!(record["mode"], "container");
    assert_eq!(record["registry"], "reg.example");
    assert_eq!(record["runtime"]["name"], "docker");
    assert_eq!(record["databases"][0]["name"], "db-osv-pypi");
    assert_eq!(
        statuses(&record),
        pairs(&[
            ("gitleaks", "ran"),
            ("gitleaks-history", "ran"),
            ("osv-scanner", "ran"),
            ("opengrep", "ran"),
            ("bandit", "ran"),
            ("ruff", "ran"),
            ("pylint-cyclic-import", "ran"),
            ("scorecard-license", "ran"),
            ("jscpd", "ran"),
            (
                "coverage.py",
                "skipped documented only: run it after your tests"
            ),
        ])
    );
    // A second run fetches nothing.
    let again = repo.run("docker");
    assert!(
        !again.stdout.contains("fetch    gitleaks"),
        "{}",
        again.stdout
    );
}

#[test]
fn images_get_their_data_and_rules_mounted_read_only() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = repo.run("docker");
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    let record = repo.record();
    let osv = &record["tools"][2];
    assert!(
        osv["image"]["reference"]
            .as_str()
            .unwrap()
            .starts_with("reg.example/analyzers/osv-scanner@sha256:")
    );
    let argv: Vec<&str> = osv["argv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap())
        .collect();
    let cache = repo.cache.0.join("db/db-osv-pypi");
    assert!(
        argv.iter().any(
            |a| a.starts_with(&format!("type=bind,source={}", cache.display()))
                && a.ends_with(",target=/db/osv-scalibr/PyPI,readonly")
        ),
        "{argv:?}"
    );
    let opengrep = &record["tools"][3]["argv"];
    assert!(
        opengrep.to_string().contains("target=/rules/0,readonly"),
        "{opengrep}"
    );
    let calls = repo.calls();
    assert_eq!(
        calls
            .matches(" pull --quiet reg.example/analyzers/gitleaks@sha256:")
            .count(),
        1,
        "one pull per image: {calls}"
    );
    assert!(
        repo.root.0.join(".sloptopop/reports/.rules").is_dir(),
        "the pack"
    );
    assert!(
        repo.root
            .0
            .join(".sloptopop/reports/pylint-cyclic-import.sarif")
            .is_file(),
        "converted"
    );
}

#[test]
fn the_c_and_cpp_analyzers_get_the_host_path_to_map_a_compile_database() {
    let repo = Repo::new(&["CMakeLists.txt"]);
    let ran = repo.run("docker");
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    let calls = repo.calls();
    let line = |entrypoint: &str| {
        calls
            .lines()
            .find(|line| line.contains(&format!("--entrypoint {entrypoint} ")))
            .unwrap_or_default()
            .to_owned()
    };
    let root = repo.root.0.to_string_lossy();
    for entrypoint in ["sloptopop-cppcheck", "sloptopop-clang-tidy"] {
        assert!(
            line(entrypoint).contains(&format!("--env SLOPTOPOP_HOST_ROOT={root} ")),
            "{entrypoint}: {calls}"
        );
    }
    assert_eq!(
        calls.matches("SLOPTOPOP_HOST_ROOT").count(),
        2,
        "no other tool: {calls}"
    );
    let statuses = statuses(&repo.record());
    assert!(
        statuses.contains(&("cppcheck".to_owned(), "ran".to_owned()))
            && statuses.contains(&("clang-tidy".to_owned(), "ran".to_owned())),
        "{statuses:?}"
    );
}

#[test]
fn go_and_dotnet_prepare_their_dependencies_once() {
    let repo = Repo::new(&["go.mod", "app.csproj"]);
    let ran = repo.run("docker");
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    let calls = repo.calls();
    assert_eq!(calls.matches("--entrypoint go ").count(), 1, "{calls}");
    assert!(
        calls.contains("--network bridge"),
        "prepare gets the network"
    );
    assert!(
        calls.contains("--env GOPROXY=https://proxy.golang.org,direct --env GOFLAGS=-mod=mod"),
        "{calls}"
    );
    let line = |entrypoint: &str| {
        calls
            .lines()
            .find(|line| line.contains(&format!("--entrypoint {entrypoint} ")))
            .unwrap_or_default()
            .to_owned()
    };
    assert!(
        line("go").contains("type=volume,source=sloptopop-gomodcache,target=/gomodcache --"),
        "rw for the download: {calls}"
    );
    assert!(
        line("golangci-lint")
            .contains("type=volume,source=sloptopop-gomodcache,target=/gomodcache,readonly"),
        "ro for the analysis: {calls}"
    );
    assert!(
        calls.contains("--entrypoint dotnet ")
            && calls.contains(" restore -p:DirectoryBuildPropsPath="),
        "{calls}"
    );
    let dotnet = format!(
        "source=sloptopop-dotnet-{},target=/work --",
        root_key(&repo.root.0)
    );
    assert_eq!(
        calls.matches(&dotnet).count(),
        2,
        "rw for the restore and for the build: {calls}"
    );
    let record = repo.record();
    let names: Vec<(String, String)> = statuses(&record);
    for tool in [
        "gosec",
        "govulncheck",
        "staticcheck",
        "golangci-lint",
        "sonaranalyzer-csharp",
    ] {
        assert!(
            names.contains(&(tool.to_owned(), "ran".to_owned())),
            "{tool}: {names:?}"
        );
    }
    assert!(record["databases"].to_string().contains("db-govulndb"));
    // Vendored modules need no download.
    let vendored = Repo::new(&["go.mod"]);
    fs::create_dir_all(vendored.root.0.join("vendor")).unwrap();
    fs::write(vendored.root.0.join("vendor/modules.txt"), "").unwrap();
    let _ran = vendored.run("docker");
    assert!(
        !vendored.calls().contains("--entrypoint go "),
        "{}",
        vendored.calls()
    );
}

#[test]
fn the_go_settings_of_the_user_reach_the_download() {
    let repo = Repo::new(&["go.mod"]);
    let mut env = env(runtime_path("docker"), &repo.cache.0);
    env.vars.push((
        "GOPROXY".to_owned(),
        OsString::from("https://proxy.corp.example"),
    ));
    env.vars
        .push(("GOPRIVATE".to_owned(), OsString::from("corp.example/*")));
    let _ran = run_with(&options(&repo.root.0), &env);
    let calls = repo.calls();
    assert!(calls.contains("--env GOPROXY=https://proxy.corp.example --env GOFLAGS=-mod=mod --env GOPRIVATE=corp.example/*"), "{calls}");
}

#[test]
fn a_failed_prepare_step_fails_the_tools_that_need_it() {
    let repo = Repo::new(&["go.mod"]);
    let _ran = repo.run("failing");
    let names = statuses(&repo.record());
    assert!(
        names.contains(&(
            "gosec".to_owned(),
            "failed prepare step (go mod download) failed: fake failure".to_owned()
        )),
        "{names:?}"
    );
}

#[test]
fn image_and_data_failures_are_per_tool() {
    let repo = Repo::new(&["pyproject.toml"]);
    let _ran = repo.run("nopull");
    let names = statuses(&repo.record());
    assert!(
        names.contains(&(
            "ruff".to_owned(),
            "failed pull failed: Error response from daemon: manifest unknown".to_owned()
        )),
        "{names:?}"
    );
    let without_data = Repo::new(&["pyproject.toml"]);
    let _ran_without_data = without_data.run("nodata");
    let data_names = statuses(&without_data.record());
    assert!(
        data_names.contains(&(
            "osv-scanner".to_owned(),
            "failed cannot fetch reg.example/analyzers/db-osv-pypi:latest: Error: not found"
                .to_owned()
        )),
        "{data_names:?}"
    );
    assert!(
        data_names.contains(&("ruff".to_owned(), "ran".to_owned())),
        "others still run"
    );
}

#[test]
fn offline_runs_use_only_what_is_there() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = run_with(
        &CollectOptions {
            offline: true,
            ..options(&repo.root.0)
        },
        &env(runtime_path("docker"), &repo.cache.0),
    );
    assert_eq!(ran.code, Ok(0));
    assert!(!ran.stdout.contains("fetch"), "{}", ran.stdout);
    let names = statuses(&repo.record());
    assert!(
        names[0]
            .1
            .starts_with("failed image not in the local store, and --offline forbids pulling it"),
        "{names:?}"
    );
}

#[test]
fn without_a_cache_directory_the_data_is_missing() {
    let repo = Repo::new(&["pyproject.toml"]);
    let env = Locator {
        path: Some(runtime_path("docker")),
        ..Locator::default()
    };
    let _ran = run_with(&options(&repo.root.0), &env);
    let record = repo.record();
    assert_eq!(record["registry"], DEFAULT_REGISTRY);
    let names = statuses(&record);
    assert!(
        names.contains(&(
            "osv-scanner".to_owned(),
            "failed no cache directory: set SLOPTOPOP_CACHE_DIR".to_owned()
        )),
        "{names:?}"
    );
}

#[test]
fn old_snapshots_are_flagged() {
    let repo = Repo::new(&["pyproject.toml"]);
    fs::write(repo.root.0.join(".fake-db-version"), "20200101").unwrap();
    let ran = repo.run("docker");
    assert!(
        ran.stderr
            .starts_with("warning: the db-osv-pypi snapshot is "),
        "{}",
        ran.stderr
    );
    assert!(ran.stderr.contains("days old (20200101)"), "{}", ran.stderr);
}

#[test]
fn snapshots_turn_stale_after_a_week() {
    let resolved = |version: &str| Resolved {
        snapshot: Snapshot {
            name: "db-osv-pypi",
            path: "osv-scalibr/PyPI",
        },
        reference: "reg/analyzers/db-osv-pypi@sha256:00".to_owned(),
        version: version.to_owned(),
        dir: PathBuf::from("/c"),
    };
    let warned = |version: &str| {
        let mut out = Vec::new();
        // 2026-10-21 is day 20_747.
        stale(&resolved(version), 20_747, &mut out);
        String::from_utf8(out).unwrap()
    };
    assert_eq!(warned("20261014"), "", "seven days is not stale");
    assert_eq!(
        warned("20261013"),
        "warning: the db-osv-pypi snapshot is 8 days old (20261013); the component library has not been updated\n"
    );
    assert_eq!(warned("latest"), "", "not a date");
    assert!(today() >= 20_747, "the clock is past 2026-10-21");
}

#[test]
fn without_a_runtime_nothing_runs_and_the_exit_code_is_3() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = repo.run("down");
    assert_eq!(ran.code, Ok(NO_RUNTIME));
    assert!(ran.stderr.starts_with("error: no container runtime found; sloptopop runs every analyzer in a pinned container image.\n  Tried: docker (on PATH, but `docker info` failed: "), "{}", ran.stderr);
    assert!(
        ran.stderr
            .contains("`sloptopop collect --host-tools` (unsupported)"),
        "{}",
        ran.stderr
    );
    let record = repo.record();
    assert!(record.get("runtime").is_none());
    let names = statuses(&record);
    assert_eq!(
        names[0],
        (
            "gitleaks".to_owned(),
            "missing no container runtime".to_owned()
        )
    );
    assert_eq!(
        names.last().unwrap().1,
        "skipped documented only: run it after your tests"
    );
    assert!(
        ran.stdout.contains("0 ran, 0 failed, 9 missing, 1 skipped"),
        "{}",
        ran.stdout
    );
}

#[test]
fn the_runtime_can_be_chosen() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = run_with(
        &CollectOptions {
            runtime: Some("podman".to_owned()),
            ..options(&repo.root.0)
        },
        &env(runtime_path("podman"), &repo.cache.0),
    );
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    assert_eq!(repo.record()["runtime"]["name"], "podman");
    assert!(
        repo.calls().contains("--userns keep-id"),
        "{}",
        repo.calls()
    );
    let mut by_env = env(runtime_path("podman"), &repo.cache.0);
    by_env.vars.push((
        "SLOPTOPOP_CONTAINER_RUNTIME".to_owned(),
        OsString::from("docker"),
    ));
    let only_docker = run_with(&options(&repo.root.0), &by_env);
    assert_eq!(only_docker.code, Ok(NO_RUNTIME), "only docker is tried");
}

#[test]
fn a_dry_run_shows_images_and_writes_nothing() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = run_with(
        &CollectOptions {
            dry_run: true,
            ..options(&repo.root.0)
        },
        &env(OsString::new(), &repo.cache.0),
    );
    assert_eq!(ran.code, Ok(0));
    assert!(
        ran.stdout.starts_with(
            "sloptopop collect: dry run, nothing fetched or run (ecosystems: python)\n"
        )
    );
    assert!(
        ran.stdout
            .contains("  run      gitleaks             reg.example/analyzers/gitleaks@sha256:"),
        "{}",
        ran.stdout
    );
    assert!(
        !repo.root.0.join(".sloptopop").exists(),
        "nothing written, no runtime needed"
    );
}

#[test]
fn host_tools_are_unpinned_and_say_so() {
    let repo = Repo::new(&["pyproject.toml"]);
    let host = CollectOptions {
        source: Source::HostTools,
        ..options(&repo.root.0)
    };
    let ran = run_with(
        &host,
        &env(fake_tools("findings").into_os_string(), &repo.cache.0),
    );
    assert_eq!(ran.code, Ok(0));
    assert!(
        ran.stderr
            .starts_with("warning: --host-tools runs unpinned tool versions from PATH"),
        "{}",
        ran.stderr
    );
    let record = repo.record();
    assert_eq!(record["mode"], "host-unpinned");
    let names = statuses(&record);
    assert!(
        names.contains(&("ruff".to_owned(), "ran".to_owned())),
        "{names:?}"
    );
    assert!(
        names.contains(&(
            "jscpd".to_owned(),
            "missing runs only from its pinned image; --host-tools cannot run it".to_owned()
        )),
        "{names:?}"
    );
    let dry = run_with(
        &CollectOptions {
            dry_run: true,
            ..host
        },
        &env(fake_tools("findings").into_os_string(), &repo.cache.0),
    );
    assert!(
        dry.stdout
            .contains("  run      ruff                 ruff check"),
        "{}",
        dry.stdout
    );
}

#[test]
fn the_old_container_flag_only_gets_a_note() {
    let repo = Repo::new(&["pyproject.toml"]);
    let ran = run_with(
        &CollectOptions {
            source: Source::ContainerFlag,
            dry_run: true,
            ..options(&repo.root.0)
        },
        &env(OsString::new(), &repo.cache.0),
    );
    assert_eq!(
        ran.stderr,
        "note: containers are the default now; --container has no effect\n"
    );
}

#[test]
fn explicit_rules_replace_the_pack() {
    let repo = Repo::new(&[]);
    let rules = TempDir::new();
    let ran = run_with(
        &CollectOptions {
            rules: Some(rules.0.to_string_lossy().into_owned()),
            only: vec!["opengrep".to_owned()],
            ..options(&repo.root.0)
        },
        &env(runtime_path("docker"), &repo.cache.0),
    );
    assert_eq!(ran.code, Ok(0), "{}", ran.stderr);
    assert!(
        !repo.root.0.join(".sloptopop/reports/.rules").exists(),
        "no pack written"
    );
    assert!(repo.calls().contains(&format!(
        "source={},target=/rules/0,readonly",
        rules.0.display()
    )));
}

#[test]
fn invalid_arguments_and_settings_are_usage_errors() {
    let repo = Repo::new(&[]);
    let cache = &repo.cache.0;
    let usage = |options: &CollectOptions, env: &Locator| run_with(options, env).code.unwrap_err();
    let base = env(runtime_path("docker"), cache);
    assert_eq!(
        usage(
            &CollectOptions {
                only: vec!["nope".to_owned()],
                ..options(&repo.root.0)
            },
            &base
        ),
        "unknown tool: nope (docs/collect.md lists them)"
    );
    assert_eq!(
        usage(
            &CollectOptions {
                root: "/no/such/dir".to_owned(),
                ..CollectOptions::default()
            },
            &base
        ),
        "not a directory: /no/such/dir"
    );
    assert_eq!(
        usage(
            &CollectOptions {
                rules: Some("/no/rules".to_owned()),
                ..options(&repo.root.0)
            },
            &base
        ),
        "rules directory not found: /no/rules"
    );
    for (name, value, message) in [
        (
            "SLOPTOPOP_REGISTRY",
            "https://x",
            "SLOPTOPOP_REGISTRY: not a registry: https://x",
        ),
        (
            "SLOPTOPOP_CONTAINER_RUNTIME",
            "nerdctl",
            "unknown container runtime: nerdctl (expected docker or podman)",
        ),
        (
            "SLOPTOPOP_COLLECT_MEMORY",
            "lots",
            "SLOPTOPOP_COLLECT_MEMORY: not a size: lots (e.g. 4g)",
        ),
        (
            "SLOPTOPOP_COLLECT_CPUS",
            "0",
            "SLOPTOPOP_COLLECT_CPUS: not a number: 0 (e.g. 2 or 1.5)",
        ),
    ] {
        let env = Locator {
            vars: vec![(name.to_owned(), OsString::from(value))],
            ..base.clone()
        };
        let error = usage(&options(&repo.root.0), &env);
        assert!(error.starts_with(message), "{name}: {error}");
    }
}

#[test]
fn settings_come_from_the_options_then_the_environment() {
    let cache = TempDir::new();
    let mut env = env(OsString::new(), &cache.0);
    env.vars.push((
        "SLOPTOPOP_COLLECT_MEMORY".to_owned(),
        OsString::from("512m"),
    ));
    env.vars
        .push(("SLOPTOPOP_COLLECT_CPUS".to_owned(), OsString::from("1.5")));
    let options = CollectOptions {
        registry: Some("given.example".to_owned()),
        timeout: Some(9),
        vuln_db: Some("20261012".to_owned()),
        ..CollectOptions::default()
    };
    let settings = Configured::new(&options, &env).unwrap();
    assert_eq!(settings.registry, "given.example", "the option wins");
    assert_eq!(
        settings.limits,
        Limits {
            memory: "512m".to_owned(),
            cpus: Some("1.5".to_owned())
        }
    );
    assert_eq!(settings.timeout, Duration::from_secs(9));
    assert_eq!(settings.vuln_db.as_deref(), Some("20261012"));
    assert_eq!(settings.cache, Some(cache.0.clone()));
    let defaults = Configured::new(&CollectOptions::default(), &Locator::default()).unwrap();
    assert_eq!(defaults.registry, DEFAULT_REGISTRY);
    assert_eq!(defaults.limits.memory, "4g");
    assert_eq!(defaults.timeout, Duration::from_mins(30));
    assert_eq!(defaults.cache, None);
    for (var, expected) in [
        ("XDG_CACHE_HOME", "/x/sloptopop"),
        ("HOME", "/x/.cache/sloptopop"),
        ("LOCALAPPDATA", "/x/sloptopop/cache"),
    ] {
        let cache_env = Locator {
            vars: vec![(var.to_owned(), OsString::from("/x"))],
            ..Locator::default()
        };
        assert_eq!(
            Configured::new(&CollectOptions::default(), &cache_env)
                .unwrap()
                .cache,
            Some(PathBuf::from(expected)),
            "{var}"
        );
    }
}

#[test]
fn quantities_are_positive_numbers() {
    for (value, unit, valid) in [
        ("4g", true, true),
        ("512m", true, true),
        ("1024", true, true),
        ("0g", true, false),
        ("g", true, false),
        ("1.5g", true, false),
        ("x", true, false),
        ("2", false, true),
        ("1.5", false, true),
        ("0", false, false),
        ("0.0", false, false),
        ("inf", false, false),
        ("-1", false, true),
        ("abc", false, false),
    ] {
        assert_eq!(quantity(value, unit), valid, "{value} {unit}");
    }
}

#[test]
fn keys_and_owners_are_stable() {
    let root = TempDir::new();
    assert_eq!(root_key(&root.0), root_key(&root.0));
    assert_eq!(root_key(&root.0).len(), 16);
    assert_ne!(root_key(&root.0), root_key(Path::new("/elsewhere")));
    #[cfg(unix)]
    assert!(owner(&root.0).unwrap().contains(':'));
    assert_eq!(owner(Path::new("/no/such/dir")), None);
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
fn an_unwritable_reports_directory_exits_1() {
    let repo = Repo::new(&[]);
    fs::write(repo.root.0.join(".sloptopop"), "a file").unwrap();
    let ran = repo.run("docker");
    assert_eq!(ran.code, Ok(WRITE_ERROR));
    assert!(
        ran.stderr.starts_with("error: cannot write "),
        "{}",
        ran.stderr
    );
}

#[test]
fn an_unwritable_reports_directory_exits_1_with_host_tools() {
    let host = Repo::new(&[]);
    fs::write(host.root.0.join(".sloptopop"), "a file").unwrap();
    let ran = run_with(
        &CollectOptions {
            source: Source::HostTools,
            ..options(&host.root.0)
        },
        &env(OsString::new(), &host.cache.0),
    );
    assert_eq!(ran.code, Ok(WRITE_ERROR));
}

#[test]
fn an_unwritable_pack_exits_1() {
    let pack = Repo::new(&[]);
    fs::create_dir_all(pack.root.0.join(".sloptopop/reports")).unwrap();
    fs::write(pack.root.0.join(".sloptopop/reports/.rules"), "a file").unwrap();
    let ran = pack.run("docker");
    assert_eq!(ran.code, Ok(WRITE_ERROR));
    assert!(ran.stderr.contains(".rules"), "{}", ran.stderr);
}

#[test]
fn an_unwritable_record_exits_1() {
    let record = Repo::new(&[]);
    fs::create_dir_all(record.root.0.join(".sloptopop/reports/.collect.json")).unwrap();
    let ran = record.run("docker");
    assert_eq!(ran.code, Ok(WRITE_ERROR));
    assert!(ran.stderr.contains(".collect.json"), "{}", ran.stderr);
}

#[test]
fn a_broken_stdout_exits_1() {
    let repo = Repo::new(&[]);
    let mut stderr = Vec::new();
    let code = collect(
        &CollectOptions {
            only: vec!["gitleaks".to_owned()],
            ..options(&repo.root.0)
        },
        &env(runtime_path("docker"), &repo.cache.0),
        &mut Broken,
        &mut stderr,
    );
    assert_eq!(code, Ok(WRITE_ERROR));
    let mut dry_stderr = Vec::new();
    let dry = collect(
        &CollectOptions {
            dry_run: true,
            ..options(&repo.root.0)
        },
        &env(OsString::new(), &repo.cache.0),
        &mut Broken,
        &mut dry_stderr,
    );
    assert_eq!(dry, Ok(WRITE_ERROR));
    assert!(String::from_utf8_lossy(&dry_stderr).starts_with("error: cannot write output: closed"));
}
