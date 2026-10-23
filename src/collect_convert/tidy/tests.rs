//! Unit tests for [`super`], on what the image's wrapper printed for the
//! synthetic WP-21 probe (`tests/fixtures/fix-probe/c/`: `é😀` before the
//! fixed site, CRLF and a byte order mark), and on doctored exports for
//! every refusal. The golden check: the carried fixes, applied once, give
//! the bytes `clang-tidy --fix` wrote with the one reviewed check.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::MARKER;
use crate::collect_convert::defect::{Defect, Rule};
use crate::collect_convert::probe::{applied, document, dropped, probe, results};
use crate::collect_convert::{Converter, convert};
use crate::tests::support::TempDir;

const STRLEN: &str = "bugprone-misplaced-operator-in-strlen-in-alloc";
const COMPARE: &str = "bugprone-suspicious-string-compare";
const WIDENING: &str = "bugprone-implicit-widening-of-multiplication-result";

const RULES: &[Rule] = &[
    Rule {
        id: STRLEN,
        class: Defect::OutOfBounds,
        reason: "reviewed",
        off: None,
    },
    Rule {
        id: COMPARE,
        class: Defect::WrongOperator,
        reason: "reported, its fix not reviewed",
        off: None,
    },
    Rule {
        id: WIDENING,
        class: Defect::Overflow,
        reason: "reported, its fix not reviewed",
        off: None,
    },
    Rule {
        id: "bugprone-off",
        class: Defect::Overflow,
        reason: "off",
        off: Some("below the precision budget"),
    },
];

fn run(raw: &[u8], root: &Path) -> Value {
    document(
        &convert(
            Converter::ClangTidy(RULES),
            raw,
            "clang-tidy",
            "19.1.0-3",
            root,
        )
        .unwrap(),
    )
}

fn owned(list: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    list.iter()
        .map(|(a, b, c)| ((*a).to_owned(), (*b).to_owned(), (*c).to_owned()))
        .collect()
}

#[test]
fn clang_tidy_fixes_reproduce_clang_tidy_fix_on_every_probe_file() {
    let root = probe("c");
    let sarif = run(&fs::read(probe("clang-tidy.txt")).unwrap(), &root);
    let fixed = applied(&sarif, &root, "safe");
    assert_eq!(fixed.len(), 4);
    for file in ["bom.c", "crlf.c", "nonascii.c", "plain.c"] {
        assert_eq!(
            fixed[file],
            fs::read(probe("expected/clang-tidy").join(file)).unwrap(),
            "{file}"
        );
    }
    assert!(
        applied(&sarif, &root, "unsafe").is_empty(),
        "no unsafe fixes"
    );
    let mut unreviewed = Vec::new();
    for rule in [WIDENING, COMPARE] {
        for file in ["bom.c", "crlf.c", "nonascii.c", "plain.c"] {
            unreviewed.push((rule, file, "fix.unreviewed"));
        }
    }
    assert_eq!(dropped(&sarif), owned(&unreviewed));
    let found = results(&sarif);
    assert_eq!(found.len(), 12);
    let fixed_results: Vec<&Value> = found
        .iter()
        .filter(|r| r["fixes"].as_array().is_some_and(|fixes| !fixes.is_empty()))
        .collect();
    assert_eq!(fixed_results.len(), 4);
    let bom = fixed_results
        .iter()
        .find(|r| r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"] == "bom.c")
        .unwrap();
    assert_eq!(bom["ruleId"], STRLEN);
    assert_eq!(
        bom["locations"][0]["physicalLocation"]["region"],
        json!({ "startLine": 4, "startColumn": 15 })
    );
    assert_eq!(
        bom["fixes"][0],
        json!({
            "description": {
                "text": "addition operator is applied to the argument of strlen instead of its result"
            },
            "artifactChanges": [{
                "artifactLocation": { "uri": "bom.c" },
                "replacements": [{
                    "deletedRegion": { "byteOffset": 91, "byteLength": 13 },
                    "insertedContent": { "text": "strlen(s) + 1" },
                }],
            }],
            "properties": {
                "sloptopop/applicability": "safe",
                "sloptopop/toolApplicability": "warning",
                "sloptopop/source": "clang-tidy-yaml",
                "sloptopop/converter": 1,
                "sloptopop/reviewed": true,
            },
        })
    );
    assert_eq!(
        sarif["runs"][0]["artifacts"].as_array().map(Vec::len),
        Some(4)
    );
}

/// One exported replacement: file, offset, length, text.
type Replacement<'a> = (&'a str, u64, u64, &'a str);

/// A note's message and replacements.
type Note<'a> = (&'a str, &'a [Replacement<'a>]);

/// The replacements of a message (`at` 8) or a note (`at` 10).
fn replacements(at: usize, list: &[Replacement<'_>]) -> String {
    if list.is_empty() {
        return " []".to_owned();
    }
    let (pad, key) = (" ".repeat(at), " ".repeat(at + 2));
    let mut out = String::new();
    for (file, offset, length, text) in list {
        write!(
            out,
            "\n{pad}- FilePath:        '{file}'\n{key}Offset:          {offset}\n\
             {key}Length:          {length}\n{key}ReplacementText: '{text}'"
        )
        .unwrap();
    }
    out
}

/// One exported diagnostic of `rule` at `offset` of `file`.
fn diagnostic(
    rule: &str,
    level: &str,
    file: &str,
    offset: u64,
    fixes: &[Replacement<'_>],
    notes: &[Note<'_>],
) -> String {
    let mut listed = String::new();
    if !notes.is_empty() {
        listed.push_str("    Notes:\n");
    }
    for (message, note_fixes) in notes {
        write!(
            listed,
            "      - Message:         '{message}'\n        FilePath:        '{file}'\n        \
             FileOffset:      {offset}\n        Replacements:{}\n",
            replacements(10, note_fixes)
        )
        .unwrap();
    }
    format!(
        "  - DiagnosticName:  {rule}\n    DiagnosticMessage:\n      Message:         'the message'\n      \
         FilePath:        '{file}'\n      FileOffset:      {offset}\n      Replacements:{}\n{listed}    \
         Level:           {level}\n    BuildDirectory:  '/src'\n",
        replacements(8, fixes)
    )
}

/// The wrapper's output: the findings `printed`, then the export of
/// `diagnostics`.
fn output(printed: &[&str], diagnostics: &[String]) -> Vec<u8> {
    let mut out = "# sloptopop-clang-tidy: compile database ./compile_commands.json\n".to_owned();
    for line in printed {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(MARKER);
    out.push_str("---\nMainSourceFile:  '/src/a.c'\nDiagnostics:\n");
    for diagnostic in diagnostics {
        out.push_str(diagnostic);
    }
    out.push_str("...\n");
    out.into_bytes()
}

fn repository() -> TempDir {
    let root = TempDir::new();
    // Byte 8 is line 2, column 2.
    fs::write(root.0.join("a.c"), "abcdef\nghij\n").unwrap();
    root
}

fn printed(rule: &str) -> String {
    format!("/src/a.c:2:2: warning: the message [{rule}]")
}

#[test]
fn notes_are_unsafe_alternatives_beside_the_warning_fix() {
    let root = repository();
    let line = printed(STRLEN);
    let both = run(
        &output(
            &[&line],
            &[diagnostic(
                STRLEN,
                "Warning",
                "/src/a.c",
                8,
                &[("/src/a.c", 8, 1, "H")],
                &[("use a wider type", &[("/src/a.c", 0, 0, "x")])],
            )],
        ),
        &root.0,
    );
    let fixes = results(&both)[0]["fixes"].clone();
    assert_eq!(fixes.as_array().map(Vec::len), Some(2));
    assert_eq!(fixes[1]["description"]["text"], "use a wider type");
    assert_eq!(
        fixes[1]["properties"]["sloptopop/toolApplicability"],
        "note"
    );
    assert_eq!(fixes[1]["properties"]["sloptopop/reviewed"], false);
    assert_eq!(applied(&both, &root.0, "safe")["a.c"], b"abcdef\ngHij\n");
    assert_eq!(applied(&both, &root.0, "unsafe")["a.c"], b"xabcdef\nghij\n");
    let two = run(
        &output(
            &[&line],
            &[diagnostic(
                STRLEN,
                "Warning",
                "/src/a.c",
                8,
                &[("/src/a.c", 8, 1, "H")],
                &[
                    ("one", &[("/src/a.c", 0, 0, "x")]),
                    ("two", &[("/src/a.c", 0, 0, "y")]),
                ],
            )],
        ),
        &root.0,
    );
    assert_eq!(dropped(&two), owned(&[(STRLEN, "a.c", "fix.alternatives")]));
    assert_eq!(
        results(&two)[0]["fixes"][0]["properties"]["sloptopop/applicability"],
        "safe"
    );
    assert_eq!(results(&two)[0]["fixes"].as_array().map(Vec::len), Some(1));
    // A note alone, under the host checkout's path.
    let host = format!("{}/a.c", root.0.display());
    let alone = run(
        &output(
            &[&line],
            &[diagnostic(
                STRLEN,
                "Warning",
                &host,
                8,
                &[],
                &[("only", &[(&host, 2, 1, "C")])],
            )],
        ),
        &root.0,
    );
    assert!(dropped(&alone).is_empty(), "nothing dropped");
    assert_eq!(applied(&alone, &root.0, "unsafe")["a.c"], b"abCdef\nghij\n");
}

#[test]
fn fixes_that_do_not_convert_are_dropped() {
    let root = repository();
    let line = printed(STRLEN);
    for (fixes, file, reason) in [
        (
            &[("/src/a.c", 8, 1, "H"), ("/src/b.c", 0, 0, "x")][..],
            "a.c",
            "fix.multi-file",
        ),
        (&[("/src/a.c", 99, 0, "x")][..], "a.c", "fix.out-of-range"),
        (&[("/src/a.c", 10, 5, "x")][..], "a.c", "fix.out-of-range"),
    ] {
        let sarif = run(
            &output(
                &[&line],
                &[diagnostic(STRLEN, "Warning", "/src/a.c", 8, fixes, &[])],
            ),
            &root.0,
        );
        assert_eq!(
            dropped(&sarif),
            owned(&[(STRLEN, file, reason)]),
            "{fixes:?}"
        );
        assert_eq!(results(&sarif)[0].get("fixes"), None);
    }
    // Where the diagnostic is cannot be found.
    let fix = [("/src/a.c", 8, 1, "H")];
    for (file, offset, path) in [("/src/a.c", 99, "a.c"), ("/src/gone.c", 8, "gone.c")] {
        let sarif = run(
            &output(
                &[&line],
                &[diagnostic(STRLEN, "Warning", file, offset, &fix, &[])],
            ),
            &root.0,
        );
        assert_eq!(dropped(&sarif), owned(&[(STRLEN, path, "fix.path")]));
    }
}

#[test]
fn only_printed_warnings_of_reported_checks_with_replacements_get_fixes() {
    let root = repository();
    let fix = [("/src/a.c", 8, 1, "H")];
    let line = printed(STRLEN);
    let off = printed("bugprone-off");
    let sarif = run(
        &output(
            &[&line, &off, &printed(COMPARE)],
            &[
                // An error, an unlisted and an off check, and one with no
                // replacements, whose fix is not reviewed either.
                diagnostic(STRLEN, "Error", "/src/a.c", 8, &fix, &[]),
                diagnostic("bugprone-unlisted", "Warning", "/src/a.c", 8, &fix, &[]),
                diagnostic("bugprone-off", "Warning", "/src/a.c", 8, &fix, &[]),
                diagnostic(COMPARE, "Warning", "/src/a.c", 8, &[], &[("n", &[])]),
                // Not printed: line 1, column 1.
                diagnostic(STRLEN, "Warning", "/src/a.c", 0, &fix, &[]),
            ],
        ),
        &root.0,
    );
    assert!(dropped(&sarif).is_empty(), "{:?}", dropped(&sarif));
    let found = results(&sarif);
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|r| r.get("fixes").is_none()), "{found:?}");
    assert!(sarif["runs"][0].get("artifacts").is_none());
}

#[test]
fn an_export_that_does_not_read_drops_every_fix() {
    let root = repository();
    let line = printed(STRLEN);
    let mut broken = output(&[&line], &[]);
    broken.extend_from_slice(b"Diagnostics: &anchor\n");
    let sarif = run(&broken, &root.0);
    assert_eq!(dropped(&sarif), owned(&[("", "", "fix.unconverted")]));
    assert_eq!(results(&sarif).len(), 1);
    // The marker right after the header: no diagnostics printed.
    let first = format!("# sloptopop-clang-tidy: x\n{MARKER}Diagnostics: &anchor\n");
    assert_eq!(
        dropped(&run(first.as_bytes(), &root.0)),
        owned(&[("", "", "fix.unconverted")])
    );
    // Nothing exported.
    let empty = format!("# sloptopop-clang-tidy: x\n{line}\n{MARKER}\n  \n");
    let quiet = run(empty.as_bytes(), &root.0);
    assert!(dropped(&quiet).is_empty(), "nothing dropped");
    assert_eq!(results(&quiet).len(), 1);
}
