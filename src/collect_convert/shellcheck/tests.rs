//! Unit tests for [`super`], on what the image's wrapper printed for the
//! synthetic WP-21 probe (`tests/fixtures/fix-probe/sh/`: `é😀` before the
//! fixed site, a tab, CRLF, a byte order mark, a file without a dialect, a
//! `# shellcheck shell=` directive and an `env bash` script without a
//! name), and on doctored reports for every refusal. The golden check: the
//! carried fixes, applied once, give the bytes `shellcheck -f diff` wrote.

use std::fs;

use serde_json::{Value, json};

use crate::collect_convert::defect::{Defect, Rule};
use crate::collect_convert::probe::{applied, document, dropped, probe, results};
use crate::collect_convert::{Converter, convert};
use crate::tests::support::TempDir;

const RULES: &[Rule] = &[
    Rule {
        id: "SC2006",
        class: Defect::ShellExpansion,
        reason: "backquotes",
        off: None,
    },
    Rule {
        id: "SC2034",
        class: Defect::WrongOperator,
        reason: "a rule that is off",
        off: Some("below the precision budget"),
    },
    Rule {
        id: "SC2086",
        class: Defect::ShellExpansion,
        reason: "an unquoted expansion",
        off: None,
    },
    Rule {
        id: "SC2164",
        class: Defect::UncheckedResult,
        reason: "an unchecked cd",
        off: None,
    },
];

fn run(output: &[u8], root: &std::path::Path) -> Value {
    document(
        &convert(
            Converter::ShellcheckJson1(RULES),
            output,
            "shellcheck",
            "0.11.0-1",
            root,
        )
        .unwrap(),
    )
}

/// The probe's report, converted.
fn probe_report() -> Value {
    run(&fs::read(probe("shellcheck.txt")).unwrap(), &probe("sh"))
}

#[test]
fn shellcheck_fixes_reproduce_shellcheck_diff_on_every_probe_file() {
    let root = probe("sh");
    let sarif = probe_report();
    let fixed = applied(&sarif, &root, "safe");
    let expected = [
        "bom.sh",
        "crlf.sh",
        "directive.sh",
        "nodialect.sh",
        "nonascii.sh",
        "plain.sh",
        "tab.sh",
    ];
    for file in expected {
        assert_eq!(
            fixed[file],
            fs::read(probe("expected/shellcheck").join(file)).unwrap(),
            "{file}"
        );
    }
    // `-f diff` was not run on the script without a name; its one fix:
    assert_eq!(
        fixed["envbash"],
        b"#!/usr/bin/env bash\ny=$(echo a); echo \"$y\"\n"
    );
    assert_eq!(fixed.len(), expected.len() + 1);
    assert!(
        applied(&sarif, &root, "unsafe").is_empty(),
        "no unsafe fixes"
    );
    assert!(dropped(&sarif).is_empty(), "nothing dropped");
    assert_eq!(
        sarif["runs"][0]["artifacts"].as_array().map(Vec::len),
        Some(8)
    );
}

#[test]
fn shellcheck_comments_are_results_in_code_points() {
    let sarif = probe_report();
    // Only the reported rules are results: SC1017, SC1082 and SC2116 are
    // not listed, SC2034 is off.
    let found = results(&sarif);
    assert_eq!(found.len(), 17);
    let rules: Vec<&str> = found
        .iter()
        .map(|r| r["ruleId"].as_str().unwrap())
        .collect();
    assert_eq!(rules.iter().filter(|rule| **rule == "SC2086").count(), 8);
    assert_eq!(rules.iter().filter(|rule| **rule == "SC2164").count(), 5);
    assert_eq!(rules.iter().filter(|rule| **rule == "SC2006").count(), 4);
    let nonascii = found
        .iter()
        .find(|r| {
            r["ruleId"] == "SC2164"
                && r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"] == "nonascii.sh"
        })
        .unwrap();
    assert_eq!(
        nonascii["locations"][0]["physicalLocation"]["region"],
        json!({ "startLine": 3, "startColumn": 9 })
    );
    assert_eq!(nonascii["level"], "warning");
    assert_eq!(
        nonascii["message"]["text"],
        "Use 'cd ... || exit' or 'cd ... || return' in case cd fails."
    );
    let carried = &nonascii["fixes"][0];
    assert_eq!(
        carried["properties"],
        json!({
            "sloptopop/applicability": "safe",
            "sloptopop/toolApplicability": "fix",
            "sloptopop/source": "shellcheck-json1",
            "sloptopop/converter": 1,
            "sloptopop/reviewed": true,
        })
    );
    // `z="é😀"; cd /tmp` is 15 code points (the fix's column 16) and 19
    // bytes, and line 3 starts at byte 36: the insertion is at byte 55.
    assert_eq!(
        carried["artifactChanges"][0]["replacements"],
        json!([{ "deletedRegion": { "byteOffset": 55, "byteLength": 0 },
                 "insertedContent": { "text": " || exit" } }])
    );
    let quoted = found.iter().find(|r| r["ruleId"] == "SC2086").unwrap();
    assert_eq!(quoted["level"], "note");
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "shellcheck");
    assert_eq!(
        run["tool"]["driver"]["rules"],
        json!([
            { "id": "SC2006", "properties": { "defectClass": "DC18" } },
            { "id": "SC2086", "properties": { "defectClass": "DC18" } },
            { "id": "SC2164", "properties": { "defectClass": "DC02" } },
        ])
    );
}

/// A wrapper output: the header, then one json1 report holding `comments`.
fn report(comments: &Value) -> Vec<u8> {
    format!(
        "# sloptopop-shellcheck: 1 files\n{}\n",
        json!({ "comments": comments })
    )
    .into_bytes()
}

/// A comment of `rule` on `file` at line 1, with `replacements` (a `null`
/// fix when there are none).
fn comment(rule: u64, level: &str, replacements: &[(u64, u64, &str)]) -> Value {
    let fix = if replacements.is_empty() {
        Value::Null
    } else {
        json!({ "replacements": replacements.iter().map(|(column, end, text)| json!({
            "line": 1, "column": column, "endLine": 1, "endColumn": end,
            "insertionPoint": "afterEnd", "precedence": 7, "replacement": text,
        })).collect::<Vec<_>>() })
    };
    json!({ "file": "a.sh", "line": 1, "endLine": 1, "column": 1, "endColumn": 2,
            "level": level, "code": rule, "message": "m", "fix": fix })
}

fn repository(text: &str) -> TempDir {
    let root = TempDir::new();
    fs::write(root.0.join("a.sh"), text).unwrap();
    root
}

#[test]
fn backquotes_holding_a_backslash_are_an_unsafe_fix() {
    let root = repository("y=`printf \\\\n`; z=`date`\n");
    let sarif = run(
        &report(&json!([
            comment(2006, "style", &[(3, 4, "$("), (14, 15, ")")]),
            comment(2006, "style", &[(19, 20, "$("), (24, 25, ")")]),
        ])),
        &root.0,
    );
    let found = results(&sarif);
    let mut levels: Vec<(&str, bool)> = found
        .iter()
        .map(|r| {
            let properties = &r["fixes"][0]["properties"];
            (
                properties["sloptopop/applicability"].as_str().unwrap(),
                properties["sloptopop/reviewed"].as_bool().unwrap(),
            )
        })
        .collect();
    levels.sort_unstable();
    assert_eq!(levels, [("safe", true), ("unsafe", false)]);
    assert_eq!(
        applied(&sarif, &root.0, "unsafe")["a.sh"],
        b"y=$(printf \\\\n); z=`date`\n"
    );
    assert_eq!(
        applied(&sarif, &root.0, "safe")["a.sh"],
        b"y=`printf \\\\n`; z=$(date)\n"
    );
    // Only SC2006 changes what a backslash means.
    let other = run(
        &report(&json!([comment(
            2086,
            "info",
            &[(3, 3, "\""), (13, 13, "\"")]
        )])),
        &root.0,
    );
    assert_eq!(
        results(&other)[0]["fixes"][0]["properties"]["sloptopop/applicability"],
        "safe"
    );
}

#[test]
fn insertions_touching_another_edit_drop_the_fix() {
    let root = repository("echo $a$b\n");
    let overlap = vec![(
        "SC2086".to_owned(),
        "a.sh".to_owned(),
        "fix.overlap-within".to_owned(),
    )];
    for replacements in [
        // At the start of a replaced span, then at its end.
        &[(6, 6, "\""), (6, 8, "${a}")][..],
        &[(6, 8, "${a}"), (8, 8, "\"")][..],
    ] {
        let sarif = run(
            &report(&json!([comment(2086, "info", replacements)])),
            &root.0,
        );
        assert_eq!(dropped(&sarif), overlap, "{replacements:?}");
        assert_eq!(results(&sarif)[0].get("fixes"), None);
    }
    // Adjacent replacements are no insertions, and separate insertions
    // touch nothing.
    for replacements in [
        &[(6, 8, "${a}"), (8, 10, "${b}")][..],
        &[(6, 6, "\""), (10, 10, "\"")][..],
    ] {
        let sarif = run(
            &report(&json!([comment(2086, "info", replacements)])),
            &root.0,
        );
        assert!(dropped(&sarif).is_empty(), "{replacements:?}");
        assert_eq!(
            results(&sarif)[0]["fixes"].as_array().map(Vec::len),
            Some(1)
        );
    }
}

#[test]
fn levels_unfixed_comments_and_unconvertible_places() {
    let root = repository("cd /tmp\n");
    let sarif = run(
        &report(&json!([
            comment(2164, "error", &[]),
            comment(2164, "warning", &[(8, 8, " || exit")]),
            comment(2086, "style", &[(9, 10, "x")]),
            comment(2006, "info", &[(1, 1, "x"), (1, 12, "y")]),
            comment(2034, "warning", &[(1, 1, "x")]),
        ])),
        &root.0,
    );
    let found: Vec<(String, String, usize)> = results(&sarif)
        .iter()
        .map(|r| {
            (
                r["ruleId"].as_str().unwrap().to_owned(),
                r["level"].as_str().unwrap().to_owned(),
                r["fixes"].as_array().map_or(0, Vec::len),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            ("SC2006".to_owned(), "note".to_owned(), 0),
            ("SC2086".to_owned(), "note".to_owned(), 0),
            ("SC2164".to_owned(), "error".to_owned(), 0),
            ("SC2164".to_owned(), "warning".to_owned(), 1),
        ]
    );
    assert_eq!(
        dropped(&sarif),
        [
            (
                "SC2006".to_owned(),
                "a.sh".to_owned(),
                "fix.out-of-range".to_owned()
            ),
            (
                "SC2086".to_owned(),
                "a.sh".to_owned(),
                "fix.out-of-range".to_owned()
            ),
        ]
    );
    // A file that is not there.
    let elsewhere = json!([{ "file": "gone.sh", "line": 1, "column": 1, "level": "info",
        "code": 2086, "message": "m", "fix": { "replacements": [
            { "line": 1, "column": 1, "endLine": 1, "endColumn": 1, "replacement": "x" }] } }]);
    assert_eq!(
        dropped(&run(&report(&elsewhere), &root.0)),
        [(
            "SC2086".to_owned(),
            "gone.sh".to_owned(),
            "fix.path".to_owned()
        )]
    );
}

#[test]
fn the_wrapper_output_is_read_whole_or_refused() {
    let root = repository("");
    for empty in [
        &b"# sloptopop-shellcheck: 0 files\n"[..],
        b"# sloptopop-shellcheck: 0 files",
        b"",
    ] {
        assert_eq!(results(&run(empty, &root.0)), Vec::<Value>::new());
    }
    // Without the header, reports are read as they come, the first too.
    let bare = format!(
        "{}\n{}\n",
        json!({ "comments": [comment(2164, "warning", &[])] }),
        json!({ "comments": [comment(2164, "error", &[])] })
    );
    assert_eq!(results(&run(bare.as_bytes(), &root.0)).len(), 2);
    let convert = |raw: &[u8]| {
        convert(
            Converter::ShellcheckJson1(RULES),
            raw,
            "shellcheck",
            "v",
            &root.0,
        )
        .unwrap_err()
    };
    assert!(
        convert(b"# sloptopop-shellcheck: 1 files\nnot json")
            .starts_with("ShellCheck printed no readable json1: ")
    );
    assert_eq!(
        convert(b"{\"comments\": 1}"),
        "ShellCheck json1: no comments array"
    );
}
