//! Converting the output of analyzers that write no SARIF (`OpenSSF`
//! Scorecard, `PHP_CodeSniffer`, Pylint, dependency-cruiser, clang-tidy) into
//! SARIF 2.1.0, which is all the engine reads. The conversion keeps each
//! tool's own rule id and message, maps its severity to a SARIF level,
//! makes paths relative to the repository root, and is deterministic:
//! results sorted, duplicates dropped, nothing time- or machine-dependent.
//!
//! Standards: SCF TDA-12 (Input Data Validation: unreadable output is an
//! error, never an empty report), MON-09 (deterministic records).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// Where the repository is mounted in the container (absolute paths below
/// it become relative).
const ROOT: &str = "/src/";

/// A format `collect` converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Converter {
    /// `OpenSSF` Scorecard's JSON: a License check that found no licence
    /// file becomes one result about the repository.
    Scorecard,
    /// `PHP_CodeSniffer`'s JSON report.
    Phpcs,
    /// Pylint's `json2` output.
    Pylint,
    /// dependency-cruiser's JSON: each rule violation.
    Depcruise,
    /// clang-tidy's diagnostics (`file:line:col: level: message [check]`).
    ClangTidy,
}

/// One SARIF result, ordered for a deterministic report.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    uri: String,
    line: u64,
    column: u64,
    rule: String,
    level: &'static str,
    message: String,
}

/// `path` relative to the repository root, with `/` separators.
fn relative(path: &str) -> String {
    let path = path.strip_prefix(ROOT).unwrap_or(path);
    path.strip_prefix("./").unwrap_or(path).to_owned()
}

/// Converts `raw`, the output of `tool` at `version`, to a SARIF document.
pub(crate) fn convert(
    converter: Converter,
    raw: &[u8],
    tool: &str,
    version: &str,
) -> Result<String, String> {
    let json = || -> Result<Value, String> {
        serde_json::from_slice(raw).map_err(|e| format!("{tool} printed no readable JSON: {e}"))
    };
    let findings = match converter {
        Converter::ClangTidy => clang_tidy(&String::from_utf8_lossy(raw)),
        Converter::Scorecard => scorecard(&json()?)?,
        Converter::Phpcs => phpcs(&json()?)?,
        Converter::Pylint => pylint(&json()?)?,
        Converter::Depcruise => depcruise(&json()?)?,
    };
    Ok(sarif(tool, version, &findings))
}

/// The SARIF 2.1.0 document holding `findings` (sorted, unique).
fn sarif(tool: &str, version: &str, findings: &BTreeSet<Finding>) -> String {
    let rules: BTreeSet<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    let results: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut result = Map::new();
            result.insert("ruleId".to_owned(), f.rule.clone().into());
            result.insert("level".to_owned(), f.level.into());
            result.insert("message".to_owned(), json!({ "text": f.message }));
            if !f.uri.is_empty() {
                let mut region = Map::new();
                region.insert("startLine".to_owned(), f.line.max(1).into());
                if f.column > 0 {
                    region.insert("startColumn".to_owned(), f.column.into());
                }
                result.insert(
                    "locations".to_owned(),
                    json!([{
                        "physicalLocation": {
                            "artifactLocation": { "uri": f.uri },
                            "region": region,
                        }
                    }]),
                );
            }
            Value::Object(result)
        })
        .collect();
    let document = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": {
                "name": tool,
                "version": version,
                "rules": rules.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>(),
            }},
            "results": results,
        }],
    });
    format!("{document:#}\n")
}

/// The SARIF log `raw` that `tool` (at `version`) wrote, made comparable
/// from run to run and machine to machine, whatever the tool:
/// - paths under the container's `/src` become relative to the repository
///   root (and base URIs naming `/src` are dropped with their uses);
/// - the invocations' start and end times are dropped;
/// - the rules are sorted by id, the results' rule indexes following them;
/// - the rules' help texts are dropped: some tools (OSV-Scanner) print
///   their tables in a random order, and the engine reads only the short
///   description.
///
/// Everything else is kept. No output at all (OSV-Scanner finding no
/// packages) is a log without results.
pub(crate) fn normalize(raw: &[u8], tool: &str, version: &str) -> Result<String, String> {
    if raw.iter().all(u8::is_ascii_whitespace) {
        return Ok(sarif(tool, version, &BTreeSet::new()));
    }
    let mut document: Value =
        serde_json::from_slice(raw).map_err(|e| format!("{tool} wrote no readable SARIF: {e}"))?;
    let runs = document
        .get_mut("runs")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("{tool} SARIF: no runs array"))?;
    let strip = |rules: Option<&mut Value>| {
        for rule in rules.and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(rule) = rule.as_object_mut() {
                rule.remove("help");
                // The C# compiler's per-rule analyzer timings.
                if let Some(properties) = rule.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.remove("executionTimeInSeconds");
                    properties.remove("executionTimeInPercentage");
                }
            }
        }
    };
    for run in runs.iter_mut() {
        sort_rules(run);
        for invocation in run
            .get_mut("invocations")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(invocation) = invocation.as_object_mut() {
                invocation.remove("startTimeUtc");
                invocation.remove("endTimeUtc");
            }
        }
        if let Some(properties) = run.get_mut("properties").and_then(Value::as_object_mut) {
            properties.remove("analyzerExecutionTime");
        }
        strip(run.pointer_mut("/tool/driver/rules"));
        for extension in run
            .pointer_mut("/tool/extensions")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            strip(extension.get_mut("rules"));
        }
        let bases = drop_root_bases(run);
        relativize(run, &bases);
    }
    Ok(format!("{document:#}\n"))
}

/// `uri` relative to the repository root, if it names the container's
/// `/src` or a path below it.
fn below_root(uri: &str) -> Option<&str> {
    ["file:///src/", "/src/"]
        .iter()
        .find_map(|prefix| uri.strip_prefix(prefix))
        .or_else(|| ["file:///src", "/src"].contains(&uri).then_some(""))
}

/// Drops the run's `originalUriBaseIds` that name `/src` (or below it) and
/// returns their names.
fn drop_root_bases(run: &mut Value) -> BTreeSet<String> {
    let Some(bases) = run
        .get_mut("originalUriBaseIds")
        .and_then(Value::as_object_mut)
    else {
        return BTreeSet::new();
    };
    let dropped: BTreeSet<String> = bases
        .iter()
        .filter(|(_, base)| below_root(&text(base, "uri")).is_some())
        .map(|(name, _)| name.clone())
        .collect();
    bases.retain(|name, _| !dropped.contains(name));
    dropped
}

/// Rewrites every `uri` below `/src` as a relative path ("." for the root
/// itself), and drops each `uriBaseId` naming one of the dropped `bases`.
fn relativize(value: &mut Value, bases: &BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if object
                .get("uriBaseId")
                .and_then(Value::as_str)
                .is_some_and(|base| bases.contains(base))
            {
                object.remove("uriBaseId");
            }
            if let Some(Value::String(uri)) = object.get_mut("uri")
                && let Some(relative) = below_root(uri).map(str::to_owned)
            {
                *uri = if relative.is_empty() {
                    ".".to_owned()
                } else {
                    relative
                };
            }
            for child in object.values_mut() {
                relativize(child, bases);
            }
        }
        Value::Array(items) => {
            for item in items {
                relativize(item, bases);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Sorts the driver's rules by id and points every result's `ruleIndex`
/// (and `rule.index`) at the rule's new place.
fn sort_rules(run: &mut Value) {
    let Some(rules) = run
        .pointer_mut("/tool/driver/rules")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut order: Vec<(usize, Value)> = rules.drain(..).enumerate().collect();
    order.sort_by_key(|(_, rule)| text(rule, "id"));
    let moved: BTreeMap<u64, u64> = order
        .iter()
        .enumerate()
        .map(|(new, (old, _))| {
            (
                u64::try_from(*old).unwrap_or(u64::MAX),
                u64::try_from(new).unwrap_or(u64::MAX),
            )
        })
        .collect();
    rules.extend(order.into_iter().map(|(_, rule)| rule));
    let remap = |index: &mut Value| {
        let mapped = index.as_u64().and_then(|old| moved.get(&old));
        if let Some(new) = mapped {
            *index = (*new).into();
        }
    };
    for result in run
        .get_mut("results")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(index) = result.get_mut("ruleIndex") {
            remap(index);
        }
        if let Some(index) = result.pointer_mut("/rule/index") {
            remap(index);
        }
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}

fn array<'v>(value: &'v Value, key: &str, what: &str) -> Result<&'v Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{what}: no {key} array"))
}

/// Scorecard: only "license file not detected" (score 0) is a finding.
fn scorecard(document: &Value) -> Result<BTreeSet<Finding>, String> {
    Ok(array(document, "checks", "Scorecard JSON")?
        .iter()
        .filter(|check| {
            text(check, "name") == "License"
                && check.get("score").and_then(Value::as_i64) == Some(0)
        })
        .map(|check| Finding {
            uri: String::new(),
            line: 0,
            column: 0,
            rule: "License".to_owned(),
            level: "warning",
            message: text(check, "reason"),
        })
        .collect())
}

/// `PHP_CodeSniffer`: every message of every file.
fn phpcs(document: &Value) -> Result<BTreeSet<Finding>, String> {
    let files = document
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| "PHP_CodeSniffer JSON: no files object".to_owned())?;
    let mut findings = BTreeSet::new();
    for (path, file) in files {
        for message in array(file, "messages", "PHP_CodeSniffer JSON")? {
            findings.insert(Finding {
                uri: relative(path),
                line: number(message, "line"),
                column: number(message, "column"),
                rule: text(message, "source"),
                level: if text(message, "type") == "ERROR" {
                    "error"
                } else {
                    "warning"
                },
                message: text(message, "message"),
            });
        }
    }
    Ok(findings)
}

/// Pylint: every message; refactor, convention and warning messages are
/// warnings, error and fatal ones errors, anything else a note.
fn pylint(document: &Value) -> Result<BTreeSet<Finding>, String> {
    Ok(array(document, "messages", "Pylint JSON")?
        .iter()
        .map(|message| Finding {
            uri: relative(&text(message, "path")),
            line: number(message, "line"),
            column: number(message, "column") + 1,
            rule: text(message, "symbol"),
            level: match text(message, "type").as_str() {
                "error" | "fatal" => "error",
                "warning" | "refactor" | "convention" => "warning",
                _ => "note",
            },
            message: text(message, "message"),
        })
        .collect())
}

/// dependency-cruiser: every rule violation, at its `from` module.
fn depcruise(document: &Value) -> Result<BTreeSet<Finding>, String> {
    let summary = document
        .get("summary")
        .ok_or_else(|| "dependency-cruiser JSON: no summary".to_owned())?;
    Ok(array(summary, "violations", "dependency-cruiser JSON")?
        .iter()
        .map(|violation| {
            let rule = violation.get("rule").cloned().unwrap_or(Value::Null);
            let (from, to) = (text(violation, "from"), text(violation, "to"));
            let cycle: Vec<String> = violation
                .get("cycle")
                .and_then(Value::as_array)
                .map(|steps| {
                    steps
                        .iter()
                        .map(|step| {
                            step.as_str()
                                .map_or_else(|| text(step, "name"), str::to_owned)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let message = if cycle.is_empty() {
                format!("{from} → {to}")
            } else {
                format!("{from} → {}", cycle.join(" → "))
            };
            Finding {
                uri: relative(&from),
                line: 1,
                column: 0,
                rule: text(&rule, "name"),
                level: match text(&rule, "severity").as_str() {
                    "error" => "error",
                    "warn" => "warning",
                    _ => "note",
                },
                message,
            }
        })
        .collect())
}

/// clang-tidy: every diagnostic of a requested check (compiler
/// diagnostics, `clang-diagnostic-*`, are not findings).
fn clang_tidy(output: &str) -> BTreeSet<Finding> {
    output.lines().filter_map(diagnostic).collect()
}

fn diagnostic(line: &str) -> Option<Finding> {
    let (rest, checks) = line.strip_suffix(']')?.rsplit_once(" [")?;
    if checks.starts_with("clang-diagnostic") {
        return None;
    }
    let (location, message) = rest
        .split_once(": warning: ")
        .map(|(l, m)| (l, (m, "warning")))
        .or_else(|| rest.split_once(": error: ").map(|(l, m)| (l, (m, "error"))))?;
    let (file_line, column) = location.rsplit_once(':')?;
    let (path, line_number) = file_line.rsplit_once(':')?;
    let column = column.parse().ok()?;
    let line_number = line_number.parse().ok()?;
    Some(Finding {
        uri: relative(path),
        line: line_number,
        column,
        rule: checks.split(',').next().unwrap_or(checks).to_owned(),
        level: message.1,
        message: message.0.to_owned(),
    })
}

#[cfg(test)]
mod tests;
