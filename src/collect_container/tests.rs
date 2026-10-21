//! Unit tests for [`super`].

use std::path::{Path, PathBuf};

use super::*;
use crate::collect_runtime::Engine;

fn runtime(limits: bool) -> Runtime {
    Runtime {
        engine: Engine::Docker,
        program: PathBuf::from("/usr/bin/docker"),
        version: "29.1.3".to_owned(),
        rootless: false,
        limits,
    }
}

fn spec<'a>(runtime: &'a Runtime, limits: &'a Limits) -> Spec<'a> {
    Spec {
        runtime,
        name: "sloptopop-collect-1-0".to_owned(),
        network: false,
        owner: Some("501:20".to_owned()),
        limits,
        env: vec![("OSV".to_owned(), "/db".to_owned())],
        mounts: vec![
            Mount::bind(Path::new("/repo"), "/src", true),
            Mount::bind(
                Path::new("/repo/.sloptopop/reports"),
                "/src/.sloptopop/reports",
                false,
            ),
            Mount::Volume {
                name: "cache".to_owned(),
                target: "/gomodcache".to_owned(),
                readonly: true,
            },
            Mount::Volume {
                name: "work".to_owned(),
                target: "/work".to_owned(),
                readonly: false,
            },
        ],
        entrypoint: "ruff",
        image: "reg/analyzers/ruff@sha256:ab",
        args: vec!["check".to_owned(), ".".to_owned()],
    }
}

#[test]
fn the_run_is_hardened() {
    let runtime = runtime(true);
    let limits = Limits {
        memory: "4g".to_owned(),
        cpus: Some("2".to_owned()),
    };
    assert_eq!(
        run_argv(&spec(&runtime, &limits)),
        [
            "docker",
            "run",
            "--rm",
            "--pull",
            "never",
            "--name",
            "sloptopop-collect-1-0",
            "--network",
            "none",
            "--read-only",
            "--tmpfs",
            "/tmp:rw,exec,nosuid,nodev,size=1g",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--user",
            "501:20",
            "--pids-limit",
            "1024",
            "--memory",
            "4g",
            "--memory-swap",
            "4g",
            "--cpus",
            "2",
            "--hostname",
            "sloptopop",
            "--env",
            "HOME=/tmp",
            "--env",
            "TZ=UTC",
            "--env",
            "LANG=C.UTF-8",
            "--env",
            "LC_ALL=C.UTF-8",
            "--env",
            "OSV=/db",
            "--mount",
            "type=bind,source=/repo,target=/src,readonly",
            "--mount",
            "type=bind,source=/repo/.sloptopop/reports,target=/src/.sloptopop/reports",
            "--mount",
            "type=volume,source=cache,target=/gomodcache,readonly",
            "--mount",
            "type=volume,source=work,target=/work",
            "--workdir",
            "/src",
            "--entrypoint",
            "ruff",
            "reg/analyzers/ruff@sha256:ab",
            "check",
            "."
        ]
    );
}

#[test]
fn limits_need_a_runtime_that_applies_them() {
    let runtime = runtime(false);
    let limits = Limits {
        memory: "4g".to_owned(),
        cpus: Some("2".to_owned()),
    };
    let argv = run_argv(&spec(&runtime, &limits));
    for flag in ["--pids-limit", "--memory", "--memory-swap", "--cpus"] {
        assert!(!argv.contains(&flag.to_owned()), "{flag}: {argv:?}");
    }
    let capable = self::runtime(true);
    let uncapped = Limits {
        memory: "2g".to_owned(),
        cpus: None,
    };
    let capped = run_argv(&spec(&capable, &uncapped));
    assert!(
        capped.contains(&"2g".to_owned()) && !capped.contains(&"--cpus".to_owned()),
        "{capped:?}"
    );
}

#[test]
fn a_prepare_step_gets_the_network() {
    let runtime = runtime(true);
    let limits = Limits {
        memory: "4g".to_owned(),
        cpus: None,
    };
    let mut spec = spec(&runtime, &limits);
    spec.network = true;
    spec.owner = None;
    let argv = run_argv(&spec);
    assert!(
        argv.windows(2).any(|w| w == ["--network", "bridge"]),
        "{argv:?}"
    );
    assert!(
        !argv.contains(&"--user".to_owned()),
        "no owner: the runtime decides"
    );
}
