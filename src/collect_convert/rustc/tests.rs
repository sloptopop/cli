//! Unit tests for [`super`], on what `cargo clippy --message-format=json`
//! printed for the synthetic WP-21 probe (`tests/fixtures/fix-probe/rs/`:
//! `é😀` before the fixed site, CRLF and a byte order mark), and on
//! doctored diagnostics for every refusal. The golden check: the carried
//! fixes, applied once, give the bytes `cargo clippy --fix` wrote.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::collect_convert::probe::{applied, document, dropped, probe, results};
use crate::collect_convert::{Converter, convert};
use crate::tests::support::TempDir;

fn run(raw: &[u8], root: &Path) -> Value {
    document(&convert(Converter::RustcJson, raw, "clippy", "1.99.0", root).unwrap())
}

fn owned(list: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    list.iter()
        .map(|(a, b, c)| ((*a).to_owned(), (*b).to_owned(), (*c).to_owned()))
        .collect()
}

#[test]
fn clippy_fixes_reproduce_cargo_clippy_fix_on_every_probe_file() {
    let root = probe("rs");
    let sarif = run(&fs::read(probe("clippy.json")).unwrap(), &root);
    let fixed = applied(&sarif, &root, "safe");
    assert_eq!(fixed.len(), 4);
    for name in ["bom", "crlf", "nonascii", "plain"] {
        let file = format!("src/bin/{name}.rs");
        assert_eq!(
            fixed[&file],
            fs::read(probe("expected/clippy").join(&file)).unwrap(),
            "{file}"
        );
    }
    assert!(dropped(&sarif).is_empty(), "nothing dropped");
    let found = results(&sarif);
    assert_eq!(found.len(), 12);
    assert!(found.iter().all(|r| r["level"] == "warning"));
    assert!(
        found
            .iter()
            .all(|r| r["fixes"].as_array().map(Vec::len) == Some(1))
    );
    let mutable = found
        .iter()
        .find(|r| {
            r["ruleId"] == "unused_mut"
                && r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                    == "src/bin/nonascii.rs"
        })
        .unwrap();
    assert_eq!(
        mutable["locations"][0]["physicalLocation"]["region"],
        json!({ "startLine": 5, "startColumn": 24 })
    );
    assert_eq!(
        mutable["message"]["text"],
        "variable does not need to be mutable"
    );
    assert_eq!(
        mutable["fixes"][0],
        json!({
            "description": { "text": "remove this `mut`" },
            "artifactChanges": [{
                "artifactLocation": { "uri": "src/bin/nonascii.rs" },
                "replacements": [{ "deletedRegion": { "byteOffset": 71, "byteLength": 4 } }],
            }],
            "properties": {
                "sloptopop/applicability": "safe",
                "sloptopop/toolApplicability": "MachineApplicable",
                "sloptopop/source": "rustc-json",
                "sloptopop/converter": 1,
                "sloptopop/reviewed": false,
            },
        })
    );
    // `needless_return`: one suggestion, two adjacent spans.
    let needless = found
        .iter()
        .find(|r| r["ruleId"] == "clippy::needless_return")
        .unwrap();
    assert_eq!(
        needless["fixes"][0]["artifactChanges"][0]["replacements"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
}

/// A span of `file` (bytes `start..end`), suggesting `replacement` when
/// there is one.
fn span(file: &str, start: u64, end: u64, replacement: Option<(&str, &str)>) -> Value {
    let mut span = json!({ "file_name": file, "byte_start": start, "byte_end": end,
        "line_start": 1, "column_start": start + 1, "is_primary": true,
        "suggested_replacement": null, "suggestion_applicability": null });
    if let Some((text, applicability)) = replacement {
        span["suggested_replacement"] = json!(text);
        span["suggestion_applicability"] = json!(applicability);
    }
    span
}

/// A Cargo message holding a diagnostic of `code` and `level` on `a.rs`,
/// with one child per entry of `children`.
fn message(code: Option<&str>, level: &str, children: &[Vec<Value>]) -> String {
    let children: Vec<Value> = children
        .iter()
        .map(
            |spans| json!({ "message": "fix it", "level": "help", "spans": spans, "children": [] }),
        )
        .collect();
    json!({ "reason": "compiler-message", "message": {
        "message": "the message", "level": level,
        "code": code.map(|code| json!({ "code": code, "explanation": null })),
        "spans": [span("a.rs", 3, 4, None)], "children": children } })
    .to_string()
}

fn repository() -> TempDir {
    let root = TempDir::new();
    fs::write(root.0.join("a.rs"), "fn é() {}\n").unwrap();
    root
}

/// The children of a diagnostic and the drops they make.
type Case<'a> = (Vec<Vec<Value>>, &'a [(&'a str, &'a str, &'a str)]);

#[test]
fn only_machine_applicable_single_file_suggestions_are_carried() {
    let root = repository();
    let machine = |start, end, text| span("a.rs", start, end, Some((text, "MachineApplicable")));
    let cases: [Case<'_>; 7] = [
        (
            vec![vec![span("a.rs", 3, 5, Some(("e", "MaybeIncorrect")))]],
            &[("c", "a.rs", "fix.unconverted")],
        ),
        (
            vec![vec![
                machine(3, 5, "e"),
                span("a.rs", 6, 6, Some((" ", "HasPlaceholders"))),
            ]],
            &[("c", "a.rs", "fix.unconverted")],
        ),
        (
            vec![vec![
                machine(3, 5, "e"),
                span("b.rs", 0, 0, Some(("x", "MachineApplicable"))),
            ]],
            &[("c", "a.rs", "fix.multi-file")],
        ),
        (
            vec![vec![machine(3, 5, "e")], vec![machine(0, 2, "pub fn")]],
            &[("c", "a.rs", "fix.alternatives")],
        ),
        (
            vec![vec![machine(3, 4, "e")]],
            &[("c", "a.rs", "fix.boundary")],
        ),
        (
            vec![vec![machine(3, 99, "e")]],
            &[("c", "a.rs", "fix.out-of-range")],
        ),
        (
            vec![vec![machine(98, 99, "e")]],
            &[("c", "a.rs", "fix.out-of-range")],
        ),
    ];
    for (children, drops) in cases {
        let sarif = run(message(Some("c"), "warning", &children).as_bytes(), &root.0);
        assert_eq!(dropped(&sarif), owned(drops), "{children:?}");
        assert_eq!(results(&sarif)[0].get("fixes"), None);
    }
    // One machine-applicable suggestion beside a child with no suggestion.
    let sarif = run(
        message(
            Some("c"),
            "warning",
            &[vec![span("a.rs", 0, 2, None)], vec![machine(3, 5, "e")]],
        )
        .as_bytes(),
        &root.0,
    );
    assert!(dropped(&sarif).is_empty(), "nothing dropped");
    assert_eq!(applied(&sarif, &root.0, "safe")["a.rs"], b"fn e() {}\n");
    assert_eq!(
        results(&sarif)[0]["fixes"][0]["description"]["text"],
        "fix it"
    );
}

#[test]
fn levels_codes_and_the_lines_that_are_no_diagnostics() {
    let root = repository();
    let mut raw = String::new();
    for level in [
        "error",
        "error: internal compiler error",
        "warning",
        "note",
        "help",
    ] {
        raw.push_str(&message(Some(level), level, &[]));
        raw.push('\n');
    }
    raw.push_str(&message(None, "error", &[]));
    raw.push_str("\n\n{\"reason\": \"compiler-artifact\", \"spans\": []}\n");
    raw.push_str("{\"reason\": \"build-finished\", \"success\": true}\n");
    // rustc's own `--error-format=json`: the diagnostic itself, no reason.
    raw.push_str(
        &json!({ "message": "raw", "level": "warning", "code": { "code": "raw" },
                 "spans": [], "children": [] })
        .to_string(),
    );
    raw.push_str("\n{\"other\": 1}\n");
    let sarif = run(raw.as_bytes(), &root.0);
    let levels: Vec<(String, String)> = results(&sarif)
        .iter()
        .map(|r| {
            (
                r["ruleId"].as_str().unwrap().to_owned(),
                r["level"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expected = [
        ("raw", "warning"),
        ("error", "error"),
        ("error: internal compiler error", "error"),
        ("help", "note"),
        ("note", "note"),
        ("warning", "warning"),
    ];
    assert_eq!(
        levels,
        expected
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect::<Vec<_>>()
    );
    // The raw diagnostic has no primary span: no location.
    assert!(results(&sarif)[0].get("locations").is_none());
    assert_eq!(
        results(&sarif)[1]["locations"][0]["physicalLocation"]["region"],
        json!({ "startLine": 1, "startColumn": 4 })
    );
    let error = convert(
        Converter::RustcJson,
        b"\n{}\nnot json\n",
        "clippy",
        "v",
        &root.0,
    )
    .unwrap_err();
    assert!(
        error.starts_with("rustc printed no readable JSON on line 3: "),
        "{error}"
    );
}
