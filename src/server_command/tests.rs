//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use std::ffi::OsString;
use std::fs;

use super::*;
use crate::tests::support::{FAKE_LOCAL, TempDir, fake_local};

/// A repository directory `name` in `dir`.
fn repo(dir: &TempDir, name: &str) -> String {
    let path = dir.0.join(name);
    fs::create_dir_all(&path).unwrap();
    path.to_string_lossy().into_owned()
}

/// The exchange for `root` with `backend`, `yes` and `stdin`; its result
/// and stderr.
fn run(
    locator: &Locator,
    root: &str,
    backend: &str,
    yes: bool,
    stdin: &[u8],
) -> (Result<Answered, u8>, String) {
    let mut stderr = Vec::new();
    let asked = Asked {
        root,
        mode: "transform",
        backend: Some(backend),
        yes,
    };
    let result = exchange(locator, asked, &mut &stdin[..], &mut stderr);
    (result, String::from_utf8_lossy(&stderr).into_owned())
}

#[test]
fn the_request_is_shown_confirmed_and_posted() {
    let dir = TempDir::new();
    let root = repo(&dir, "plain");
    let (answered, stderr) = run(&fake_local(), &root, "http://ok.test", false, b"yes\n");
    let answered = answered.unwrap();
    assert_eq!(answered.run, "0123456789abcdef", "the run");
    assert!(
        answered
            .response
            .ends_with(".sloptopop/transform/0123456789abcdef/response.json"),
        "the response"
    );
    assert_eq!(
        fs::read_to_string(&answered.response).unwrap(),
        r#"{"answered":true}"#,
        "written by curl"
    );
    assert!(
        stderr.contains("sloptopop transform will upload to http://ok.test/v1/transform:"),
        "{stderr}"
    );
    assert!(stderr.contains("Upload? [y/N] "), "asked");
    let posted = format!("{root}/.sloptopop/transform/0123456789abcdef/request.json.posted");
    assert!(
        fs::read_to_string(posted)
            .unwrap()
            .contains("\"mode\":\"transform\""),
        "the request as prepared"
    );
}

#[test]
fn nothing_goes_without_a_yes_curl_or_a_server() {
    let dir = TempDir::new();
    let root = repo(&dir, "plain");
    let (declined, stderr) = run(&fake_local(), &root, "http://ok.test", false, b"no\n");
    assert_eq!(declined, Err(UPLOAD_ERROR), "declined");
    assert!(stderr.contains("the upload was not confirmed"), "{stderr}");
    let no_curl = Locator {
        explicit: Some(OsString::from(FAKE_LOCAL)),
        ..Locator::default()
    };
    let (missing, told) = run(&no_curl, &root, "http://ok.test", true, b"");
    assert_eq!(missing, Err(UPLOAD_ERROR), "no curl");
    assert!(told.contains("curl not found on PATH"), "{told}");
    let (refused, said) = run(&fake_local(), &root, "http://refuses.test", true, b"");
    assert_eq!(refused, Err(UPLOAD_ERROR), "refused");
    assert!(
        said.contains("every slot is taken")
            && said.contains("hint: sloptopop detects only on its server"),
        "{stderr}"
    );
    let (garbage, _) = run(
        &fake_local(),
        &repo(&dir, "garbage"),
        "http://ok.test",
        true,
        b"",
    );
    assert_eq!(garbage, Err(3), "not a document");
    let (broken, _) = run(
        &fake_local(),
        &repo(&dir, "broken"),
        "http://ok.test",
        true,
        b"",
    );
    assert_eq!(broken, Err(3), "a failure");
    let mut sink = Vec::new();
    let default = exchange(
        &fake_local(),
        Asked {
            root: &root,
            mode: "check",
            backend: None,
            yes: true,
        },
        &mut &b""[..],
        &mut sink,
    );
    assert_eq!(
        default,
        Err(UPLOAD_ERROR),
        "the hosted default is not reachable from the fake"
    );
    assert!(
        String::from_utf8_lossy(&sink).contains("https://api.sloptopop.dev/v1/transform"),
        "the default server"
    );
}
