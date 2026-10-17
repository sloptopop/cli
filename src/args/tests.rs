//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use super::*;

#[test]
fn check_and_fix_arguments_are_parsed() {
    let cases = [
        (&["check"][..], Command::Check(".".to_owned(), Format::Text)),
        (
            &["check", "--format", "json"],
            Command::Check(".".to_owned(), Format::Json),
        ),
        (
            &["check", "src"],
            Command::Check("src".to_owned(), Format::Text),
        ),
        (
            &["check", "src", "--format", "text"],
            Command::Check("src".to_owned(), Format::Text),
        ),
        (&["fix"], Command::Fix(".".to_owned(), false)),
        (&["fix", "--apply"], Command::Fix(".".to_owned(), true)),
        (&["fix", "src"], Command::Fix("src".to_owned(), false)),
        (
            &["fix", "src", "--apply"],
            Command::Fix("src".to_owned(), true),
        ),
    ];
    for (args, expected) in cases {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        assert_eq!(parse(&args), Ok(expected), "{args:?}");
    }
    for (args, message) in [
        (
            &["check", "--format", "xml"][..],
            "unknown format: xml (expected text or json)",
        ),
        (&["check", "--json"], "unrecognised option: --json"),
        (&["check", "a", "b"], "unrecognised arguments: check a b"),
        (&["fix", "--force"], "unrecognised option: --force"),
        (&["fix", "a", "b"], "unrecognised arguments: fix a b"),
    ] {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        assert_eq!(parse(&args), Err(message.to_owned()), "{args:?}");
    }
}

fn collect(args: &[&str]) -> Result<Command, String> {
    let args: Vec<OsString> = std::iter::once("collect")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect();
    parse(&args)
}

#[test]
fn collect_defaults_to_the_current_directory() {
    assert_eq!(
        collect(&[]),
        Ok(Command::Collect(CollectOptions {
            root: ".".to_owned(),
            ..CollectOptions::default()
        })),
        "defaults"
    );
}

#[test]
fn collect_options_are_parsed_in_any_order() {
    let expected = Command::Collect(CollectOptions {
        root: "repo".to_owned(),
        container: true,
        only: vec![
            "ruff".to_owned(),
            "gitleaks".to_owned(),
            "bandit".to_owned(),
        ],
        rules: Some("rules".to_owned()),
        dry_run: true,
    });
    for args in [
        &[
            "repo",
            "--container",
            "--only",
            "ruff,gitleaks",
            "--rules",
            "rules",
            "--dry-run",
            "--only",
            "bandit",
        ][..],
        &[
            "--dry-run",
            "--only",
            "ruff",
            "--rules",
            "elsewhere",
            "--container",
            "--only",
            "gitleaks,bandit",
            "repo",
            "--rules",
            "rules",
        ],
    ] {
        assert_eq!(collect(args), Ok(expected.clone()), "{args:?}");
    }
}

#[test]
fn collect_rejects_bad_arguments() {
    for (args, message) in [
        (&["--only"][..], "--only needs a value"),
        (&["--only", "--dry-run"], "--only needs a value"),
        (&["--rules"], "--rules needs a value"),
        (&["--only", "ruff,,bandit"], "--only has an empty tool name"),
        (&["--force"], "unrecognised option: --force"),
        (&["a", "b"], "unrecognised option or argument: b"),
        (
            &["a", "--force"],
            "unrecognised option or argument: --force",
        ),
    ] {
        assert_eq!(collect(args), Err(message.to_owned()), "{args:?}");
    }
}

#[test]
fn transform_arguments_are_parsed() {
    let parsed = |args: &[&str]| parse(&args.iter().map(OsString::from).collect::<Vec<_>>());
    let Ok(Command::Transform(defaults)) = parsed(&["transform"]) else {
        panic!("transform");
    };
    assert_eq!(
        defaults,
        TransformOptions {
            root: ".".to_owned(),
            apply: false,
            branch: None,
            backend: None,
            yes: false,
            confirm: Vec::new(),
            by: None,
            format: Format::Text,
        },
        "defaults"
    );
    let Ok(Command::Transform(all)) = parsed(&[
        "transform",
        "repo",
        "--apply",
        "--branch",
        "fixes",
        "--backend",
        "http://192.0.2.10:8080",
        "--yes",
        "--confirm",
        "att-1,,att-2",
        "--confirm",
        "att-3",
        "--by",
        "lead",
        "--format",
        "json",
    ]) else {
        panic!("all");
    };
    assert_eq!(
        all,
        TransformOptions {
            root: "repo".to_owned(),
            apply: true,
            branch: Some("fixes".to_owned()),
            backend: Some("http://192.0.2.10:8080".to_owned()),
            yes: true,
            confirm: vec!["att-1".to_owned(), "att-2".to_owned(), "att-3".to_owned()],
            by: Some("lead".to_owned()),
            format: Format::Json,
        },
        "every option"
    );
    let Ok(Command::Transform(text)) = parsed(&["transform", "--format", "text"]) else {
        panic!("text");
    };
    assert_eq!(text.format, Format::Text, "text");
    for bad in [
        &["transform", "--format", "xml"][..],
        &["transform", "--branch"],
        &["transform", "--backend"],
        &["transform", "--by"],
        &["transform", "--confirm"],
        &["transform", "--format"],
        &["transform", "a", "b"],
        &["transform", "-x"],
        &["transform", "--confirm", "att-1"],
        &["transform", "--by", "lead"],
    ] {
        assert!(parsed(bad).is_err(), "{bad:?}");
    }
}
