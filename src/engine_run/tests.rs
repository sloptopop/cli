//! Unit tests for [`super`], with the committed fake engine in
//! `tests/fake-engine/` standing in for the real one.

use std::path::Path;

use super::*;
use crate::tests::support::{FAKE_ENGINE, TempDir, explicit};

#[test]
fn missing_engine_is_reported_with_a_hint() {
    let mut stderr = Vec::new();
    let result = invoke(&Locator::default(), &["check", "."], &mut stderr);
    assert_eq!(result.err(), Some(ENGINE_ERROR), "exit 3");
    assert_eq!(String::from_utf8_lossy(&stderr), NOT_FOUND, "hint");
}

#[test]
fn unstartable_engine_is_reported() {
    let dir = TempDir::new();
    let mut stderr = Vec::new();
    let result = invoke(&explicit(&dir.0.join("nope")), &["check", "."], &mut stderr);
    assert_eq!(result.err(), Some(ENGINE_ERROR), "exit 3");
    assert!(
        String::from_utf8_lossy(&stderr).starts_with("error: cannot run "),
        "message"
    );
}

#[test]
fn engine_outcomes() {
    for (mode, expected) in [
        ("pass", Ok(0)),
        ("fail", Ok(1)),
        ("usage-error", Err(3)),
        ("killed", Err(3)),
    ] {
        let mut stderr = Vec::new();
        let result = invoke(
            &explicit(Path::new(FAKE_ENGINE)),
            &["check", mode],
            &mut stderr,
        );
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            stderr.starts_with(&format!("fake-engine: check {mode}\n")),
            "{mode}: engine stderr forwarded with the arguments"
        );
        match (result, expected) {
            (Ok((output, code)), Ok(want)) => {
                assert_eq!(code, want, "{mode}");
                assert!(
                    output.stdout.starts_with(b"{"),
                    "{mode}: document on stdout"
                );
            }
            (Err(code), Err(want)) => {
                assert_eq!(code, want, "{mode}");
                assert!(
                    stderr.contains("error: sloptopop-engine failed ("),
                    "{mode}: {stderr}"
                );
            }
            (other, _) => panic!("{mode}: unexpected {other:?}"),
        }
    }
}
