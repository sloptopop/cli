//! Unit tests for [`super`]: positions in each convention, the edit checks,
//! the carrier's shape and the per-report bookkeeping.

use std::fs;

use serde_json::{Map, Value, json};

use super::*;
use crate::tests::support::TempDir;

fn source(text: &[u8]) -> Source {
    Source::new(text.to_vec()).unwrap()
}

/// `é` is 2 bytes, 1 code point, 1 UTF-16 unit; `😀` 4, 1, 2.
const MIXED: &[u8] = "a = \"é😀\"; b\nnext\n".as_bytes();

#[test]
fn files_that_are_not_utf8_or_have_a_lone_carriage_return_are_refused() {
    assert_eq!(Source::new(vec![0xFF, b'a']), Err(Drop::NotUtf8));
    assert_eq!(Source::new(b"a\rb\n".to_vec()), Err(Drop::LoneCr));
    assert_eq!(Source::new(b"a\r".to_vec()), Err(Drop::LoneCr));
    assert!(Source::new(b"a\r\nb\r\n".to_vec()).is_ok());
    assert_eq!(source(b"xy").bytes(), b"xy");
}

#[test]
fn code_point_columns_count_characters() {
    let mixed = source(MIXED);
    // `;` is code point 9 (column 10): byte 4 + 1 + 2 + 4 + 1 = 12.
    for columns in [Columns::CodePointsBomSkipped, Columns::CodePointsBomCounted] {
        assert_eq!(mixed.line_column(columns, 1, 1), Ok(0));
        assert_eq!(mixed.line_column(columns, 1, 9), Ok(12));
        assert_eq!(
            mixed.line_column(columns, 1, 12),
            Ok(15),
            "past the last character"
        );
        assert_eq!(mixed.line_column(columns, 1, 13), Err(Drop::OutOfRange));
        assert_eq!(mixed.line_column(columns, 2, 1), Ok(16));
        assert_eq!(
            mixed.line_column(columns, 3, 1),
            Ok(21),
            "after the final newline"
        );
        assert_eq!(mixed.line_column(columns, 4, 1), Err(Drop::OutOfRange));
        assert_eq!(mixed.line_column(columns, 0, 1), Err(Drop::OutOfRange));
        assert_eq!(mixed.line_column(columns, 1, 0), Err(Drop::OutOfRange));
    }
}

#[test]
fn utf8_columns_count_bytes_and_must_fall_on_a_character() {
    let mixed = source(MIXED);
    assert_eq!(mixed.line_column(Columns::Utf8Bytes, 1, 13), Ok(12));
    assert_eq!(
        mixed.line_column(Columns::Utf8Bytes, 1, 7),
        Err(Drop::Boundary),
        "inside é"
    );
    assert_eq!(mixed.line_column(Columns::Utf8Bytes, 1, 16), Ok(15));
    assert_eq!(
        mixed.line_column(Columns::Utf8Bytes, 1, 17),
        Err(Drop::OutOfRange)
    );
}

#[test]
fn the_byte_order_mark_is_a_column_for_biome_and_not_for_ruff() {
    let bom = source(b"\xEF\xBB\xBFab\ncd\n");
    assert_eq!(bom.line_column(Columns::CodePointsBomSkipped, 1, 1), Ok(3));
    assert_eq!(bom.line_column(Columns::CodePointsBomCounted, 1, 1), Ok(0));
    assert_eq!(bom.line_column(Columns::CodePointsBomCounted, 1, 2), Ok(3));
    assert_eq!(
        bom.line_column(Columns::CodePointsBomSkipped, 2, 1),
        Ok(6),
        "only line 1"
    );
    assert_eq!(bom.line_column(Columns::CodePointsBomCounted, 2, 2), Ok(7));
    assert_eq!(bom.line_column(Columns::Utf8Bytes, 1, 1), Ok(0));
}

#[test]
fn a_position_between_carriage_return_and_line_feed_is_refused() {
    let crlf = source(b"ab\r\ncd\r\n");
    assert_eq!(crlf.line_column(Columns::CodePointsBomSkipped, 1, 3), Ok(2));
    assert_eq!(
        crlf.line_column(Columns::CodePointsBomSkipped, 1, 4),
        Err(Drop::Boundary)
    );
    assert_eq!(crlf.line_column(Columns::CodePointsBomSkipped, 2, 1), Ok(4));
    assert_eq!(crlf.byte(3), Err(Drop::Boundary));
    assert_eq!(crlf.byte(2), Ok(2));
    assert_eq!(source(b"\nx").byte(0), Ok(0), "a line feed at the start");
}

#[test]
fn utf16_offsets_skip_the_byte_order_mark_and_never_split_a_pair() {
    let mixed = source(MIXED);
    // Units: a=0 … `"`=4, é=5, 😀=6,7, `"`=8, `;`=9.
    assert_eq!(mixed.utf16(0), Ok(0));
    assert_eq!(mixed.utf16(6), Ok(7));
    assert_eq!(mixed.utf16(7), Err(Drop::Surrogate));
    assert_eq!(mixed.utf16(8), Ok(11));
    assert_eq!(mixed.utf16(9), Ok(12));
    let units = i64::try_from("a = \"é😀\"; b\nnext\n".encode_utf16().count()).unwrap();
    assert_eq!(mixed.utf16(units), Ok(MIXED.len()), "the end");
    assert_eq!(mixed.utf16(units + 1), Err(Drop::OutOfRange));
    assert_eq!(mixed.utf16(-1), Err(Drop::Bom), "ESLint's BOM removal");
    let bom = source(b"\xEF\xBB\xBFab");
    assert_eq!(bom.utf16(0), Ok(3));
    assert_eq!(bom.utf16(2), Ok(5));
}

#[test]
fn byte_offsets_are_checked_against_the_file() {
    let mixed = source(MIXED);
    assert_eq!(mixed.byte(5), Ok(5));
    assert_eq!(mixed.byte(6), Err(Drop::Boundary));
    assert_eq!(
        mixed.byte(u64::try_from(MIXED.len()).unwrap()),
        Ok(MIXED.len())
    );
    assert_eq!(
        mixed.byte(u64::try_from(MIXED.len()).unwrap() + 1),
        Err(Drop::OutOfRange)
    );
    assert_eq!(mixed.byte(u64::MAX), Err(Drop::OutOfRange));
}

#[test]
fn byte_offsets_have_clangs_line_and_byte_column() {
    let text = source("\u{feff}ab\r\né😀x\n".as_bytes());
    assert_eq!(text.position(0), Some((1, 1)));
    assert_eq!(
        text.position(3),
        Some((1, 4)),
        "the byte order mark is 3 bytes"
    );
    assert_eq!(text.position(5), Some((1, 6)), "the carriage return");
    assert_eq!(text.position(6), Some((1, 7)), "the line feed ends line 1");
    assert_eq!(text.position(7), Some((2, 1)));
    assert_eq!(text.position(13), Some((2, 7)), "after `é😀`");
    assert_eq!(text.position(15), Some((3, 1)), "the end of the file");
    assert_eq!(text.position(16), None);
    assert_eq!(text.position(u64::MAX), None);
    assert_eq!(source(b"").position(0), Some((1, 1)));
}

fn edit(start: usize, end: usize, text: &str) -> Edit {
    Edit {
        start,
        end,
        text: text.to_owned(),
    }
}

fn fix(edits: Vec<Edit>) -> Fix {
    Fix {
        level: Level::Safe,
        tool_level: "safe".to_owned(),
        description: "Do it".to_owned(),
        source: "test",
        path: "a.py".to_owned(),
        edits,
        reviewed: true,
    }
}

#[test]
fn edits_must_be_ordered_disjoint_and_keep_the_byte_order_mark() {
    let plain = source(b"0123456789");
    let ok = |edits| fix(edits).checked(&plain).map(|f| f.edits.len());
    assert_eq!(
        ok(vec![edit(1, 2, "x"), edit(2, 3, "y")]),
        Ok(2),
        "adjacent"
    );
    assert_eq!(
        ok(vec![edit(1, 1, "x"), edit(3, 3, "y")]),
        Ok(2),
        "two insertions apart"
    );
    assert_eq!(
        ok(vec![edit(1, 1, "x"), edit(1, 2, "y")]),
        Ok(2),
        "insert then replace"
    );
    assert_eq!(
        ok(vec![edit(1, 2, "x"), edit(2, 2, "y")]),
        Ok(2),
        "replace then insert"
    );
    assert_eq!(
        ok(vec![edit(2, 2, "x"), edit(2, 2, "y")]),
        Err(Drop::Overlap),
        "same point"
    );
    assert_eq!(
        ok(vec![edit(1, 3, "x"), edit(2, 4, "y")]),
        Err(Drop::Overlap)
    );
    assert_eq!(
        ok(vec![edit(4, 5, "x"), edit(1, 2, "y")]),
        Err(Drop::Overlap),
        "order"
    );
    assert_eq!(ok(vec![edit(3, 2, "x")]), Err(Drop::Overlap), "backwards");
    assert_eq!(ok(vec![]), Err(Drop::Placeholder));
    let bom = source(b"\xEF\xBB\xBFab");
    let checked = |edits| fix(edits).checked(&bom).map(|f| f.edits.len());
    assert_eq!(checked(vec![edit(0, 3, "")]), Err(Drop::Bom));
    assert_eq!(checked(vec![edit(2, 4, "")]), Err(Drop::Bom));
    assert_eq!(
        checked(vec![edit(0, 0, "x")]),
        Err(Drop::Bom),
        "an insertion before it"
    );
    assert_eq!(checked(vec![edit(3, 4, "x")]), Ok(1));
}

#[test]
fn the_carrier_holds_byte_regions_and_the_levels() {
    let mut carried = fix(vec![edit(4, 6, "b"), edit(8, 9, "")]);
    carried.reviewed = false;
    assert_eq!(
        carried.sarif(),
        json!({
            "description": { "text": "Do it" },
            "artifactChanges": [{
                "artifactLocation": { "uri": "a.py" },
                "replacements": [
                    { "deletedRegion": { "byteOffset": 4, "byteLength": 2 },
                      "insertedContent": { "text": "b" } },
                    { "deletedRegion": { "byteOffset": 8, "byteLength": 1 } },
                ],
            }],
            "properties": {
                "sloptopop/applicability": "safe",
                "sloptopop/toolApplicability": "safe",
                "sloptopop/source": "test",
                "sloptopop/converter": 1,
                "sloptopop/reviewed": false,
            },
        })
    );
    assert!(is_carrier(&carried.sarif()));
    assert!(!is_carrier(&json!({ "properties": {} })));
    assert_eq!(
        [Level::Safe, Level::Unsafe, Level::Suggestion].map(Level::as_str),
        ["safe", "unsafe", "suggestion"]
    );
}

#[test]
fn every_drop_reason_has_its_name() {
    let all = [
        (Drop::NotUtf8, "fix.not-utf8"),
        (Drop::LoneCr, "fix.lone-cr"),
        (Drop::OutOfRange, "fix.out-of-range"),
        (Drop::Boundary, "fix.boundary"),
        (Drop::Surrogate, "fix.surrogate"),
        (Drop::Bom, "fix.bom"),
        (Drop::Notebook, "fix.notebook"),
        (Drop::Overlap, "fix.overlap-within"),
        (Drop::Alternatives, "fix.alternatives"),
        (Drop::MultiFile, "fix.multi-file"),
        (Drop::Placeholder, "fix.placeholder"),
        (Drop::Stale, "fix.stale"),
        (Drop::Path, "fix.path"),
        (Drop::Config, "fix.config"),
        (Drop::Unconverted, "fix.unconverted"),
        (Drop::Unreviewed, "fix.unreviewed"),
    ];
    for (drop, name) in all {
        assert_eq!(drop.as_str(), name);
    }
}

#[test]
fn files_are_read_once_and_only_below_the_root() {
    let root = TempDir::new();
    fs::create_dir_all(root.0.join("src")).unwrap();
    fs::write(root.0.join("src/a.py"), "import os\n").unwrap();
    fs::write(root.0.join("bad.py"), [0xFF]).unwrap();
    let mut store = Files::new(&root.0);
    assert_eq!(store.get("src/a.py").map(|s| s.bytes().len()), Ok(10));
    fs::write(root.0.join("src/a.py"), "changed").unwrap();
    assert_eq!(
        store.get("src/a.py").map(|s| s.bytes().len()),
        Ok(10),
        "read once"
    );
    assert_eq!(store.get("bad.py").err(), Some(Drop::NotUtf8));
    for path in [
        "",
        "../x",
        "/etc/passwd",
        "src",
        "missing.py",
        "src/../src/a.py",
    ] {
        assert_eq!(store.get(path).err(), Some(Drop::Path), "{path}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.0.join("src/a.py"), root.0.join("link.py")).unwrap();
        assert_eq!(store.get("link.py").err(), Some(Drop::Path), "a link");
    }
}

#[test]
fn kept_fixes_name_their_files_and_drops_are_recorded_sorted() {
    let root = TempDir::new();
    fs::write(root.0.join("a.py"), "abc\n").unwrap();
    fs::write(root.0.join("b.py"), "xyz\n").unwrap();
    let mut store = Files::new(&root.0);
    let kept = store.keep("R1", "a.py", |_| Ok(fix(vec![edit(0, 1, "z")])));
    assert_eq!(kept.map(|f| f.edits), Some(vec![edit(0, 1, "z")]));
    assert!(
        store
            .keep("R2", "a.py", |_| Err(Drop::OutOfRange))
            .is_none()
    );
    assert!(
        store.keep("R0", "b.py", |_| Ok(fix(vec![]))).is_none(),
        "checked"
    );
    assert!(
        store
            .keep("R3", "none.py", |_| Ok(fix(vec![edit(0, 1, "")])))
            .is_none()
    );
    store.drop("R2", "a.py", Drop::OutOfRange);
    assert_eq!(
        store.artifacts(),
        vec![json!({
            "location": { "uri": "a.py" },
            "length": 4,
            "hashes": { "blake3": blake3::hash(b"abc\n").to_hex().to_string() },
        })],
        "only files a kept fix names"
    );
    assert_eq!(
        store.dropped(),
        vec![
            json!({ "rule": "R0", "path": "b.py", "reason": "fix.placeholder" }),
            json!({ "rule": "R2", "path": "a.py", "reason": "fix.out-of-range" }),
            json!({ "rule": "R3", "path": "none.py", "reason": "fix.path" }),
        ]
    );
    let mut run = Map::new();
    run.insert(
        "artifacts".to_owned(),
        json!([{ "location": { "uri": "x" } }]),
    );
    run.insert("properties".to_owned(), json!({ "kept": true }));
    store.record(&mut run);
    assert_eq!(
        run["artifacts"].as_array().map(Vec::len),
        Some(2),
        "appended"
    );
    assert_eq!(run["properties"]["kept"], true);
    assert_eq!(
        run["properties"]["sloptopop/droppedFixes"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );
    let mut bare = Map::new();
    store.record(&mut bare);
    assert!(bare["artifacts"].is_array() && bare["properties"].is_object());
    let mut untouched = Map::new();
    Files::new(&root.0).record(&mut untouched);
    assert!(untouched.is_empty(), "nothing to record");
    let mut odd = Map::new();
    odd.insert("artifacts".to_owned(), Value::Null);
    odd.insert("properties".to_owned(), Value::Null);
    store.record(&mut odd);
    assert_eq!(odd["artifacts"], Value::Null, "not an array: left alone");
    assert_eq!(odd["properties"], Value::Null);
}

#[test]
fn several_fixes_of_one_level_are_alternatives_and_none_is_kept() {
    let root = TempDir::new();
    let mut store = Files::new(&root.0);
    let mut suggestion = fix(vec![edit(0, 1, "a")]);
    suggestion.level = Level::Suggestion;
    let mut other = suggestion.clone();
    other.edits = vec![edit(0, 1, "b")];
    let safe = fix(vec![edit(2, 3, "c")]);
    let kept = one_per_level(&mut store, "R", vec![suggestion, safe.clone(), other]);
    assert_eq!(kept, vec![safe]);
    assert_eq!(
        store.dropped(),
        vec![json!({ "rule": "R", "path": "a.py", "reason": "fix.alternatives" })]
    );
}
