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
        source: Source::ContainerFlag,
        only: vec![
            "ruff".to_owned(),
            "gitleaks".to_owned(),
            "bandit".to_owned(),
        ],
        rules: Some("rules".to_owned()),
        dry_run: true,
        offline: true,
        registry: Some("mirror.example:5000/sloptopop".to_owned()),
        runtime: Some("podman".to_owned()),
        timeout: Some(60),
        vuln_db: Some("20261020".to_owned()),
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
            "--offline",
            "--registry",
            "mirror.example:5000/sloptopop/",
            "--runtime",
            "podman",
            "--timeout",
            "60",
            "--vuln-db",
            "20261020",
        ][..],
        &[
            "--vuln-db",
            "20261020",
            "--timeout",
            "60",
            "--runtime",
            "podman",
            "--registry",
            "mirror.example:5000/sloptopop",
            "--offline",
            "--dry-run",
            "--only",
            "ruff",
            "--rules",
            "elsewhere",
            "--container",
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
    let Ok(Command::Collect(host)) = collect(&["--host-tools", "--host-tools"]) else {
        panic!("host tools");
    };
    assert_eq!(host.source, Source::HostTools);
}

#[test]
fn collect_rejects_bad_arguments() {
    for (args, message) in [
        (&["--only"][..], "--only needs a value"),
        (&["--only", "--dry-run"], "--only needs a value"),
        (&["--rules"], "--rules needs a value"),
        (&["--registry"], "--registry needs a value"),
        (&["--runtime"], "--runtime needs a value"),
        (&["--timeout"], "--timeout needs a value"),
        (&["--vuln-db"], "--vuln-db needs a value"),
        (&["--only", "ruff,,bandit"], "--only has an empty tool name"),
        (&["--force"], "unrecognised option: --force"),
        (&["a", "b"], "unrecognised option or argument: b"),
        (
            &["a", "--force"],
            "unrecognised option or argument: --force",
        ),
        (
            &["--host-tools", "--container"],
            "--host-tools and --container exclude each other",
        ),
        (
            &["--container", "--host-tools"],
            "--host-tools and --container exclude each other",
        ),
        (
            &["--runtime", "nerdctl"],
            "unknown runtime: nerdctl (expected docker or podman)",
        ),
        (
            &["--timeout", "0"],
            "--timeout needs a whole number of seconds, not 0",
        ),
        (
            &["--timeout", "soon"],
            "--timeout needs a whole number of seconds, not soon",
        ),
        (
            &["--vuln-db", "2026-10-20"],
            "--vuln-db needs a date as YYYYMMDD, not 2026-10-20",
        ),
        (
            &["--vuln-db", "2026102x"],
            "--vuln-db needs a date as YYYYMMDD, not 2026102x",
        ),
        (
            &["--vuln-db", "202610201"],
            "--vuln-db needs a date as YYYYMMDD, not 202610201",
        ),
    ] {
        assert_eq!(collect(args), Err(message.to_owned()), "{args:?}");
    }
}

#[test]
fn registries_are_hosts_with_an_optional_port_and_path() {
    for (value, expected) in [
        ("registry.example", "registry.example"),
        ("registry.example/", "registry.example"),
        ("127.0.0.1:5000", "127.0.0.1:5000"),
        (
            "harbor.corp/proxy_cache/sloptopop",
            "harbor.corp/proxy_cache/sloptopop",
        ),
    ] {
        assert_eq!(registry(value), Ok(expected.to_owned()), "{value}");
    }
    for bad in [
        "",
        "/",
        "https://registry.example",
        "/abs",
        "white space",
        "a@b",
    ] {
        assert!(
            registry(bad)
                .unwrap_err()
                .starts_with(&format!("not a registry: {bad} (")),
            "{bad}"
        );
    }
    assert!(
        collect(&["--registry", "a@b"])
            .unwrap_err()
            .starts_with("not a registry: a@b (")
    );
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
