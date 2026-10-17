//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use std::fs;
use std::path::Path;

use super::{USER_AGENT, endpoint, post};
use crate::tests::support::TempDir;

const CURL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-curl/curl");

#[test]
fn the_endpoint_is_under_the_base_url() {
    assert_eq!(
        endpoint("https://b.example/"),
        "https://b.example/v1/transform",
        "slash"
    );
    assert_eq!(
        endpoint("http://192.0.2.10:8080"),
        "http://192.0.2.10:8080/v1/transform",
        "none"
    );
    assert_eq!(
        USER_AGENT,
        concat!("sloptopop/", env!("CARGO_PKG_VERSION")),
        "generic"
    );
}

#[test]
fn a_post_writes_the_answer_or_says_why_not() {
    let dir = TempDir::new();
    let request = dir.0.join("request.json");
    let response = dir.0.join("response.json");
    fs::write(&request, "{}").unwrap();
    post(
        Path::new(CURL),
        "http://ok/v1/transform",
        &request,
        &response,
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&response).unwrap(),
        r#"{"answered":true}"#,
        "answered"
    );
    assert_eq!(
        fs::read_to_string(dir.0.join("request.json.posted")).unwrap(),
        "{}",
        "posted"
    );
    let refused = post(
        Path::new(CURL),
        "http://refuses/v1/transform",
        &request,
        &response,
    )
    .unwrap_err();
    assert!(
        refused.contains("(22)") && refused.contains("every slot is taken"),
        "{refused}"
    );
    fs::remove_file(&response).unwrap();
    let down = post(
        Path::new(CURL),
        "http://down/v1/transform",
        &request,
        &response,
    )
    .unwrap_err();
    assert!(down.ends_with("curl: (7) Failed to connect"), "{down}");
    let missing = post(
        Path::new("/nonexistent/curl"),
        "http://ok",
        &request,
        &response,
    )
    .unwrap_err();
    assert!(
        missing.starts_with("cannot run /nonexistent/curl"),
        "{missing}"
    );
}
