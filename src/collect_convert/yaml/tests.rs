//! Unit tests for [`super`]: the YAML clang-tidy 19 writes for
//! `--export-fixes` (one document as the LX-CAR-1 probe recorded it), every
//! scalar form and escape, and every refusal with its line.

use serde_json::json;

use super::documents;

fn one(text: &str) -> serde_json::Value {
    let mut found = documents(text).unwrap();
    assert_eq!(found.len(), 1, "{text:?}");
    found.remove(0)
}

fn refused(text: &str) -> String {
    documents(text).unwrap_err()
}

#[test]
fn reads_what_clang_tidy_exports() {
    let text = "---
MainSourceFile:  '/src/./bom.c'
Diagnostics:
  - DiagnosticName:  bugprone-x
    DiagnosticMessage:
      Message:         'function ''strcmp'' is called'
      FilePath:        '/src/bom.c'
      FileOffset:      115
      Replacements:
        - FilePath:        '/src/bom.c'
          Offset:          3
          Length:          0
          ReplacementText: \"#include <stddef.h>\\n\"
    Notes:           []
    Level:           Warning
    BuildDirectory:  '/src'
...
";
    assert_eq!(
        documents(text).unwrap(),
        vec![json!({
            "MainSourceFile": "/src/./bom.c",
            "Diagnostics": [{
                "DiagnosticName": "bugprone-x",
                "DiagnosticMessage": {
                    "Message": "function 'strcmp' is called",
                    "FilePath": "/src/bom.c",
                    "FileOffset": "115",
                    "Replacements": [{
                        "FilePath": "/src/bom.c",
                        "Offset": "3",
                        "Length": "0",
                        "ReplacementText": "#include <stddef.h>\n",
                    }],
                },
                "Notes": [],
                "Level": "Warning",
                "BuildDirectory": "/src",
            }],
        })]
    );
}

#[test]
fn documents_are_split_by_their_markers() {
    assert_eq!(documents("").unwrap(), Vec::<serde_json::Value>::new());
    assert_eq!(
        documents("\n  \n").unwrap(),
        Vec::<serde_json::Value>::new()
    );
    assert_eq!(
        documents("---\na: 1\n...\n---\nb: 2\n--- \nc: 3").unwrap(),
        vec![json!({"a": "1"}), json!({"b": "2"}), json!({"c": "3"})]
    );
    assert_eq!(documents("a: 1\n...").unwrap(), vec![json!({"a": "1"})]);
    assert_eq!(documents("- a\n- b").unwrap(), vec![json!(["a", "b"])]);
}

#[test]
fn blocks_nest_by_indentation() {
    assert_eq!(
        one("a:\n  b:\n    c: d\n  e: f\ng: h"),
        json!({"a": {"b": {"c": "d"}, "e": "f"}, "g": "h"})
    );
    // A sequence may sit at its key's own indentation.
    assert_eq!(
        one("a:\n- b\n- c\nd: e"),
        json!({"a": ["b", "c"], "d": "e"})
    );
    assert_eq!(one("a:\n  - b\n\n  - c"), json!({"a": ["b", "c"]}));
    // A key with nothing below it.
    assert_eq!(one("a:\nb: c"), json!({"a": null, "b": "c"}));
    assert_eq!(one("a:"), json!({"a": null}));
    // Items on their own line, and items that open a mapping.
    assert_eq!(one("-\n  - a\n  - b\n- c"), json!([["a", "b"], "c"]));
    assert_eq!(one("-\n  a: b"), json!([{"a": "b"}]));
    assert_eq!(
        one("-   k: v\n    j: w\n-   l: x"),
        json!([{"k": "v", "j": "w"}, {"l": "x"}])
    );
    assert_eq!(one("- k:\n    - v"), json!([{"k": ["v"]}]));
    assert_eq!(one("-   a\n- b  "), json!(["a", "b"]));
    assert_eq!(one("a:    b"), json!({"a": "b"}));
    assert_eq!(one("x.y_z-1: 2"), json!({"x.y_z-1": "2"}));
    // A deeper sequence ends where a shallower one goes on.
    assert_eq!(one("a:\n  - b\nc:\n  - d"), json!({"a": ["b"], "c": ["d"]}));
}

#[test]
fn scalars_plain_quoted_and_empty_collections() {
    assert_eq!(
        one("a: []\nb: {}\nc: x y  \nd: 'it''s'\ne: '' \nf: \"\"\ng: ':#'"),
        json!({"a": [], "b": {}, "c": "x y", "d": "it's", "e": "", "f": "", "g": ":#"})
    );
    assert_eq!(one("- 'a''b''' \n- \"q\""), json!(["a'b'", "q"]));
}

#[test]
fn double_quoted_escapes() {
    assert_eq!(
        one(r#"a: "\0\a\b\t\	\n\v\f\r\e\ \"\/\\\N\_\L\P""#),
        json!({"a": "\0\u{7}\u{8}\t\t\n\u{b}\u{c}\r\u{1b} \"/\\\u{85}\u{a0}\u{2028}\u{2029}"})
    );
    assert_eq!(
        one(r#"a: "\x41B\u00e9\U0001F600z""#),
        json!({"a": "ABé😀z"})
    );
    assert_eq!(one("a: \"it's\""), json!({"a": "it's"}));
    assert_eq!(
        one("a: 'back\\slash \"q\"'"),
        json!({"a": "back\\slash \"q\""})
    );
}

#[test]
fn quoted_scalars_fold_across_lines() {
    // One line break is a space, the white space around it dropped.
    assert_eq!(one("a: 'one  \n   two'"), json!({"a": "one two"}));
    assert_eq!(one("a: \"one\t\n two\""), json!({"a": "one two"}));
    // Blank lines are line feeds.
    assert_eq!(one("a: 'one\n\n  \n two'"), json!({"a": "one\n\ntwo"}));
    // An escaped line break joins the lines, keeping the white space
    // before it; blank lines after it are still line feeds.
    assert_eq!(one("a: \"one \\\n  two\""), json!({"a": "one two"}));
    assert_eq!(one("a: \"one\\\n  two\""), json!({"a": "onetwo"}));
    assert_eq!(one("a: \"one\\\n\n two\""), json!({"a": "one\ntwo"}));
    assert_eq!(one("a: 'x\n y'\nb: c"), json!({"a": "x y", "b": "c"}));
}

#[test]
fn refusals_name_their_line() {
    for (text, error) in [
        ("-", "YAML line 2: expected a nested node"),
        ("-\nx: 1", "YAML line 2: expected a nested node"),
        ("-\n- a", "YAML line 2: expected a nested node"),
        ("-\n  ---", "YAML line 2: expected `key:`"),
        ("a: b\n  c: d", "YAML line 2: unexpected indentation"),
        ("a: b\n- c", "YAML line 2: unexpected indentation"),
        ("just text", "YAML line 1: expected `key:`"),
        ("a b: c", "YAML line 1: expected `key:`"),
        (": c", "YAML line 1: expected `key:`"),
        ("a: 1\na: 2", "YAML line 2: duplicate key"),
        ("- a: &x", "YAML line 1: unsupported scalar"),
        ("- &x", "YAML line 1: unsupported scalar"),
        ("a:\n  b: &x", "YAML line 2: unsupported scalar"),
        ("a:\n- &x", "YAML line 2: unsupported scalar"),
        ("a: &anchor x", "YAML line 1: unsupported scalar"),
        ("a: *alias", "YAML line 1: unsupported scalar"),
        ("a: !tag x", "YAML line 1: unsupported scalar"),
        ("a: |", "YAML line 1: unsupported scalar"),
        ("a: >", "YAML line 1: unsupported scalar"),
        ("a: [b]", "YAML line 1: unsupported scalar"),
        ("a: {b: c}", "YAML line 1: unsupported scalar"),
        ("a: %x", "YAML line 1: unsupported scalar"),
        ("a: @x", "YAML line 1: unsupported scalar"),
        ("a: `x", "YAML line 1: unsupported scalar"),
        ("a: b # comment", "YAML line 1: unsupported scalar"),
        ("a: 'b' c", "YAML line 1: text after a quoted scalar"),
        (
            "x: y\na: \"b\" c",
            "YAML line 2: text after a quoted scalar",
        ),
        ("a: 'b", "YAML line 2: unterminated quoted scalar"),
        ("a: 'b\n\n", "YAML line 4: unterminated quoted scalar"),
        ("a: \"b\\", "YAML line 2: unterminated quoted scalar"),
        ("a: \"\\q\"", "YAML line 1: bad escape"),
        ("a: \"\\xZZ\"", "YAML line 1: bad escape"),
        ("a: \"\\uD800\"", "YAML line 1: bad escape"),
        ("a: \"\\x4\n b\"", "YAML line 1: bad escape"),
        ("a: \"\\U0000004", "YAML line 1: bad escape"),
    ] {
        assert_eq!(refused(text), error, "{text:?}");
    }
}
