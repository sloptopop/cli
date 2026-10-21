//! Unit tests for [`super`], with the committed fake analyzers in
//! `tests/fake-tools/` and fake runtimes in `tests/fake-runtime/` standing
//! in for the real ones.

use std::fs;

use super::*;
use crate::collect_catalogue::{CATALOGUE, Kind, RAW_DIR, Sink};
use crate::collect_container::Mount;
use crate::collect_runtime::detect;
use crate::tests::support::{TempDir, fake_tools, runtime_path};

fn named(name: &str) -> &'static Tool {
    CATALOGUE.iter().find(|tool| tool.name == name).unwrap()
}

fn image_of(name: &str) -> Image {
    match named(name).kind {
        Kind::Run(image) | Kind::NeedsGit(image) => image,
        Kind::Documented => panic!("{name}"),
    }
}

fn root() -> TempDir {
    let root = TempDir::new();
    fs::create_dir_all(root.0.join(RAW_DIR)).unwrap();
    root
}

fn host_plan(tool: &'static Tool, program: PathBuf, root: &TempDir) -> Planned<'static, PathBuf> {
    Planned {
        tool,
        argv: tool.argv(&[], true),
        stdout: (tool.sink == Sink::Stdout).then(|| root.0.join(tool.written_path())),
        step: Step::Run(program),
    }
}

fn failed(status: Status) -> (Option<i32>, String) {
    let Status::Failed { code, detail } = status else {
        panic!("{status:?}");
    };
    (code, detail)
}

fn calls(root: &TempDir, file: &str) -> String {
    fs::read_to_string(root.0.join(file)).unwrap_or_default()
}

#[test]
fn exit_codes_map_to_statuses() {
    let unused = || -> String { panic!("no detail for success") };
    assert_eq!(status(Some(0), &[], unused), Status::Ran(0), "success");
    assert_eq!(status(Some(1), &[2, 1], unused), Status::Ran(1), "findings");
    let detail = || "boom".to_owned();
    assert_eq!(
        status(Some(2), &[1], detail),
        Status::failure("boom").with_code(2),
        "other code"
    );
    assert_eq!(
        status(None, &[1], detail),
        Status::failure("boom"),
        "signal"
    );
}

impl Status {
    fn with_code(self, code: i32) -> Self {
        let Self::Failed { detail, .. } = self else {
            panic!("{self:?}");
        };
        Self::Failed {
            code: Some(code),
            detail,
        }
    }
}

#[test]
fn statuses_expose_name_code_and_reason() {
    for (status, name, code, reason) in [
        (Status::Ran(1), "ran", Some(1), None),
        (
            Status::failure("boom").with_code(70),
            "failed",
            Some(70),
            Some("boom"),
        ),
        (Status::Missing("m".to_owned()), "missing", None, Some("m")),
        (Status::Skipped("s".to_owned()), "skipped", None, Some("s")),
    ] {
        assert_eq!(
            (status.name(), status.exit_code(), status.reason()),
            (name, code, reason),
            "{status:?}"
        );
    }
    assert_eq!(
        Status::from(NotRun::Missing("m".to_owned())),
        Status::Missing("m".to_owned())
    );
    assert_eq!(
        Status::from(NotRun::Skipped("s".to_owned())),
        Status::Skipped("s".to_owned())
    );
}

#[test]
fn a_host_tool_writing_its_own_report_runs_in_the_root() {
    let root = root();
    let outcome = run_host(
        host_plan(named("ruff"), fake_tools("findings").join("ruff"), &root),
        &root.0,
    );
    assert_eq!(outcome.version.as_deref(), Some("ruff 1.0.0"), "first line");
    assert_eq!(outcome.status, Status::Ran(1), "findings are success");
    assert_eq!(outcome.argv, named("ruff").argv(&[], true), "argv kept");
    assert_eq!(outcome.image, None);
    assert_eq!(
        calls(&root, ".fake-calls"),
        "ruff --version\nruff check --output-format sarif --output-file .sloptopop/reports/ruff.sarif --no-cache --extend-select S110,S501,N999 .\n",
        "argv, run in the root"
    );
    assert!(
        fs::read_to_string(root.0.join(".sloptopop/reports/ruff.sarif"))
            .unwrap()
            .contains("\"name\": \"ruff\""),
        "normalized"
    );
}

#[test]
fn stdout_is_captured_then_normalized() {
    let root = root();
    let outcome = run_host(
        host_plan(
            named("osv-scanner"),
            fake_tools("findings").join("osv-scanner"),
            &root,
        ),
        &root.0,
    );
    assert_eq!(outcome.status, Status::Ran(1), "vulnerabilities found");
    let report = fs::read_to_string(root.0.join(".sloptopop/reports/osv-scanner.sarif")).unwrap();
    assert!(
        report.starts_with("{\n") && report.contains("\"osv-scanner\""),
        "pretty, normalized: {report}"
    );
}

#[test]
fn failures_are_recorded_and_leave_no_report() {
    let root = root();
    let stale = root.0.join(".sloptopop/reports/ruff.sarif");
    fs::write(&stale, "{}").unwrap();
    let broken = run_host(
        host_plan(named("ruff"), fake_tools("broken").join("ruff"), &root),
        &root.0,
    );
    assert_eq!(broken.version, None, "failed --version");
    assert_eq!(
        broken.status,
        Status::failure("fake failure").with_code(70),
        "first stderr line"
    );
    assert!(!stale.exists(), "no stale or partial report");
    let killed = run_host(
        host_plan(
            named("staticcheck"),
            fake_tools("killed").join("staticcheck"),
            &root,
        ),
        &root.0,
    );
    assert_eq!(
        killed.version.as_deref(),
        Some("staticcheck 1.0.0"),
        "version"
    );
    let (code, detail) = failed(killed.status);
    assert_eq!(code, None, "no exit code");
    assert!(detail.contains("signal"), "{detail}");
    assert!(!root.0.join(".sloptopop/reports/staticcheck.sarif").exists());
}

#[test]
fn not_running_and_unstartable_host_tools() {
    let root = root();
    let mut plan = host_plan(named("ruff"), PathBuf::new(), &root);
    plan.step = Step::NotRun(NotRun::Missing("not found on PATH".to_owned()));
    let missing = run_host(plan, &root.0);
    assert_eq!(
        missing.status,
        Status::Missing("not found on PATH".to_owned())
    );
    assert_eq!(missing.version, None);
    let program = root.0.join("not-a-program");
    fs::write(&program, "").unwrap();
    let outcome = run_host(host_plan(named("ruff"), program.clone(), &root), &root.0);
    let (code, detail) = failed(outcome.status);
    assert_eq!(code, None);
    assert!(
        detail.starts_with(&format!("cannot run {}: ", program.display())),
        "{detail}"
    );
    let mut unwritable = host_plan(
        named("osv-scanner"),
        fake_tools("findings").join("osv-scanner"),
        &root,
    );
    unwritable.stdout = Some(root.0.join("missing/out.json"));
    let (_, why) = failed(run_host(unwritable, &root.0).status);
    assert!(
        why.starts_with("cannot run "),
        "the stdout file cannot be created: {why}"
    );
}

#[test]
fn earlier_reports_that_cannot_be_removed_stop_the_tool() {
    let root = root();
    fs::create_dir_all(root.0.join(".sloptopop/reports/osv-scanner.sarif")).unwrap();
    let outcome = run_host(
        host_plan(
            named("osv-scanner"),
            fake_tools("findings").join("osv-scanner"),
            &root,
        ),
        &root.0,
    );
    let (_, detail) = failed(outcome.status);
    assert!(
        detail.starts_with("cannot remove an earlier report: "),
        "{detail}"
    );
    assert_eq!(
        calls(&root, ".fake-calls"),
        "osv-scanner --version\n",
        "never run"
    );
}

#[test]
fn reports_matching_a_pattern_are_cleared() {
    let root = root();
    let reports = root.0.join(".sloptopop/reports");
    for name in [
        "sonaranalyzer-csharp.Old.sarif",
        "sonaranalyzer-csharp.sarif",
        "other.sarif",
    ] {
        fs::write(reports.join(name), "{}").unwrap();
    }
    remove_reports(&root.0, named("sonaranalyzer-csharp")).unwrap();
    assert!(!reports.join("sonaranalyzer-csharp.Old.sarif").exists());
    assert!(
        reports.join("sonaranalyzer-csharp.sarif").exists(),
        "the * stands for at least the dot-separated project name"
    );
    assert!(reports.join("other.sarif").exists());
    let gone = TempDir::new();
    assert!(
        remove_reports(&gone.0, named("sonaranalyzer-csharp")).is_err(),
        "no reports directory"
    );
    assert!(
        remove_reports(&gone.0, named("ruff")).is_ok(),
        "nothing to remove"
    );
}

#[test]
fn only_sarif_reports_are_normalized() {
    let root = root();
    fs::write(root.0.join(REPORTS_DIR).join("cargo.lcov"), "TN:\n").unwrap();
    assert_eq!(
        sarif_reports(&root.0, named("cargo-llvm-cov")).unwrap(),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn conversions_that_fail_are_failures() {
    let root = root();
    let tool = named("pylint-cyclic-import");
    let (code, detail) = failed(finish(&root.0, tool, "v", Status::Ran(0)));
    assert_eq!(code, Some(0));
    assert!(
        detail.starts_with("cannot read .sloptopop/reports/.raw/pylint-cyclic-import.json: "),
        "{detail}"
    );
    fs::write(root.0.join(tool.written_path()), "not json").unwrap();
    let (_, unreadable) = failed(finish(&root.0, tool, "v", Status::Ran(0)));
    assert!(
        unreadable.starts_with("pylint-cyclic-import printed no readable JSON"),
        "{unreadable}"
    );
    fs::write(root.0.join(tool.written_path()), "{\"messages\":[]}").unwrap();
    fs::create_dir_all(root.0.join(tool.output_path())).unwrap();
    let (_, unwritable) = failed(finish(&root.0, tool, "v", Status::Ran(0)));
    assert!(
        unwritable.starts_with("cannot write .sloptopop/reports/pylint-cyclic-import.sarif: "),
        "{unwritable}"
    );
    fs::remove_dir(root.0.join(tool.output_path())).unwrap();
    assert_eq!(finish(&root.0, tool, "v", Status::Ran(0)), Status::Ran(0));
    assert_eq!(
        finish(&root.0, tool, "v", Status::Missing("m".to_owned())),
        Status::Missing("m".to_owned())
    );
    assert_eq!(
        finish(&root.0, named("ruff"), "v", Status::Ran(1)),
        Status::Ran(1),
        "SARIF already"
    );
    let gone = TempDir::new();
    let (_, unlisted) = failed(finish(
        &gone.0,
        named("sonaranalyzer-csharp"),
        "v",
        Status::Ran(0),
    ));
    assert!(
        unlisted.starts_with("cannot list the reports: "),
        "{unlisted}"
    );
}

struct Boxed {
    root: TempDir,
    runtime: Runtime,
    limits: Limits,
}

fn container(mode: &str) -> Boxed {
    let root = root();
    let runtime = detect(Some(&runtime_path(mode)), None, &root.0).unwrap();
    Boxed {
        root,
        runtime,
        limits: Limits {
            memory: "4g".to_owned(),
            cpus: None,
        },
    }
}

impl Boxed {
    fn shared(&self, timeout: Duration) -> Shared<'_> {
        Shared {
            runtime: &self.runtime,
            limits: &self.limits,
            owner: Some("1:2"),
            root: &self.root.0,
            timeout,
        }
    }
}

fn job<'a>(tool: &'a Tool, image: &'a str) -> Job<'a> {
    Job {
        name: "sloptopop-collect-test-0".to_owned(),
        network: false,
        env: Vec::new(),
        mounts: vec![Mount::bind(Path::new("/r"), "/src", true)],
        entrypoint: tool.command,
        image,
        args: tool.argv(&[], false).into_iter().skip(1).collect(),
    }
}

fn image_plan(tool: &'static Tool, root: &TempDir) -> Planned<'static, Image> {
    Planned {
        tool,
        argv: tool.argv(&[], false),
        stdout: (tool.sink == Sink::Stdout).then(|| root.0.join(tool.written_path())),
        step: Step::Run(image_of(tool.name)),
    }
}

#[test]
fn an_image_runs_hardened_and_records_its_reference() {
    let setup = container("docker");
    let tool = named("ruff");
    let image = image_of("ruff");
    let reference = image.reference("reg");
    let used = Used {
        reference: reference.clone(),
        version: image.version,
    };
    let job = job(tool, &reference);
    let shared = setup.shared(Duration::from_secs(60));
    let outcome = run_image(
        &image_plan(tool, &setup.root),
        &image,
        used.clone(),
        &job,
        &shared,
    );
    assert_eq!(outcome.status, Status::Ran(1));
    assert_eq!(
        outcome.version.as_deref(),
        Some(image.version),
        "the image's version label"
    );
    assert_eq!(outcome.image, Some(used));
    assert_eq!(outcome.argv, job.argv(&shared));
    assert!(
        outcome.argv.windows(2).any(|w| w == ["--user", "1:2"]),
        "{:?}",
        outcome.argv
    );
    let calls = calls(&setup.root, ".fake-runtime-calls");
    assert!(
        calls.contains(&format!("--entrypoint ruff {reference} check")),
        "{calls}"
    );
    assert!(setup.root.0.join(".sloptopop/reports/ruff.sarif").is_file());
}

#[test]
fn a_converted_image_tool_gets_its_sarif() {
    let setup = container("docker");
    let tool = named("scorecard-license");
    let image = image_of("scorecard-license");
    let used = Used {
        reference: "r".to_owned(),
        version: image.version,
    };
    let job = job(tool, "reg/analyzers/scorecard@sha256:00");
    let outcome = run_image(
        &image_plan(tool, &setup.root),
        &image,
        used,
        &job,
        &setup.shared(Duration::from_secs(60)),
    );
    assert_eq!(outcome.status, Status::Ran(0));
    let sarif = fs::read_to_string(
        setup
            .root
            .0
            .join(".sloptopop/reports/scorecard-license.sarif"),
    )
    .unwrap();
    assert!(
        sarif.contains("license file not detected") && sarif.contains(image.version),
        "{sarif}"
    );
}

#[test]
fn containers_past_their_time_limit_are_stopped() {
    let setup = container("slow");
    let tool = named("ruff");
    let shared = setup.shared(Duration::from_secs(1));
    let status = run_job(&shared, &job(tool, "img@sha256:00"), None, &[]);
    assert_eq!(
        status,
        Status::failure("stopped after 1 s (the time limit; --timeout changes it)")
    );
    assert!(
        calls(&setup.root, ".fake-runtime-calls")
            .contains("docker kill sloptopop-collect-test-0\n")
    );
}

#[test]
fn failing_and_unstartable_containers() {
    let setup = container("failing");
    let tool = named("ruff");
    let shared = setup.shared(Duration::from_secs(60));
    assert_eq!(
        run_job(&shared, &job(tool, "img@sha256:00"), None, &[1]),
        Status::failure("fake failure").with_code(70)
    );
    let gone = Runtime {
        program: setup.root.0.join("no-such-docker"),
        ..setup.runtime.clone()
    };
    let shared = Shared {
        runtime: &gone,
        ..shared
    };
    let (_, detail) = failed(run_job(&shared, &job(tool, "img"), None, &[]));
    assert!(detail.starts_with("cannot run docker: "), "{detail}");
    let ready = container("docker");
    let ready_shared = ready.shared(Duration::from_secs(60));
    let stale = ready.root.0.join(".sloptopop/reports/ruff.sarif");
    fs::create_dir_all(&stale).unwrap();
    fs::write(stale.join("x"), "").unwrap();
    let image = image_of("ruff");
    let used = Used {
        reference: "r".to_owned(),
        version: image.version,
    };
    let outcome = run_image(
        &image_plan(tool, &ready.root),
        &image,
        used,
        &job(tool, "img"),
        &ready_shared,
    );
    let (_, blocked) = failed(outcome.status);
    assert!(
        blocked.starts_with("cannot remove an earlier report: "),
        "{blocked}"
    );
}

#[test]
fn entries_that_did_not_run_keep_their_plan() {
    let root = root();
    let plan = image_plan(named("ruff"), &root);
    let image = image_of("ruff");
    let used = Used {
        reference: "r".to_owned(),
        version: image.version,
    };
    let outcome = Outcome::without_run(plan.clone(), Some(used.clone()), Status::failure("x"));
    assert_eq!(outcome.argv, plan.argv);
    assert_eq!(outcome.version.as_deref(), Some(image.version));
    assert_eq!(outcome.image, Some(used));
    let bare = Outcome::without_run(plan, None, Status::Skipped("s".to_owned()));
    assert_eq!(bare.version, None);
}
