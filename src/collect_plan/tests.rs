//! Unit tests for [`super`].

use std::ffi::OsString;

use super::*;
use crate::collect_catalogue::{Arg, CATALOGUE, Scope};
use crate::tests::support::{TempDir, fake_tools};

const IMAGE: Image = Image {
    name: "analyzers/boxed",
    version: "1-1",
    digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    bytes: [2_000_000, 2_000_000],
};

const BOXED: Tool = Tool {
    name: "boxed",
    command: "gosec",
    licence: "MIT",
    licence_files: &[],
    scope: Scope::CrossCutting,
    kind: Kind::Run(IMAGE),
    default: true,
    args: &[Arg::Fixed("-out"), Arg::Output(""), Arg::Fixed("./...")],
    host: Host::Same,
    output: "boxed.sarif",
    sink: Sink::PathArgument,
    findings: &[1],
    convert: None,
    env: &[],
    prepare: None,
    data: None,
    sarif_fixes: None,
    refusals: &[],
};

struct Fixture {
    root: TempDir,
    ecosystems: BTreeSet<Ecosystem>,
    only: Vec<String>,
    pack: PathBuf,
    local: Option<PathBuf>,
    git: bool,
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
        }
    }

    fn settings(&self) -> Settings<'_> {
        Settings {
            root: &self.root.0,
            ecosystems: &self.ecosystems,
            only: &self.only,
            rules: RuleDirs {
                first: &self.pack,
                second: self.local.as_deref(),
            },
            git: self.git,
        }
    }

    fn names<R>(planned: &[Planned<'_, R>]) -> Vec<&'static str> {
        planned.iter().map(|p| p.tool.name).collect()
    }
}

#[test]
fn images_cover_every_applicable_default_entry() {
    let fixture = Fixture::new();
    let planned = images(CATALOGUE, &fixture.settings());
    assert_eq!(
        Fixture::names(&planned),
        [
            "gitleaks",
            "gitleaks-history",
            "osv-scanner",
            "opengrep",
            "bandit",
            "ruff",
            "pylint-cyclic-import",
            "scorecard-license",
            "jscpd",
            "coverage.py"
        ],
        "python, no semgrep by default"
    );
    for entry in &planned {
        match (&entry.step, entry.tool.kind) {
            (Step::Run(image), Kind::Run(pinned) | Kind::NeedsGit(pinned)) => {
                assert_eq!(*image, pinned);
            }
            (Step::NotRun(NotRun::Skipped(reason)), Kind::Documented) => {
                assert_eq!(reason, "documented only: run it after your tests");
            }
            other => panic!("{}: {other:?}", entry.tool.name),
        }
    }
}

#[test]
fn only_selects_and_can_add_semgrep() {
    let mut fixture = Fixture::new();
    fixture.only = vec!["semgrep".to_owned(), "ruff".to_owned(), "oxlint".to_owned()];
    let planned = images(CATALOGUE, &fixture.settings());
    assert_eq!(
        Fixture::names(&planned),
        ["semgrep", "ruff"],
        "oxlint does not apply"
    );
}

#[test]
fn history_needs_git() {
    let mut fixture = Fixture::new();
    fixture.git = false;
    fixture.only = vec!["gitleaks-history".to_owned()];
    let planned = images(CATALOGUE, &fixture.settings());
    assert_eq!(
        planned[0].step,
        Step::NotRun(NotRun::Skipped("not a git repository".to_owned()))
    );
}

#[test]
fn rules_are_mounted_in_a_container_and_named_on_the_host() {
    let mut fixture = Fixture::new();
    let local = fixture.root.0.join(".sloptopop/rules");
    fixture.local = Some(local.clone());
    fixture.only = vec!["opengrep".to_owned()];
    let planned = images(CATALOGUE, &fixture.settings());
    assert_eq!(
        planned[0].argv,
        [
            "opengrep",
            "scan",
            "--json",
            "--no-rewrite-rule-ids",
            "--config",
            "/rules/0",
            "--config",
            "/rules/1",
            "."
        ]
    );
    assert_eq!(
        planned[0].stdout,
        Some(fixture.root.0.join(".sloptopop/reports/.raw/opengrep.json"))
    );
    let path = fake_tools("findings").into_os_string();
    let host = host(CATALOGUE, &fixture.settings(), Some(path.as_os_str()));
    let config: Vec<&String> = host[0].argv.iter().skip(5).step_by(2).take(2).collect();
    assert_eq!(
        config,
        [
            &fixture.pack.to_string_lossy().into_owned(),
            &local.to_string_lossy().into_owned()
        ]
    );
    assert_eq!(
        host[0].step,
        Step::Run(fake_tools("findings").join("opengrep"))
    );
}

#[test]
fn host_tools_find_programs_or_say_why_not() {
    let mut fixture = Fixture::new();
    fixture.ecosystems = BTreeSet::from([Ecosystem::Node]);
    fixture.only = vec![
        "oxlint".to_owned(),
        "eslint-sonarjs".to_owned(),
        "c8".to_owned(),
    ];
    let empty = OsString::new();
    let planned = host(CATALOGUE, &fixture.settings(), Some(empty.as_os_str()));
    assert_eq!(
        planned[0].step,
        Step::NotRun(NotRun::Missing("not found on PATH".to_owned()))
    );
    assert_eq!(
        planned[1].step,
        Step::NotRun(NotRun::Missing(
            "runs only from its pinned image; --host-tools cannot run it".to_owned()
        ))
    );
    assert!(
        matches!(planned[2].step, Step::NotRun(NotRun::Skipped(_))),
        "documented"
    );
}

#[test]
fn host_tools_use_their_host_arguments() {
    let mut fixture = Fixture::new();
    fixture.only = vec!["osv-scanner".to_owned()];
    let path = fake_tools("findings").into_os_string();
    let planned = host(CATALOGUE, &fixture.settings(), Some(path.as_os_str()));
    assert!(
        planned[0]
            .argv
            .contains(&"--download-offline-databases".to_owned())
    );
    fixture.only.clear();
    let boxed = host(&[BOXED], &fixture.settings(), Some(path.as_os_str()));
    assert_eq!(
        boxed[0].argv,
        ["gosec", "-out", ".sloptopop/reports/boxed.sarif", "./..."]
    );
    assert_eq!(boxed[0].stdout, None);
    let images = images(&[BOXED], &fixture.settings());
    assert_eq!(images[0].step, Step::Run(IMAGE));
}

#[test]
fn rule_directories_iterate_in_order() {
    let fixture = Fixture::new();
    let second = PathBuf::from("/x");
    let dirs = RuleDirs {
        first: &fixture.pack,
        second: Some(&second),
    };
    assert_eq!(
        dirs.iter().collect::<Vec<_>>(),
        [fixture.pack.as_path(), second.as_path()]
    );
}
