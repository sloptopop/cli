//! Unit tests for [`super`], with the fake runtimes in
//! `tests/fake-runtime/` standing in for Docker and Podman.

use std::ffi::OsString;

use super::*;
use crate::tests::support::{TempDir, runtime_path};

fn detected(mode: &str, wanted: Option<Engine>) -> Result<Runtime, String> {
    let dir = TempDir::new();
    detect(Some(&runtime_path(mode)), wanted, &dir.0)
}

#[test]
fn engines_have_names() {
    for engine in [Engine::Docker, Engine::Podman] {
        assert_eq!(Engine::parse(engine.name()), Some(engine));
    }
    assert_eq!(Engine::parse("nerdctl"), None);
}

#[test]
fn docker_is_found_first() {
    let runtime = detected("docker", None).unwrap();
    assert_eq!(runtime.engine, Engine::Docker);
    assert_eq!(runtime.version, "29.1.3");
    assert!(!runtime.rootless);
    assert!(runtime.limits);
    assert!(runtime.program.ends_with("docker"));
    let rootless = detected("rootless", None).unwrap();
    assert!(rootless.rootless, "name=rootless in the security options");
}

#[test]
fn podman_reports_rootless_and_its_controllers() {
    let podman = detected("podman", None).unwrap();
    assert_eq!(podman.engine, Engine::Podman, "no docker on PATH");
    assert_eq!(podman.version, "5.6.0");
    assert!(podman.rootless);
    assert!(podman.limits);
    let limited = detected("nolimits", Some(Engine::Podman)).unwrap();
    assert!(!limited.limits, "no memory controller");
}

#[test]
fn what_was_tried_is_reported() {
    assert_eq!(
        detected("down", None).unwrap_err(),
        "docker (on PATH, but `docker info` failed: Cannot connect to the Docker daemon at unix:///var/run/docker.sock), \
         podman (on PATH, but `podman info` failed: Cannot connect to the Docker daemon at unix:///var/run/docker.sock)"
    );
    assert_eq!(
        detected("silent", Some(Engine::Docker)).unwrap_err(),
        "docker (on PATH, but `docker info` failed: it printed no server version)"
    );
    let dir = TempDir::new();
    assert_eq!(
        detect(Some(&OsString::from(dir.0.as_os_str())), None, &dir.0).unwrap_err(),
        "docker (not on PATH), podman (not on PATH)"
    );
    assert_eq!(
        detected("podman", Some(Engine::Docker)).unwrap_err(),
        "docker (not on PATH)",
        "only the wanted engine"
    );
}

#[test]
fn a_cli_that_cannot_start_is_reported() {
    let dir = TempDir::new();
    let program = dir.0.join("docker");
    std::fs::write(&program, "not a script").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let reason = detect(Some(dir.0.as_os_str()), Some(Engine::Docker), &dir.0).unwrap_err();
    assert!(
        reason.starts_with("docker (on PATH, but `docker info` failed: "),
        "{reason}"
    );
}

#[test]
fn reports_belong_to_the_user() {
    let runtime = |engine, rootless| Runtime {
        engine,
        program: "x".into(),
        version: "1".to_owned(),
        rootless,
        limits: true,
    };
    let owner = Some("501:20");
    assert_eq!(
        runtime(Engine::Docker, false).user(owner),
        ["--user", "501:20"]
    );
    assert_eq!(runtime(Engine::Docker, true).user(owner), ["--user", "0:0"]);
    assert_eq!(
        runtime(Engine::Podman, true).user(owner),
        ["--userns", "keep-id", "--user", "501:20"]
    );
    assert_eq!(
        runtime(Engine::Podman, true).user(None),
        ["--userns", "keep-id"]
    );
    assert_eq!(
        runtime(Engine::Podman, false).user(owner),
        ["--user", "501:20"]
    );
    assert_eq!(
        runtime(Engine::Docker, false).user(None),
        Vec::<String>::new()
    );
    assert_eq!(runtime(Engine::Docker, true).user(None), ["--user", "0:0"]);
}

#[test]
fn output_runs_the_cli() {
    let dir = TempDir::new();
    let runtime = detected("docker", None).unwrap();
    let output = runtime.output(&["info"], &dir.0).unwrap();
    assert!(output.status.success());
    assert_eq!(
        first_line(&output.stdout).unwrap(),
        "29.1.3|[\"name=seccomp,profile=builtin\"]|true"
    );
    let calls = std::fs::read_to_string(dir.0.join(".fake-runtime-calls")).unwrap();
    assert_eq!(calls, "docker info\n", "run in cwd");
}

#[test]
fn first_lines_skip_blank_ones() {
    assert_eq!(
        first_line(b"\n  \n hello \nworld"),
        Some("hello".to_owned())
    );
    assert_eq!(first_line(b" \n"), None);
}
