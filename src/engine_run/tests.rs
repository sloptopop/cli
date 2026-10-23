//! Unit tests for [`super`], with the committed fake `sloptopop-local` in
//! `tests/fake-local/`.

use std::path::Path;

use super::*;
use crate::tests::support::{FAKE_LOCAL, TempDir, explicit};

#[test]
fn missing_local_is_reported_with_a_hint() {
    let mut stderr = Vec::new();
    let result = invoke(&Locator::default(), &["report", "."], &mut stderr);
    assert_eq!(result.err(), Some(ENGINE_ERROR), "exit 3");
    assert_eq!(String::from_utf8_lossy(&stderr), NOT_FOUND, "hint");
}

#[test]
fn unstartable_local_is_reported() {
    let dir = TempDir::new();
    let mut stderr = Vec::new();
    let result = invoke(
        &explicit(&dir.0.join("nope")),
        &["report", "."],
        &mut stderr,
    );
    assert_eq!(result.err(), Some(ENGINE_ERROR), "exit 3");
    assert!(
        String::from_utf8_lossy(&stderr).starts_with("error: cannot run "),
        "message"
    );
}

#[test]
fn local_outcomes() {
    for (mode, expected) in [
        ("pass", Ok(0)),
        ("fail", Ok(1)),
        ("other", Err(3)),
        ("killed", Err(3)),
    ] {
        let mut stderr = Vec::new();
        let result = invoke(&explicit(Path::new(FAKE_LOCAL)), &[mode], &mut stderr);
        let stderr = String::from_utf8_lossy(&stderr);
        assert!(
            stderr.starts_with(&format!("fake-local: {mode}\n")),
            "{mode}: stderr forwarded"
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
                    stderr.contains("error: sloptopop-local failed ("),
                    "{mode}: {stderr}"
                );
            }
            (other, _) => panic!("{mode}: unexpected {other:?}"),
        }
    }
}
