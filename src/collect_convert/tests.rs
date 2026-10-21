//! Unit tests for [`super`], on outputs the real tools printed for
//! sloptopop's synthetic fixtures (trimmed).

use serde_json::{Value, json};

use super::*;

fn results(sarif: &str) -> Vec<Value> {
    let document: Value = serde_json::from_str(sarif).unwrap();
    assert_eq!(document["version"], "2.1.0");
    document["runs"][0]["results"].as_array().unwrap().clone()
}

fn location(result: &Value) -> (String, u64, Option<u64>) {
    let physical = &result["locations"][0]["physicalLocation"];
    (
        physical["artifactLocation"]["uri"]
            .as_str()
            .unwrap()
            .to_owned(),
        physical["region"]["startLine"].as_u64().unwrap(),
        physical["region"]["startColumn"].as_u64(),
    )
}

#[test]
fn scorecard_reports_only_a_missing_licence() {
    let raw = br#"{"date":"2026-10-21T06:29:25Z","repo":{"name":"file://.","commit":"unknown"},"score":0.0,"checks":[{"details":null,"score":0,"reason":"license file not detected","name":"License"},{"score":10,"reason":"fine","name":"License"},{"score":0,"reason":"x","name":"Pinned-Dependencies"}],"metadata":null}"#;
    let sarif = convert(Converter::Scorecard, raw, "scorecard-license", "5.6.0-1").unwrap();
    let found = results(&sarif);
    assert_eq!(found.len(), 1, "{sarif}");
    assert_eq!(found[0]["ruleId"], "License");
    assert_eq!(found[0]["level"], "warning");
    assert_eq!(found[0]["message"]["text"], "license file not detected");
    assert!(found[0].get("locations").is_none(), "about the repository");
    let document: Value = serde_json::from_str(&sarif).unwrap();
    let driver = &document["runs"][0]["tool"]["driver"];
    assert_eq!(driver["name"], "scorecard-license");
    assert_eq!(driver["version"], "5.6.0-1");
    assert_eq!(driver["rules"][0]["id"], "License");
    assert!(!sarif.contains("2026-10-21"), "no timestamps: {sarif}");
    assert!(sarif.ends_with("}\n"), "a final newline");
}

#[test]
fn phpcs_messages_keep_their_sniff_and_line() {
    let raw = br#"{"totals":{"errors":2,"warnings":1,"fixable":0},"files":{"/src/src/UserStore.php":{"errors":2,"warnings":1,"messages":[
        {"message":"Class name doesn't match filename; expected \"class UserStore\"","source":"Squiz.Classes.ClassFileName.NoMatch","severity":5,"fixable":false,"type":"ERROR","line":2,"column":1},
        {"message":"Empty CATCH statement detected","source":"Generic.CodeAnalysis.EmptyStatement.DetectedCatch","severity":5,"fixable":false,"type":"WARNING","line":8,"column":11}]},
        "./lib/Other.php":{"errors":0,"warnings":0,"messages":[]}}}"#;
    let found = results(&convert(Converter::Phpcs, raw, "phpcs", "4.0.4-1").unwrap());
    assert_eq!(found.len(), 2);
    assert_eq!(found[0]["ruleId"], "Squiz.Classes.ClassFileName.NoMatch");
    assert_eq!(found[0]["level"], "error");
    assert_eq!(
        location(&found[0]),
        ("src/UserStore.php".to_owned(), 2, Some(1))
    );
    assert_eq!(found[1]["level"], "warning");
    assert_eq!(
        location(&found[1]),
        ("src/UserStore.php".to_owned(), 8, Some(11))
    );
}

#[test]
fn pylint_levels_and_one_based_columns() {
    let raw = br#"{"messages":[
        {"type":"refactor","symbol":"cyclic-import","message":"Cyclic import (orders.models -> orders.service)","messageId":"R0401","line":1,"column":0,"path":"orders/service.py"},
        {"type":"fatal","symbol":"parse-error","message":"boom","line":3,"column":4,"path":"./x.py"},
        {"type":"info","symbol":"note","message":"fyi","line":0,"column":0,"path":""}],"statistics":{}}"#;
    let found = results(&convert(Converter::Pylint, raw, "pylint", "4.1.3-1").unwrap());
    let by_rule = |rule: &str| found.iter().find(|r| r["ruleId"] == rule).unwrap().clone();
    let cyclic = by_rule("cyclic-import");
    assert_eq!(cyclic["level"], "warning");
    assert_eq!(
        location(&cyclic),
        ("orders/service.py".to_owned(), 1, Some(1))
    );
    let fatal = by_rule("parse-error");
    assert_eq!(fatal["level"], "error");
    assert_eq!(location(&fatal), ("x.py".to_owned(), 3, Some(5)));
    let info = by_rule("note");
    assert_eq!(info["level"], "note");
    assert!(info.get("locations").is_none(), "no path, no location");
    for (kind, level) in [
        ("error", "error"),
        ("warning", "warning"),
        ("convention", "warning"),
    ] {
        let one = format!(
            r#"{{"messages":[{{"type":"{kind}","symbol":"s","message":"m","line":2,"column":0,"path":"a.py"}}]}}"#
        );
        let levels = results(&convert(Converter::Pylint, one.as_bytes(), "pylint", "v").unwrap());
        assert_eq!(levels[0]["level"], level, "{kind}");
    }
}

#[test]
fn depcruise_violations_name_their_cycle() {
    let raw = br#"{"modules":[],"summary":{"violations":[
        {"type":"cycle","from":"src/format.ts","to":"src/invoice.ts","rule":{"severity":"warn","name":"no-circular"},
         "cycle":[{"name":"src/invoice.ts","dependencyTypes":["local"]},{"name":"src/format.ts","dependencyTypes":["local"]}]},
        {"type":"dependency","from":"src/a.ts","to":"src/b.ts","rule":{"severity":"error","name":"no-x"}},
        {"type":"dependency","from":"src/c.ts","to":"src/d.ts","rule":{"severity":"info","name":"no-y"},"cycle":["src/d.ts"]}]}}"#;
    let found =
        results(&convert(Converter::Depcruise, raw, "dependency-cruiser", "18.6.0-1").unwrap());
    let by_rule = |rule: &str| found.iter().find(|r| r["ruleId"] == rule).unwrap().clone();
    let cycle = by_rule("no-circular");
    assert_eq!(cycle["level"], "warning");
    assert_eq!(
        cycle["message"]["text"],
        "src/format.ts → src/invoice.ts → src/format.ts"
    );
    assert_eq!(location(&cycle), ("src/format.ts".to_owned(), 1, None));
    let plain = by_rule("no-x");
    assert_eq!(plain["level"], "error");
    assert_eq!(plain["message"]["text"], "src/a.ts → src/b.ts");
    let info = by_rule("no-y");
    assert_eq!(info["level"], "note");
    assert_eq!(info["message"]["text"], "src/c.ts → src/d.ts");
}

#[test]
fn clang_tidy_keeps_only_requested_checks() {
    let raw = b"# sloptopop-clang-tidy: no compile database, files parsed as -std=c++17
/src/src/main.cpp:1:10: error: 'stdexcept' file not found [clang-diagnostic-error]
    1 | #include <stdexcept>
/src/src/main.cpp:7:5: warning: empty catch statements hide issues; to handle exceptions appropriately, consider re-throwing, handling, or avoiding catch altogether [bugprone-empty-catch]
/src/src/main.cpp:7:5: warning: empty catch statements hide issues; to handle exceptions appropriately, consider re-throwing, handling, or avoiding catch altogether [bugprone-empty-catch]
./b.cc:3:1: error: bad thing [bugprone-x,cert-y]
not a diagnostic [bugprone-empty-catch]
no checks]
a.cc: warning: m [bugprone-empty-catch]
a.cc:2: warning: m [bugprone-empty-catch]
a.cc:x:1: warning: m [bugprone-empty-catch]
a.cc:1:y: warning: m [bugprone-empty-catch]
a.cc:2:3: note: m [bugprone-empty-catch]
:2:3: warning: m [bugprone-empty-catch]
";
    let found = results(&convert(Converter::ClangTidy, raw, "clang-tidy", "19.1.0-1").unwrap());
    // Sorted by file: the one without a path first; duplicates dropped.
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].get("locations").is_none(), "no path, no location");
    assert_eq!(found[1]["ruleId"], "bugprone-x", "the first check named");
    assert_eq!(found[1]["level"], "error");
    assert_eq!(location(&found[1]), ("b.cc".to_owned(), 3, Some(1)));
    assert_eq!(found[2]["ruleId"], "bugprone-empty-catch");
    assert_eq!(found[2]["level"], "warning");
    assert_eq!(location(&found[2]), ("src/main.cpp".to_owned(), 7, Some(5)));
    assert!(
        found[2]["message"]["text"]
            .as_str()
            .unwrap()
            .starts_with("empty catch statements hide issues"),
        "{found:?}"
    );
}

#[test]
fn unreadable_output_is_an_error_never_an_empty_report() {
    for converter in [
        Converter::Scorecard,
        Converter::Phpcs,
        Converter::Pylint,
        Converter::Depcruise,
    ] {
        let error = convert(converter, b"not json", "tool", "v").unwrap_err();
        assert!(
            error.starts_with("tool printed no readable JSON"),
            "{error}"
        );
        let shapeless = convert(converter, b"{}", "tool", "v").unwrap_err();
        assert!(shapeless.contains(": no "), "{converter:?}: {shapeless}");
    }
    let phpcs = convert(Converter::Phpcs, br#"{"files":{"a.php":{}}}"#, "phpcs", "v").unwrap_err();
    assert_eq!(phpcs, "PHP_CodeSniffer JSON: no messages array");
    let depcruise = convert(Converter::Depcruise, br#"{"summary":{}}"#, "d", "v").unwrap_err();
    assert_eq!(depcruise, "dependency-cruiser JSON: no violations array");
}

#[test]
fn normalizing_drops_times_and_help_and_sorts_rules() {
    let raw = br#"{"version":"2.1.0","runs":[{
        "tool":{"driver":{"name":"staticcheck","rules":[
            {"id":"SA4026","help":{"text":"b"}},
            {"id":"SA4003","help":{"text":"a"}},
            "not an object"]},
            "extensions":[{"name":"x","rules":[{"id":"E1","help":{"text":"e"}}]}]},
        "invocations":[{"startTimeUtc":"2026-10-21T07:40:50Z","endTimeUtc":"2026-10-21T07:41:00Z","executionSuccessful":true},"x"],
        "results":[{"ruleId":"SA4003","ruleIndex":1,"rule":{"index":1}},{"ruleId":"SA4026","ruleIndex":0},{"ruleId":"Q","ruleIndex":9},{"ruleId":"R"}]}]}"#;
    let sarif = normalize(raw, "staticcheck", "v").unwrap();
    let document: Value = serde_json::from_str(&sarif).unwrap();
    let run = &document["runs"][0];
    let rules = run["tool"]["driver"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0], "not an object",
        "a rule without an id sorts first"
    );
    assert_eq!(rules[1]["id"], "SA4003");
    assert_eq!(rules[2]["id"], "SA4026");
    assert!(
        rules[1].get("help").is_none() && rules[2].get("help").is_none(),
        "{sarif}"
    );
    assert!(
        run["tool"]["extensions"][0]["rules"][0]
            .get("help")
            .is_none()
    );
    assert_eq!(
        run["invocations"][0],
        json!({ "executionSuccessful": true })
    );
    assert_eq!(run["invocations"][1], "x");
    assert_eq!(
        run["results"][0]["ruleIndex"], 1,
        "SA4003 moved from 1 to 1"
    );
    assert_eq!(run["results"][0]["rule"]["index"], 1);
    assert_eq!(
        run["results"][1]["ruleIndex"], 2,
        "SA4026 moved from 0 to 2"
    );
    assert_eq!(
        run["results"][2]["ruleIndex"], 9,
        "an index out of range stays"
    );
    assert!(sarif.ends_with("}\n"));
    assert_eq!(
        normalize(sarif.as_bytes(), "staticcheck", "v").unwrap(),
        sarif,
        "idempotent"
    );
}

#[test]
fn paths_under_the_container_root_become_relative() {
    let raw = br#"{"version":"2.1.0","runs":[{
        "tool":{"driver":{"name":"eslint"}},
        "originalUriBaseIds":{"ROOT":{"uri":"file:///src/"},"KEEP":{"uri":"file:///elsewhere/"},"HERE":{"uri":"file:///src"}},
        "artifacts":[{"location":{"uri":"file:///src/src/a.ts"}},{"location":{"uri":"src/b.ts","uriBaseId":"ROOT"}},{"location":{"uri":"c.ts","uriBaseId":"KEEP"}}],
        "results":[{"ruleId":"r","locations":[{"physicalLocation":{"artifactLocation":{"uri":"/src/src/d.ts","index":0}}}]},
                   {"ruleId":"s","locations":[{"physicalLocation":{"artifactLocation":{"uri":"/src"}}}]},
                   {"ruleId":"t","properties":{"note":1,"flag":true,"nothing":null,"uri":7}}]}]}"#;
    let document: Value = serde_json::from_str(&normalize(raw, "eslint", "v").unwrap()).unwrap();
    let run = &document["runs"][0];
    assert_eq!(
        run["originalUriBaseIds"],
        json!({ "KEEP": { "uri": "file:///elsewhere/" } })
    );
    assert_eq!(run["artifacts"][0]["location"]["uri"], "src/a.ts");
    assert_eq!(
        run["artifacts"][1]["location"],
        json!({ "uri": "src/b.ts" }),
        "base dropped"
    );
    assert_eq!(run["artifacts"][2]["location"]["uriBaseId"], "KEEP");
    let uri = |i: usize| {
        run["results"][i]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"].clone()
    };
    assert_eq!(uri(0), "src/d.ts");
    assert_eq!(uri(1), ".", "the root itself");
    assert_eq!(
        run["results"][2]["properties"]["uri"], 7,
        "only strings are paths"
    );
    let plain = normalize(
        br#"{"runs":[{"tool":{"driver":{"name":"t"}},"originalUriBaseIds":"odd"}]}"#,
        "t",
        "v",
    )
    .unwrap();
    assert!(plain.contains("\"originalUriBaseIds\": \"odd\""), "{plain}");
}

#[test]
fn compiler_analyzer_timings_are_dropped() {
    let raw = br#"{"version":"2.1.0","runs":[{
        "properties":{"analyzerExecutionTime":"1.552","kept":1},
        "tool":{"driver":{"name":"Microsoft (R) Visual C# Compiler","rules":[
            {"id":"S108","properties":{"category":"Major Code Smell","executionTimeInSeconds":"0.008","executionTimeInPercentage":"<1"}},
            {"id":"S2486","properties":"odd"}]}}}]}"#;
    let document: Value =
        serde_json::from_str(&normalize(raw, "sonaranalyzer-csharp", "v").unwrap()).unwrap();
    let run = &document["runs"][0];
    assert_eq!(run["properties"], json!({ "kept": 1 }));
    assert_eq!(
        run["tool"]["driver"]["rules"][0]["properties"],
        json!({ "category": "Major Code Smell" })
    );
    assert_eq!(run["tool"]["driver"]["rules"][1]["properties"], "odd");
}

#[test]
fn normalizing_handles_runs_without_rules_and_no_output() {
    let bare = normalize(br#"{"runs":[{"tool":{"driver":{"name":"t"}}}]}"#, "t", "v").unwrap();
    assert!(bare.contains("\"name\": \"t\""), "{bare}");
    let empty = normalize(b"  \n", "osv-scanner", "2.7.0-1").unwrap();
    assert_eq!(results(&empty).len(), 0);
    assert!(
        empty.contains("osv-scanner") && empty.contains("2.7.0-1"),
        "{empty}"
    );
    let error = normalize(br#"{"version":"2.1.0"}"#, "bandit", "v").unwrap_err();
    assert_eq!(error, "bandit SARIF: no runs array");
    let unreadable = normalize(b"{", "bandit", "v").unwrap_err();
    assert!(
        unreadable.starts_with("bandit wrote no readable SARIF"),
        "{unreadable}"
    );
}
