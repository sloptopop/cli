//! `ShellCheck`'s `json1` reports (one per file, as the image's wrapper
//! prints them), converted to SARIF findings of the reported rules, with
//! each comment's `fix.replacements` in the byte-region carrier.
//!
//! Positions: 1-based lines and columns, the columns counted in Unicode
//! code points, a tab one column (`shellcheck.1` at v0.11.0: "tabs are
//! counted as 1 character"), the byte order mark column 1 of line 1. The
//! LX-CAR-1 probe confirmed all of it on `ShellCheck` 0.11.0: `é😀` before a
//! site moves it by 2 columns, not 6; a byte order mark moves line 1 by
//! one. A replacement's `insertionPoint` only orders insertions at one
//! point and its `precedence` only orders overlapping fixes of different
//! comments: within a fix, an insertion that touches another edit is
//! dropped as an overlap, so neither ever decides the text.
//!
//! Levels: `ShellCheck` has none; `shellcheck -f diff` applies every fix, so
//! each is `safe`, except an SC2006 fix whose backquoted text holds a
//! backslash: inside backquotes `\$`, `` \` `` and `\\` lose their
//! backslash before the command runs, inside `$(…)` they keep it, so that
//! fix changes what the command does and is `unsafe`.
//!
//! Standards: SCF TDA-12 (Input Data Validation), SCF TDA-17.1 (Static Code
//! Analysis: each finding keeps its tool's rule and its fix).

use std::collections::BTreeSet;

use serde_json::Value;

use super::defect::Rule;
use super::fix::{Columns, Drop, Edit, Files, Level, Source};
use super::native::Context;
use super::{Finding, items, number, relative, text};

/// The wrapper's first line.
const HEADER: &str = "# sloptopop-shellcheck: ";

/// The SARIF level of a `ShellCheck` level.
fn level(shellcheck: &str) -> &'static str {
    match shellcheck {
        "error" => "error",
        "warning" => "warning",
        _ => "note",
    }
}

/// Every comment of a reported rule in `output`, with its fix.
pub(super) fn findings(
    output: &str,
    rules: &[Rule],
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let body = match output.split_once('\n') {
        Some((first, rest)) if first.starts_with(HEADER) => rest,
        _ if output.starts_with(HEADER) => "",
        _ => output,
    };
    let mut findings = BTreeSet::new();
    for report in serde_json::Deserializer::from_str(body).into_iter::<Value>() {
        let report = report.map_err(|e| format!("ShellCheck printed no readable json1: {e}"))?;
        let comments = report
            .get("comments")
            .and_then(Value::as_array)
            .ok_or_else(|| "ShellCheck json1: no comments array".to_owned())?;
        for comment in comments {
            let rule = format!("SC{}", number(comment, "code"));
            if Rule::reported(rules, &rule).is_none() {
                continue;
            }
            let uri = relative(&text(comment, "file"));
            let replacements = comment.pointer("/fix/replacements").map_or(&[][..], items);
            let fixes = if replacements.is_empty() {
                Vec::new()
            } else {
                store
                    .keep(&rule, &uri, |source| {
                        let edits = replacements
                            .iter()
                            .map(|replacement| edit(source, replacement))
                            .collect::<Result<Vec<_>, Drop>>()?;
                        if touching_insertion(&edits) {
                            return Err(Drop::Overlap);
                        }
                        let level = if rule == "SC2006" && backslash_inside(source, &edits) {
                            Level::Unsafe
                        } else {
                            Level::Safe
                        };
                        Ok(context.fix(&rule, level, "fix", "", "shellcheck-json1", &uri, edits))
                    })
                    .into_iter()
                    .collect()
            };
            findings.insert(Finding {
                line: number(comment, "line"),
                column: number(comment, "column"),
                level: level(&text(comment, "level")),
                message: text(comment, "message"),
                fixes,
                uri,
                rule,
            });
        }
    }
    Ok(findings)
}

/// One replacement in byte offsets.
fn edit(source: &Source, replacement: &Value) -> Result<Edit, Drop> {
    let place = |line: &str, column: &str| {
        source.line_column(
            Columns::CodePointsBomCounted,
            number(replacement, line),
            number(replacement, column),
        )
    };
    Ok(Edit {
        start: place("line", "column")?,
        end: place("endLine", "endColumn")?,
        text: text(replacement, "replacement"),
    })
}

/// Whether an insertion shares a point with another edit of the fix, where
/// `ShellCheck`'s `insertionPoint` would decide the order.
fn touching_insertion(edits: &[Edit]) -> bool {
    edits.iter().enumerate().any(|(i, insertion)| {
        insertion.start == insertion.end
            && edits.iter().enumerate().any(|(j, other)| {
                i != j && (other.start == insertion.start || other.end == insertion.start)
            })
    })
}

/// Whether the text between the fix's first and last edit holds a
/// backslash.
fn backslash_inside(source: &Source, edits: &[Edit]) -> bool {
    let from = edits.first().map_or(0, |edit| edit.start);
    let to = edits.last().map_or(0, |edit| edit.end);
    source
        .bytes()
        .get(from..to)
        .is_some_and(|span| span.contains(&b'\\'))
}

#[cfg(test)]
mod tests;
