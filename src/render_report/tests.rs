//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::json;

use super::*;

#[test]
fn report_groups_findings_by_severity() {
    let document = json!({
        "schemaVersion": 1,
        "ecosystems": ["python", "rust"],
        "findings": [
            {"rule": "a.warn", "severity": "warning", "path": "src/a.py", "line": 3, "message": "W", "remediation": "Do W."},
            {"rule": "b.err", "severity": "error", "path": "Cargo.toml", "message": "E", "control": "STD-1",
             "fix": {"kind": "createFile", "path": "rust-toolchain.toml", "template": "t"}, "remediation": "Do E."},
            {"rule": "c.note", "severity": "note", "path": "x", "message": "N"}
        ],
        "controls": [
            {"id": "STD-1", "title": "Pinned toolchain", "status": "fail"},
            {"id": "STD-2", "title": "Formatting", "status": "notChecked"}
        ],
        "passed": false
    });
    assert_eq!(
        report(&document),
        "\
sloptopop check: FAILED (3 findings; ecosystems: python, rust)

Errors (1)
  Cargo.toml  b.err  [STD-1]
    E
    fix: `sloptopop fix --apply` creates rust-toolchain.toml
    how: Do E.

Warnings (1)
  src/a.py:3  a.warn
    W
    how: Do W.

note (1)
  x  c.note
    N

Controls
  STD-1   fail        Pinned toolchain
  STD-2   notChecked  Formatting
",
        "text"
    );
}

#[test]
fn a_passing_report_is_short() {
    let document = json!({"findings": [], "ecosystems": [], "controls": [], "passed": true});
    assert_eq!(
        report(&document),
        "sloptopop check: passed (0 findings; ecosystems: none)\n",
        "passed"
    );
    let one = json!({"findings": [{"severity": "error"}], "passed": false});
    assert!(
        report(&one).starts_with("sloptopop check: FAILED (1 finding;"),
        "singular"
    );
    assert!(
        report(&json!(null)).starts_with("sloptopop check: FAILED (0 findings"),
        "not a report"
    );
}
