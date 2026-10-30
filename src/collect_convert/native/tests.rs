//! Unit tests for [`super`], on what the pinned tools printed for the
//! synthetic WP-21 probe (`tests/fixtures/fix-probe/`: `é😀` before the fixed
//! site, a CRLF file and a file with a byte order mark), and on doctored
//! outputs for every refusal. The golden check: the carried fixes, applied
//! once, give the bytes the tool's own fix mode wrote.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::collect_convert::{Carrier, Converter, carry, convert};
use crate::tests::support::TempDir;

fn probe(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fix-probe")
        .join(dir)
}

fn raw(name: &str) -> Vec<u8> {
    fs::read(probe(name)).unwrap()
}

fn document(sarif: &str) -> Value {
    serde_json::from_str(sarif).unwrap()
}

fn results(sarif: &Value) -> Vec<Value> {
    sarif["runs"][0]["results"].as_array().unwrap().clone()
}

/// Every carried fix of `level`, applied once per file to the files below
/// `root`, in byte order.
fn applied(sarif: &Value, root: &Path, level: &str) -> BTreeMap<String, Vec<u8>> {
    let mut edits: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
    for result in results(sarif) {
        for fix in result["fixes"].as_array().into_iter().flatten() {
            if fix["properties"]["sloptopop/applicability"] != level {
                continue;
            }
            let change = &fix["artifactChanges"][0];
            let path = change["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .to_owned();
            for replacement in change["replacements"].as_array().unwrap() {
                let region = &replacement["deletedRegion"];
                let start = usize::try_from(region["byteOffset"].as_u64().unwrap()).unwrap();
                let length = usize::try_from(region["byteLength"].as_u64().unwrap()).unwrap();
                let text = replacement["insertedContent"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                edits
                    .entry(path.clone())
                    .or_default()
                    .push((start, start + length, text));
            }
        }
    }
    edits
        .into_iter()
        .map(|(path, mut list)| {
            list.sort();
            let bytes = fs::read(root.join(&path)).unwrap();
            let mut out = Vec::new();
            let mut last = 0;
            for (start, end, text) in list {
                assert!(start >= last, "{path}: overlapping edits");
                out.extend_from_slice(&bytes[last..start]);
                out.extend_from_slice(text.as_bytes());
                last = end;
            }
            out.extend_from_slice(&bytes[last..]);
            (path, out)
        })
        .collect()
}

fn dropped(sarif: &Value) -> Vec<(String, String)> {
    sarif["runs"][0]["properties"]["sloptopop/droppedFixes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            (
                d["path"].as_str().unwrap().to_owned(),
                d["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

const PROBE_FILES: [&str; 4] = ["ascii", "bom", "crlf", "nonascii"];

#[test]
fn ruff_fixes_reproduce_ruff_check_fix_on_every_probe_file() {
    let root = probe("py");
    let sarif = document(
        &convert(
            Converter::RuffJson,
            &raw("ruff.json"),
            "ruff",
            "0.17.1-1",
            &root,
        )
        .unwrap(),
    );
    let fixed = applied(&sarif, &root, "safe");
    assert_eq!(fixed.len(), 4);
    for name in PROBE_FILES {
        let file = format!("{name}.py");
        assert_eq!(
            fixed[&file],
            fs::read(probe("expected/ruff").join(&file)).unwrap(),
            "{file}"
        );
    }
    let first = &results(&sarif)[0];
    assert_eq!(first["ruleId"], "F401");
    assert_eq!(first["level"], "error");
    let properties = &first["fixes"][0]["properties"];
    assert_eq!(properties["sloptopop/toolApplicability"], "safe");
    assert_eq!(properties["sloptopop/source"], "ruff-json");
    assert_eq!(
        properties["sloptopop/reviewed"], false,
        "F401 is not in the table"
    );
    let artifacts = sarif["runs"][0]["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 4);
    assert_eq!(
        artifacts[0]["hashes"]["blake3"],
        blake3::hash(&fs::read(root.join("ascii.py")).unwrap())
            .to_hex()
            .to_string()
    );
}

/// A one-message Ruff report on `a.py` with `fix`.
fn ruff_report(fix: &Value, cell: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!([{
        "cell": cell, "code": "UP045", "name": "x", "severity": "warning",
        "filename": "/src/a.py", "message": "m",
        "location": { "row": 1, "column": 1 }, "end_location": { "row": 1, "column": 2 },
        "fix": fix,
    }, {
        "cell": null, "code": null, "name": "invalid-syntax", "severity": "info",
        "filename": "b.py", "message": "s", "location": null, "end_location": null, "fix": null,
    }]))
    .unwrap()
}

fn edit_fix(applicability: &str, end_column: u64) -> Value {
    json!({ "applicability": applicability, "message": "Use it", "edits": [
        { "content": "y", "location": { "row": 1, "column": 1 },
          "end_location": { "row": 1, "column": end_column } }] })
}

#[test]
fn ruff_levels_notebooks_ranges_and_promoting_configurations() {
    let root = TempDir::new();
    fs::write(root.0.join("a.py"), "xz\n").unwrap();
    let run = |report: Vec<u8>| {
        document(&convert(Converter::RuffJson, &report, "ruff", "0.17.1-1", &root.0).unwrap())
    };
    let ok = run(ruff_report(&edit_fix("safe", 2), &Value::Null));
    let found = results(&ok);
    assert_eq!(found[0]["level"], "warning");
    assert_eq!(
        found[1]["ruleId"], "invalid-syntax",
        "the name when there is no code"
    );
    assert_eq!(found[1]["level"], "note");
    assert!(found[1].get("locations").is_some_and(Value::is_array));
    let fix = &found[0]["fixes"][0];
    assert_eq!(fix["description"]["text"], "Use it");
    assert_eq!(
        fix["properties"]["sloptopop/reviewed"], true,
        "UP045 is reviewed"
    );
    assert_eq!(applied(&ok, &root.0, "safe")["a.py"], b"yz\n");
    for (applicability, level) in [("unsafe", "unsafe"), ("displayonly", "suggestion")] {
        let sarif = run(ruff_report(&edit_fix(applicability, 2), &Value::Null));
        assert_eq!(
            results(&sarif)[0]["fixes"][0]["properties"]["sloptopop/applicability"],
            level
        );
    }
    for (report, reason) in [
        (ruff_report(&edit_fix("safe", 2), &json!(1)), "fix.notebook"),
        (
            ruff_report(&edit_fix("sometimes", 2), &Value::Null),
            "fix.unconverted",
        ),
        (
            ruff_report(&edit_fix("safe", 9), &Value::Null),
            "fix.out-of-range",
        ),
    ] {
        let sarif = run(report);
        assert!(results(&sarif)[0].get("fixes").is_none());
        assert_eq!(
            dropped(&sarif),
            vec![("a.py".to_owned(), reason.to_owned())]
        );
    }
    for member in ["location", "end_location"] {
        let mut broken = edit_fix("safe", 2);
        broken["edits"][0][member] = Value::Null;
        assert_eq!(
            dropped(&run(ruff_report(&broken, &Value::Null))),
            vec![("a.py".to_owned(), "fix.out-of-range".to_owned())],
            "{member}"
        );
    }
    fs::write(
        root.0.join("pyproject.toml"),
        "[tool.ruff.lint]\nextend-safe-fixes = [\"UP\"]\n",
    )
    .unwrap();
    let promoted = run(ruff_report(&edit_fix("safe", 2), &Value::Null));
    assert_eq!(
        dropped(&promoted),
        vec![("a.py".to_owned(), "fix.config".to_owned())]
    );
    let unsafe_kept = run(ruff_report(&edit_fix("unsafe", 2), &Value::Null));
    assert!(
        dropped(&unsafe_kept).is_empty(),
        "only safe fixes can be promoted"
    );
    assert!(
        convert(Converter::RuffJson, b"{}", "ruff", "v", &root.0)
            .unwrap_err()
            .contains("not an array")
    );
}

#[test]
fn eslint_fixes_are_utf16_offsets_without_the_byte_order_mark() {
    let root = probe("js");
    let sarif = document(
        &convert(
            Converter::EslintJson,
            &raw("eslint.json"),
            "eslint-typescript",
            "8.71.2-1",
            &root,
        )
        .unwrap(),
    );
    assert!(dropped(&sarif).is_empty(), "{:?}", dropped(&sarif));
    let fixed = applied(&sarif, &root, "safe");
    for name in PROBE_FILES {
        let file = format!("{name}.js");
        let original = String::from_utf8(fs::read(root.join(&file)).unwrap()).unwrap();
        // One pass: ESLint's own --fix makes `let u` a `const u` in a second
        // pass (prefer-const after no-var), which one report never shows.
        let once = original
            .replace("let a = 1;", "const a = 1;")
            .replace("var u", "let u");
        assert_eq!(
            String::from_utf8(fixed[&file].clone()).unwrap(),
            once,
            "{file}"
        );
    }
    let suggested = applied(&sarif, &root, "suggestion");
    let original = String::from_utf8(fs::read(root.join("nonascii.js")).unwrap()).unwrap();
    assert_eq!(
        String::from_utf8(suggested["nonascii.js"].clone()).unwrap(),
        original.replace("a == 1", "a === 1")
    );
    let levels: Vec<String> = results(&sarif)
        .iter()
        .map(|r| r["level"].as_str().unwrap().to_owned())
        .collect();
    assert!(levels.iter().all(|level| level == "error"));
}

fn eslint_report(source: Option<&str>, message: &Value) -> Vec<u8> {
    let mut result = json!({ "filePath": "/src/a.js", "messages": [message,
        { "ruleId": null, "severity": 2, "message": "Parsing error", "line": 1, "column": 1 }] });
    if let Some(source) = source {
        result["source"] = json!(source);
    }
    serde_json::to_vec(&json!({ "results": [result], "metadata": {} })).unwrap()
}

#[test]
fn eslint_stale_sources_alternatives_and_malformed_ranges_drop_the_fix() {
    let root = TempDir::new();
    fs::write(root.0.join("a.js"), "var x = 1;\n").unwrap();
    let run = |report: Vec<u8>| {
        document(
            &convert(
                Converter::EslintJson,
                &report,
                "eslint-sonarjs",
                "2.0.4-1",
                &root.0,
            )
            .unwrap(),
        )
    };
    let message = |fix: Value, suggestions: Value| {
        json!({ "ruleId": "sonarjs/no-var", "severity": 1, "message": "m", "line": 1,
                "column": 1, "fix": fix, "suggestions": suggestions })
    };
    let good = json!({ "range": [0, 3], "text": "let" });
    let ok = run(eslint_report(
        Some("var x = 1;\n"),
        &message(good.clone(), json!([])),
    ));
    assert_eq!(results(&ok).len(), 1, "ESLint's own errors are no findings");
    assert_eq!(results(&ok)[0]["level"], "warning");
    assert_eq!(
        results(&ok)[0]["fixes"][0]["properties"]["sloptopop/reviewed"],
        true
    );
    assert_eq!(applied(&ok, &root.0, "safe")["a.js"], b"let x = 1;\n");
    let stale = run(eslint_report(
        Some("var y = 2;\n"),
        &message(good.clone(), json!([])),
    ));
    assert_eq!(
        dropped(&stale),
        vec![("a.js".to_owned(), "fix.stale".to_owned())]
    );
    let unread = run(eslint_report(None, &message(good.clone(), json!([]))));
    assert!(dropped(&unread).is_empty(), "no source: nothing to compare");
    let two = json!([{ "desc": "a", "fix": { "range": [0, 0], "text": "x" } },
                     { "desc": "b", "fix": { "range": [1, 1], "text": "y" } }]);
    let alternatives = run(eslint_report(None, &message(good.clone(), two)));
    assert_eq!(
        dropped(&alternatives),
        vec![("a.js".to_owned(), "fix.alternatives".to_owned())]
    );
    assert_eq!(
        results(&alternatives)[0]["fixes"].as_array().map(Vec::len),
        Some(1)
    );
    let one = json!([{ "desc": "Use let", "fix": { "range": [0, 3], "text": "let" } }]);
    let suggested = run(eslint_report(None, &message(Value::Null, one)));
    let carried = &results(&suggested)[0]["fixes"][0];
    assert_eq!(carried["description"]["text"], "Use let");
    assert_eq!(
        carried["properties"]["sloptopop/applicability"],
        "suggestion"
    );
    assert!(convert(Converter::EslintJson, b"[]", "e", "v", &root.0).is_err());
}

#[test]
fn eslint_ranges_that_do_not_convert_drop_the_fix() {
    let root = TempDir::new();
    fs::write(root.0.join("a.js"), "var x = 1;\n").unwrap();
    let run = |report: Vec<u8>| {
        document(
            &convert(
                Converter::EslintJson,
                &report,
                "eslint-sonarjs",
                "2.0.4-1",
                &root.0,
            )
            .unwrap(),
        )
    };
    let message = |fix: Value, suggestions: Value| {
        json!({ "ruleId": "sonarjs/no-var", "severity": 1, "message": "m", "line": 1,
                "column": 1, "fix": fix, "suggestions": suggestions })
    };
    for (fix, reason) in [
        (json!({ "range": [0], "text": "x" }), "fix.placeholder"),
        (json!({ "range": ["a", 1], "text": "x" }), "fix.placeholder"),
        (json!({ "range": [0, "b"], "text": "x" }), "fix.placeholder"),
        (json!({ "range": [-1, 0], "text": "" }), "fix.bom"),
        (json!({ "range": [0, 99], "text": "" }), "fix.out-of-range"),
    ] {
        let sarif = run(eslint_report(None, &message(fix, json!([]))));
        assert_eq!(
            dropped(&sarif),
            vec![("a.js".to_owned(), reason.to_owned())]
        );
    }
    let missing = run(eslint_report(
        None,
        &message(Value::Null, json!([{ "desc": "d" }])),
    ));
    assert_eq!(
        dropped(&missing),
        vec![("a.js".to_owned(), "fix.placeholder".to_owned())]
    );
}

#[test]
fn biome_fixes_reproduce_biome_lint_write_on_every_probe_file() {
    let root = probe("js");
    let sarif = document(
        &convert(
            Converter::BiomeRdjson,
            &raw("biome.rdjson"),
            "biome",
            "2.5.15-1",
            &root,
        )
        .unwrap(),
    );
    assert!(dropped(&sarif).is_empty(), "{:?}", dropped(&sarif));
    let fixed = applied(&sarif, &root, "safe");
    for name in PROBE_FILES {
        let file = format!("{name}.js");
        assert_eq!(
            fixed[&file],
            fs::read(probe("expected/biome").join(&file)).unwrap(),
            "{file}"
        );
    }
    let by_rule = |rule: &str| {
        results(&sarif)
            .into_iter()
            .find(|r| r["ruleId"] == rule)
            .unwrap()
    };
    let unsafe_fix = &by_rule("lint/suspicious/noDoubleEquals")["fixes"][0]["properties"];
    assert_eq!(unsafe_fix["sloptopop/applicability"], "unsafe");
    assert_eq!(unsafe_fix["sloptopop/reviewed"], false);
    assert_eq!(
        by_rule("lint/style/useConst")["fixes"][0]["properties"]["sloptopop/reviewed"],
        true
    );
}

fn biome_report(rule: &str, suggestions: &Value, location: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({ "diagnostics": [{
        "code": { "value": rule }, "message": "m", "severity": "ERROR",
        "location": location, "suggestions": suggestions }] }))
    .unwrap()
}

#[test]
fn biome_kinds_versions_configurations_and_alternatives() {
    let root = TempDir::new();
    fs::write(root.0.join("a.js"), "let a = 1;\n").unwrap();
    let at = json!({ "path": "a.js", "range": { "start": { "line": 1, "column": 1 }, "end": { "line": 1, "column": 4 } } });
    let one = json!([{ "range": { "start": { "line": 1, "column": 1 }, "end": { "line": 1, "column": 4 } }, "text": "const" }]);
    let run = |report: Vec<u8>, version: &str| {
        document(&convert(Converter::BiomeRdjson, &report, "biome", version, &root.0).unwrap())
    };
    let ok = run(biome_report("lint/style/useConst", &one, &at), "2.5.15-1");
    assert_eq!(results(&ok)[0]["level"], "error");
    assert_eq!(applied(&ok, &root.0, "safe")["a.js"], b"const a = 1;\n");
    for (report, version, reason) in [
        (
            biome_report("lint/style/useConst", &one, &at),
            "2.6.0-1",
            "fix.unconverted",
        ),
        (
            biome_report("lint/x/unknownRule", &one, &at),
            "2.5.15-1",
            "fix.unconverted",
        ),
        (
            biome_report(
                "lint/style/useConst",
                &json!([one[0].clone(), one[0].clone()]),
                &at,
            ),
            "2.5.15-1",
            "fix.alternatives",
        ),
    ] {
        assert_eq!(
            dropped(&run(report, version)),
            vec![("a.js".to_owned(), reason.to_owned())]
        );
    }
    for bad_range in [
        json!([{ "range": { "start": { "line": 9, "column": 1 }, "end": { "line": 9, "column": 1 } }, "text": "x" }]),
        json!([{ "range": { "start": { "line": 1, "column": 1 }, "end": { "line": 9, "column": 1 } }, "text": "x" }]),
    ] {
        assert_eq!(
            dropped(&run(
                biome_report("lint/style/useConst", &bad_range, &at),
                "2.5.15-1"
            )),
            vec![("a.js".to_owned(), "fix.out-of-range".to_owned())]
        );
    }
    let nowhere = run(biome_report("parse", &json!([]), &Value::Null), "2.5.15-1");
    assert!(
        results(&nowhere)[0].get("locations").is_none(),
        "no location"
    );
    fs::write(
        root.0.join("biome.json"),
        r#"{"linter":{"rules":{"style":{"useConst":{"fix":"unsafe"}}}}}"#,
    )
    .unwrap();
    assert_eq!(
        dropped(&run(
            biome_report("lint/style/useConst", &one, &at),
            "2.5.15-1"
        )),
        vec![("a.js".to_owned(), "fix.config".to_owned())]
    );
    let unsafe_rule = run(
        biome_report("lint/suspicious/noDoubleEquals", &one, &at),
        "2.5.15-1",
    );
    assert!(
        dropped(&unsafe_rule).is_empty(),
        "an unsafe fix stays unsafe"
    );
    assert!(convert(Converter::BiomeRdjson, b"{}", "biome", "v", &root.0).is_err());
}

#[test]
fn opengrep_rule_fixes_are_byte_offsets_and_suggestions() {
    let root = probe("js");
    let sarif = document(
        &convert(
            Converter::SemgrepJson,
            &raw("opengrep.json"),
            "opengrep",
            "1.31.0-1",
            &root,
        )
        .unwrap(),
    );
    let fixed = applied(&sarif, &root, "suggestion");
    for name in PROBE_FILES {
        let file = format!("{name}.js");
        let original = String::from_utf8(fs::read(root.join(&file)).unwrap()).unwrap();
        assert_eq!(
            String::from_utf8(fixed[&file].clone()).unwrap(),
            original.replace("(a == 1)", "(a === 1)"),
            "{file}"
        );
    }
    let first = &results(&sarif)[0];
    assert_eq!(first["ruleId"], "probe-loose-equality");
    assert_eq!(first["level"], "warning");
    assert_eq!(
        first["fixes"][0]["properties"]["sloptopop/toolApplicability"],
        "rule-fix"
    );
}

#[test]
fn opengrep_severities_and_unconvertible_offsets() {
    let root = TempDir::new();
    fs::write(root.0.join("a.js"), "é\n").unwrap();
    let report = json!({ "results": [
        { "check_id": "r1", "path": "a.js", "start": { "line": 1, "col": 1, "offset": 1 },
          "end": { "offset": 2 }, "extra": { "message": "m", "severity": "ERROR", "fix": "x" } },
        { "check_id": "r0", "path": "a.js", "start": { "line": 1, "col": 1, "offset": 0 },
          "end": { "offset": 9 }, "extra": { "message": "m", "severity": "ERROR", "fix": "x" } },
        { "check_id": "r2", "path": "a.js", "start": { "line": 1, "col": 1, "offset": 0 },
          "end": { "offset": 0 }, "extra": { "message": "m", "severity": "INFO" } },
        { "check_id": "r3", "path": "a.js", "start": { "line": 1, "col": 1, "offset": 0 },
          "end": { "offset": 0 }, "extra": { "message": "m", "severity": "MEDIUM" } },
        { "check_id": "r4", "path": "a.js", "start": { "line": 1, "col": 1, "offset": 0 },
          "end": { "offset": 0 }, "extra": { "message": "m", "severity": "CRITICAL" } }] });
    let sarif = document(
        &convert(
            Converter::SemgrepJson,
            &serde_json::to_vec(&report).unwrap(),
            "semgrep",
            "1.181.0-1",
            &root.0,
        )
        .unwrap(),
    );
    let levels: Vec<String> = results(&sarif)
        .iter()
        .map(|r| {
            format!(
                "{} {}",
                r["ruleId"].as_str().unwrap(),
                r["level"].as_str().unwrap()
            )
        })
        .collect();
    assert_eq!(
        levels,
        ["r0 error", "r1 error", "r2 note", "r3 warning", "r4 error"]
    );
    assert_eq!(
        dropped(&sarif),
        vec![
            ("a.js".to_owned(), "fix.out-of-range".to_owned()),
            ("a.js".to_owned(), "fix.boundary".to_owned()),
        ]
    );
    assert!(convert(Converter::SemgrepJson, b"{}", "opengrep", "v", &root.0).is_err());
}

#[test]
fn the_public_carrier_converts_as_collect_does() {
    for (name, format, converter, raw_name, dir, tool, version) in [
        (
            "ruff-json",
            Carrier::RuffJson,
            Converter::RuffJson,
            "ruff.json",
            "py",
            "ruff",
            "0.17.1-1",
        ),
        (
            "eslint-json",
            Carrier::EslintJson,
            Converter::EslintJson,
            "eslint.json",
            "js",
            "eslint-typescript",
            "8.71.2-1",
        ),
        (
            "biome-rdjson",
            Carrier::BiomeRdjson,
            Converter::BiomeRdjson,
            "biome.rdjson",
            "js",
            "biome",
            "2.5.15-1",
        ),
        (
            "semgrep-json",
            Carrier::SemgrepJson,
            Converter::SemgrepJson,
            "opengrep.json",
            "js",
            "opengrep",
            "1.31.0-1",
        ),
    ] {
        assert_eq!(Carrier::from_name(name), Some(format), "{name}");
        let root = probe(dir);
        let carried = carry(format, &raw(raw_name), tool, version, &root).unwrap();
        assert_eq!(
            carried,
            convert(converter, &raw(raw_name), tool, version, &root).unwrap(),
            "{name}: the same SARIF"
        );
        assert!(!results(&document(&carried)).is_empty(), "{name}: findings");
    }
    assert_eq!(Carrier::from_name("ruff-sarif"), None, "an unknown format");
}
