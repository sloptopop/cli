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
