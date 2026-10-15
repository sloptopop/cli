//! Unit tests for [`super`].

use std::ffi::OsString;

use super::*;
use crate::collect_catalogue::{Arg, CATALOGUE, Image, Scope};
use crate::tests::support::{TempDir, fake_tools, install_managed};

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

const BOXED: Tool = Tool {
    name: "boxed",
    command: "gosec",
    licence: "MIT",
    licence_files: &[],
    scope: Scope::CrossCutting,
    kind: Kind::Run,
    args: &[Arg::Fixed("-out"), Arg::Output(""), Arg::Fixed("./...")],
    output: "boxed.sarif",
    sink: Sink::PathArgument,
    findings: &[1],
    network: Network::Offline,
    image: Some(Image {
        repository: "example/gosec",
        digest: Some(DIGEST),
    }),
};

struct Fixture {
    root: TempDir,
    ecosystems: BTreeSet<Ecosystem>,
    only: Vec<String>,
    pack: PathBuf,
    local: Option<PathBuf>,
    git: bool,
    container: bool,
    path: OsString,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new();
        let pack = root.0.join(".sloptopop/reports/.rules");
        Self {
            root,
            ecosystems: BTreeSet::from([Ecosystem::Python]),
            only: Vec::new(),
            pack,
            local: None,
            git: true,
            container: false,
            path: fake_tools("findings").into_os_string(),
        }
    }

    fn plan<'t>(&self, catalogue: &'t [Tool]) -> Vec<Planned<'t>> {
        plan(
            catalogue,
            &Settings {
                root: &self.root.0,
                ecosystems: &self.ecosystems,
                only: &self.only,
                rules: RuleDirs {
                    first: &self.pack,
                    second: self.local.as_deref(),
                },
                git: self.git,
                container: self.container,
                path: Some(&self.path),
            },
        )
    }

    fn one(&self, tool: &Tool) -> Planned<'static> {
        let catalogue: &'static [Tool] = Box::leak(Box::new([*tool]));
        self.plan(catalogue).remove(0)
    }
}

fn named(name: &str) -> &'static Tool {
    CATALOGUE.iter().find(|tool| tool.name == name).unwrap()
}

fn names(planned: &[Planned<'_>]) -> Vec<&'static str> {
    planned.iter().map(|p| p.tool.name).collect()
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn applicable_entries_are_planned_in_catalogue_order() {
    let mut fixture = Fixture::new();
    assert_eq!(
        names(&fixture.plan(CATALOGUE)),
        [
            "gitleaks",
            "gitleaks-history",
            "osv-scanner",
            "opengrep",
            "semgrep",
            "bandit",
            "ruff",
            "coverage.py"
        ],
        "python repository"
    );
    fixture.only = strings(&["gosec", "ruff", "gitleaks"]);
    assert_eq!(
        names(&fixture.plan(CATALOGUE)),
        ["gitleaks", "ruff"],
        "--only narrows, never adds an entry that does not apply"
    );
}

#[test]
fn installed_tools_run_from_path() {
    let fixture = Fixture::new();
    let ruff = fixture.one(named("ruff"));
    assert_eq!(
        ruff.argv,
        strings(&[
            "ruff",
            "check",
            "--output-format",
            "sarif",
            "--output-file",
            ".sloptopop/reports/ruff.sarif",
            "--no-cache",
            "."
        ]),
        "argv"
    );
    assert_eq!(
        ruff.step,
        Step::Run {
            program: fake_tools("findings").join("ruff"),
            version: strings(&["--version"]),
            stdout: None,
        },
        "writes its own file"
    );
    let osv = fixture.one(named("osv-scanner"));
    assert_eq!(
        osv.step,
        Step::Run {
            program: fake_tools("findings").join("osv-scanner"),
            version: strings(&["--version"]),
            stdout: Some(fixture.root.0.join(".sloptopop/reports/osv-scanner.sarif")),
        },
        "stdout captured"
    );
}

#[test]
fn absent_tools_are_missing() {
    let mut fixture = Fixture::new();
    fixture.path = OsString::new();
    assert_eq!(
        fixture.one(named("bandit")).step,
        Step::Missing("not found on PATH".to_owned()),
        "bandit"
    );
}

#[test]
fn entries_are_skipped_for_a_reason() {
    let mut fixture = Fixture::new();
    fixture.git = false;
    let skipped = |planned: Planned<'_>| {
        let Step::Skipped(reason) = planned.step else {
            panic!("{planned:?}");
        };
        reason
    };
    assert_eq!(
        skipped(fixture.one(named("coverage.py"))),
        "documented only: run it after your tests",
        "coverage"
    );
    assert_eq!(
        skipped(fixture.one(named("gitleaks-history"))),
        "not a git repository",
        "history"
    );
    fixture.git = true;
    assert!(
        matches!(
            fixture.one(named("gitleaks-history")).step,
            Step::Run { .. }
        ),
        "git repository"
    );
}

#[test]
fn rule_directories_are_passed_in_order() {
    let mut fixture = Fixture::new();
    let config = |planned: &Planned<'_>| -> Vec<String> {
        planned
            .argv
            .windows(2)
            .filter(|pair| pair[0] == "--config")
            .map(|pair| pair[1].clone())
            .collect()
    };
    let pack = fixture.pack.to_string_lossy().into_owned();
    let semgrep = fixture.one(named("semgrep"));
    assert!(matches!(semgrep.step, Step::Run { .. }), "always runs");
    assert_eq!(config(&semgrep), std::slice::from_ref(&pack), "the pack");
    let local = fixture.root.0.join(".sloptopop/rules");
    fixture.local = Some(local.clone());
    assert_eq!(
        config(&fixture.one(named("opengrep"))),
        [pack, local.to_string_lossy().into_owned()],
        "the pack, then the local rules"
    );
}

#[cfg(unix)]
fn owner(path: &Path) -> Vec<String> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).unwrap();
    strings(&["--user", &format!("{}:{}", meta.uid(), meta.gid())])
}

#[cfg(not(unix))]
fn owner(_: &Path) -> Vec<String> {
    Vec::new()
}

fn docker(fixture: &Fixture, network: &str, extra: &[String], tail: &[&str]) -> Vec<String> {
    let root = fixture.root.0.to_string_lossy();
    let mut argv = strings(&["docker", "run", "--rm", "--network", network]);
    argv.extend(owner(&fixture.root.0));
    argv.extend(strings(&[
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--env",
        "HOME=/tmp",
        "--mount",
        &format!("type=bind,source={root},target=/src,readonly"),
        "--mount",
        &format!("type=bind,source={root}/.sloptopop/reports,target=/src/.sloptopop/reports"),
    ]));
    argv.extend_from_slice(extra);
    argv.extend(strings(&["--workdir", "/src"]));
    argv.extend(strings(tail));
    argv
}

#[test]
fn containers_run_pinned_images_offline_and_read_only() {
    let mut fixture = Fixture::new();
    fixture.container = true;
    fixture.local = Some(fixture.root.0.join("rules"));
    let image = format!("example/gosec@{DIGEST}");
    let boxed = fixture.one(&BOXED);
    assert_eq!(
        boxed.argv,
        docker(
            &fixture,
            "none",
            &[],
            &[
                "--entrypoint",
                "gosec",
                &image,
                "-out",
                ".sloptopop/reports/boxed.sarif",
                "./..."
            ]
        ),
        "docker argv, no rules mounted for a tool without rules"
    );
    assert_eq!(
        boxed.step,
        Step::Run {
            program: fake_tools("findings").join("docker"),
            version: strings(&[
                "run",
                "--rm",
                "--network",
                "none",
                "--entrypoint",
                "gosec",
                &image,
                "--version"
            ]),
            stdout: None,
        },
        "docker runs it"
    );
    assert!(
        !boxed
            .argv
            .iter()
            .any(|arg| arg.starts_with("-p") || arg.starts_with("--publish")),
        "no ports"
    );
}

#[test]
fn containers_mount_rules_and_allow_only_database_downloads() {
    let mut fixture = Fixture::new();
    fixture.container = true;
    fixture.local = Some(fixture.root.0.join("my rules"));
    let tool = Tool {
        args: &[Arg::Rules("--config")],
        sink: Sink::Stdout,
        network: Network::DatabaseDownload,
        ..BOXED
    };
    let image = format!("example/gosec@{DIGEST}");
    let mount = |dir: &Path, index: usize| {
        format!(
            "type=bind,source={},target=/rules/{index},readonly",
            dir.to_string_lossy()
        )
    };
    let planned = fixture.one(&tool);
    assert_eq!(
        planned.argv,
        docker(
            &fixture,
            "bridge",
            &strings(&[
                "--mount",
                &mount(&fixture.pack, 0),
                "--mount",
                &mount(&fixture.root.0.join("my rules"), 1)
            ]),
            &[
                "--entrypoint",
                "gosec",
                &image,
                "--config",
                "/rules/0",
                "--config",
                "/rules/1"
            ]
        ),
        "rules mounted read-only; network for the download"
    );
    let Step::Run {
        version, stdout, ..
    } = planned.step
    else {
        panic!("runs");
    };
    assert_eq!(
        version.get(3).map(String::as_str),
        Some("none"),
        "version offline"
    );
    assert_eq!(
        stdout,
        Some(fixture.root.0.join(".sloptopop/reports/boxed.sarif")),
        "stdout captured"
    );
}

#[test]
fn containers_need_a_pinned_image_and_docker() {
    let mut fixture = Fixture::new();
    fixture.container = true;
    let unpinned = Tool {
        image: Some(Image {
            repository: "example/gosec",
            digest: None,
        }),
        ..BOXED
    };
    let imageless = Tool {
        image: None,
        ..BOXED
    };
    for (tool, reason) in [
        (unpinned, "container image digest not pinned"),
        (imageless, "no container image catalogued"),
    ] {
        let planned = fixture.one(&tool);
        assert_eq!(planned.step, Step::Skipped(reason.to_owned()), "{reason}");
        assert_eq!(
            planned.argv.first().map(String::as_str),
            Some("gosec"),
            "{reason}"
        );
    }
    assert!(
        matches!(fixture.one(named("coverage.py")).step, Step::Skipped(_)),
        "documented entries are skipped first"
    );
    fixture.path = OsString::new();
    let planned = fixture.one(&BOXED);
    assert_eq!(
        planned.step,
        Step::Missing("docker not found on PATH".to_owned()),
        "no docker"
    );
    assert_eq!(
        planned.argv.first().map(String::as_str),
        Some("docker"),
        "argv"
    );
}

#[test]
fn managed_tools_are_missing_until_installed_then_run() {
    let mut fixture = Fixture::new();
    fixture.ecosystems = BTreeSet::from([Ecosystem::Node, Ecosystem::Dotnet]);
    let tools = fixture.root.0.join(".sloptopop/tools");
    let script = tools
        .join("eslint-sonarjs/node_modules/eslint/bin/eslint.js")
        .to_string_lossy()
        .into_owned();
    let eslint = fixture.one(named("eslint-sonarjs"));
    assert_eq!(
        eslint.step,
        Step::Missing(
            "not installed in .sloptopop/tools/eslint-sonarjs: run \
             `npm ci --ignore-scripts --prefix .sloptopop/tools/eslint-sonarjs`"
                .to_owned()
        ),
        "npm"
    );
    assert_eq!(
        eslint.argv[..3],
        strings(&["node", &script, "--config"]),
        "argv"
    );
    let props = format!(
        "-p:DirectoryBuildPropsPath={}",
        tools
            .join("sonaranalyzer-csharp/Directory.Build.props")
            .display()
    );
    let dotnet = fixture.one(named("sonaranalyzer-csharp"));
    assert_eq!(
        dotnet.step,
        Step::Missing(
            "not installed in .sloptopop/tools/sonaranalyzer-csharp: run \
             `dotnet restore .sloptopop/tools/sonaranalyzer-csharp/fetch.proj`"
                .to_owned()
        ),
        "NuGet"
    );
    assert_eq!(dotnet.argv.last(), Some(&props), "the generated props");
    install_managed(&fixture.root.0);
    assert_eq!(
        fixture.one(named("eslint-sonarjs")).step,
        Step::Run {
            program: fake_tools("findings").join("node"),
            version: strings(&[&script, "--version"]),
            stdout: None,
        },
        "ESLint's version"
    );
    assert_eq!(
        fixture.one(named("sonaranalyzer-csharp")).step,
        Step::Run {
            program: fake_tools("findings").join("dotnet"),
            version: strings(&["--version"]),
            stdout: None,
        },
        "the props name the reports"
    );
}

#[test]
fn managed_tools_have_no_container_image() {
    let mut fixture = Fixture::new();
    fixture.ecosystems = BTreeSet::from([Ecosystem::Node]);
    fixture.container = true;
    assert_eq!(
        fixture.one(named("eslint-sonarjs-json")).step,
        Step::Skipped("no container image catalogued".to_owned()),
        "skipped before the installation is looked at"
    );
}
