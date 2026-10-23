//! The fix-carrying outputs of Ruff (`--output-format json`), `ESLint`
//! (`json-with-metadata`), Biome (`--reporter=rdjson`) and Opengrep or
//! Semgrep (`--json`), converted to SARIF findings with their fixes in the
//! byte-region carrier ([`super::fix`]). Each tool's position convention is
//! applied exactly as the tool's own source and the WP-21 probe show:
//! - Ruff: 1-based rows and columns in code points, the byte order mark not
//!   a column;
//! - `ESLint`: `range` in UTF-16 code units of the text without its byte
//!   order mark;
//! - Biome: 1-based lines and columns in code points, the byte order mark
//!   column 1 of line 1;
//! - Opengrep and Semgrep: byte offsets.
//!
//! Standards: SCF TDA-12 (Input Data Validation), SCF TDA-17.1 (Static Code
//! Analysis: each finding keeps its tool's rule and its fix), SCF VPM-06
//! (Vulnerability Remediation Process: a finding carries its remediation).

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::fix::{Columns, Drop, Edit, Files, Fix, Level, Source, one_per_level};
use super::reviewed::{Guard, biome_kind, promoting_config, reviewed};
use super::{Finding, array, items, number, relative, text};

/// What one converter needs besides the tool's output.
#[derive(Debug)]
pub(super) struct Context<'r> {
    /// The catalogue name of the tool.
    pub(super) tool: &'r str,
    /// Its image version.
    pub(super) version: &'r str,
    /// The repository root the tool read.
    pub(super) root: &'r Path,
}

impl Context<'_> {
    /// Whether the project's configuration may change `guard`'s fix
    /// safety.
    fn promotes(&self, guard: Guard) -> bool {
        promoting_config(self.root, guard).is_some()
    }

    /// A fix of `level` for `rule`, its review looked up.
    #[expect(
        clippy::too_many_arguments,
        reason = "one argument per field of the fix the caller decides"
    )]
    pub(super) fn fix(
        &self,
        rule: &str,
        level: Level,
        tool_level: &str,
        description: &str,
        source: &'static str,
        path: &str,
        edits: Vec<Edit>,
    ) -> Fix {
        Fix {
            level,
            tool_level: tool_level.to_owned(),
            description: description.to_owned(),
            source,
            path: path.to_owned(),
            edits,
            reviewed: reviewed(self.tool, self.version, rule, level),
        }
    }
}

/// A position object's member as `u64` (0 when absent).
fn at(value: &Value, object: &str, member: &str) -> u64 {
    value.get(object).map_or(0, |object| number(object, member))
}

/// The SARIF level of a Ruff severity.
fn ruff_level(severity: &str) -> &'static str {
    match severity {
        "error" | "fatal" => "error",
        "warning" => "warning",
        _ => "note",
    }
}

/// Ruff's JSON: every diagnostic, with its fix.
pub(super) fn ruff(
    document: &Value,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let diagnostics = document
        .as_array()
        .ok_or_else(|| "Ruff JSON: not an array".to_owned())?;
    let promotes = context.promotes(Guard::Ruff);
    let mut findings = BTreeSet::new();
    for diagnostic in diagnostics {
        let code = text(diagnostic, "code");
        let rule = if code.is_empty() {
            text(diagnostic, "name")
        } else {
            code
        };
        let uri = relative(&text(diagnostic, "filename"));
        let mut fixes = Vec::new();
        if let Some(fix) = diagnostic.get("fix").filter(|fix| fix.is_object()) {
            let tool_level = text(fix, "applicability");
            let claimed = match tool_level.as_str() {
                "safe" => Some(Level::Safe),
                "unsafe" => Some(Level::Unsafe),
                "displayonly" => Some(Level::Suggestion),
                _ => None,
            };
            let decided = match claimed {
                _ if !diagnostic["cell"].is_null() => Err(Drop::Notebook),
                None => Err(Drop::Unconverted),
                Some(Level::Safe) if promotes => Err(Drop::Config),
                Some(level) => Ok(level),
            };
            match decided {
                Ok(level) => {
                    let edits = fix.get("edits").map_or(&[][..], items);
                    let built = store.keep(&rule, &uri, |source| {
                        let edits = edits
                            .iter()
                            .map(|edit| {
                                let place = |member: &str| {
                                    source.line_column(
                                        Columns::CodePointsBomSkipped,
                                        at(edit, member, "row"),
                                        at(edit, member, "column"),
                                    )
                                };
                                Ok(Edit {
                                    start: place("location")?,
                                    end: place("end_location")?,
                                    text: text(edit, "content"),
                                })
                            })
                            .collect::<Result<Vec<_>, Drop>>()?;
                        Ok(context.fix(
                            &rule,
                            level,
                            &tool_level,
                            &text(fix, "message"),
                            "ruff-json",
                            &uri,
                            edits,
                        ))
                    });
                    fixes.extend(built);
                }
                Err(drop) => store.drop(&rule, &uri, drop),
            }
        }
        findings.insert(Finding {
            line: at(diagnostic, "location", "row"),
            column: at(diagnostic, "location", "column"),
            level: ruff_level(&text(diagnostic, "severity")),
            message: text(diagnostic, "message"),
            fixes,
            uri,
            rule,
        });
    }
    Ok(findings)
}

/// `ESLint`'s `json-with-metadata`: every message with a rule (a message
/// without one is `ESLint`'s own error, which is no finding), with its
/// autofix (safe) and its suggestion (one only; several are alternatives).
pub(super) fn eslint(
    document: &Value,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let mut findings = BTreeSet::new();
    for result in array(document, "results", "ESLint JSON")? {
        let uri = relative(&text(result, "filePath"));
        // `source` is the text ESLint read: a fix for any other text is stale.
        let stale = result
            .get("source")
            .and_then(Value::as_str)
            .is_some_and(|read| {
                store
                    .get(&uri)
                    .is_ok_and(|file| file.bytes() != read.as_bytes())
            });
        for message in result.get("messages").map_or(&[][..], items) {
            let Some(rule) = message.get("ruleId").and_then(Value::as_str) else {
                continue;
            };
            let mut fixes = Vec::new();
            let mut carry = |level, tool_level, description: String, fix: &Value| {
                if stale {
                    store.drop(rule, &uri, Drop::Stale);
                    return;
                }
                let range = fix.get("range").map_or(&[][..], items);
                let built = store.keep(rule, &uri, |source| {
                    let [start, end] = range else {
                        return Err(Drop::Placeholder);
                    };
                    let offset = |value: &Value| {
                        value
                            .as_i64()
                            .ok_or(Drop::Placeholder)
                            .and_then(|offset| source.utf16(offset))
                    };
                    Ok(context.fix(
                        rule,
                        level,
                        tool_level,
                        &description,
                        "eslint-json",
                        &uri,
                        vec![Edit {
                            start: offset(start)?,
                            end: offset(end)?,
                            text: text(fix, "text"),
                        }],
                    ))
                });
                fixes.extend(built);
            };
            if let Some(fix) = message.get("fix").filter(|fix| fix.is_object()) {
                carry(Level::Safe, "fix", String::new(), fix);
            }
            for suggestion in message.get("suggestions").map_or(&[][..], items) {
                let fix = suggestion.get("fix").unwrap_or(&Value::Null);
                carry(
                    Level::Suggestion,
                    "suggestion",
                    text(suggestion, "desc"),
                    fix,
                );
            }
            findings.insert(Finding {
                line: number(message, "line"),
                column: number(message, "column"),
                level: if number(message, "severity") == 2 {
                    "error"
                } else {
                    "warning"
                },
                message: text(message, "message"),
                fixes: one_per_level(store, rule, fixes),
                uri: uri.clone(),
                rule: rule.to_owned(),
            });
        }
    }
    Ok(findings)
}

/// The SARIF level of a Biome or Semgrep severity.
fn severity_level(severity: &str) -> &'static str {
    match severity.to_ascii_lowercase().as_str() {
        "error" | "fatal" | "critical" | "high" => "error",
        "warning" | "medium" => "warning",
        _ => "note",
    }
}

/// Biome's rdjson: every diagnostic, with its code suggestion; the fix
/// kind is Biome's own, from the pinned table, unless the project's
/// configuration sets one.
pub(super) fn biome(
    document: &Value,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let promotes = context.promotes(Guard::Biome);
    let mut findings = BTreeSet::new();
    for diagnostic in array(document, "diagnostics", "Biome rdjson")? {
        let rule = diagnostic
            .get("code")
            .map_or_else(String::new, |code| text(code, "value"));
        let location = diagnostic.get("location").unwrap_or(&Value::Null);
        let uri = relative(&text(location, "path"));
        let range = location.get("range").unwrap_or(&Value::Null);
        let mut fixes = Vec::new();
        for suggestion in diagnostic.get("suggestions").map_or(&[][..], items) {
            match biome_kind(context.version, &rule) {
                None => store.drop(&rule, &uri, Drop::Unconverted),
                Some(Level::Safe) if promotes => store.drop(&rule, &uri, Drop::Config),
                Some(level) => {
                    let place = suggestion.get("range").unwrap_or(&Value::Null);
                    let built = store.keep(&rule, &uri, |source: &Source| {
                        let point = |end: &str| {
                            source.line_column(
                                Columns::CodePointsBomCounted,
                                at(place, end, "line"),
                                at(place, end, "column"),
                            )
                        };
                        Ok(context.fix(
                            &rule,
                            level,
                            level.as_str(),
                            "",
                            "biome-rdjson",
                            &uri,
                            vec![Edit {
                                start: point("start")?,
                                end: point("end")?,
                                text: text(suggestion, "text"),
                            }],
                        ))
                    });
                    fixes.extend(built);
                }
            }
        }
        findings.insert(Finding {
            line: at(range, "start", "line"),
            column: at(range, "start", "column"),
            level: severity_level(&text(diagnostic, "severity")),
            message: text(diagnostic, "message"),
            fixes: one_per_level(store, &rule, fixes),
            uri,
            rule,
        });
    }
    Ok(findings)
}

/// Opengrep's or Semgrep's JSON: every result, with its rule's `fix`
/// (suggestion level: a rule's fix has no applicability).
pub(super) fn semgrep(
    document: &Value,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let mut findings = BTreeSet::new();
    for result in array(document, "results", "Opengrep JSON")? {
        let rule = text(result, "check_id");
        let uri = relative(&text(result, "path"));
        let extra = result.get("extra").unwrap_or(&Value::Null);
        let mut fixes = Vec::new();
        if let Some(replacement) = extra.get("fix").and_then(Value::as_str) {
            let built = store.keep(&rule, &uri, |source| {
                Ok(context.fix(
                    &rule,
                    Level::Suggestion,
                    "rule-fix",
                    "",
                    "semgrep-json",
                    &uri,
                    vec![Edit {
                        start: source.byte(at(result, "start", "offset"))?,
                        end: source.byte(at(result, "end", "offset"))?,
                        text: replacement.to_owned(),
                    }],
                ))
            });
            fixes.extend(built);
        }
        findings.insert(Finding {
            line: at(result, "start", "line"),
            column: at(result, "start", "col"),
            level: severity_level(&text(extra, "severity")),
            message: text(extra, "message"),
            fixes,
            uri,
            rule,
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests;
