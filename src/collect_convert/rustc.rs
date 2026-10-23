//! rustc's and Clippy's JSON diagnostics (`cargo clippy
//! --message-format=json`, one JSON object per line, or rustc's own
//! `--error-format=json`), converted to SARIF findings with their
//! `MachineApplicable` suggestions in the byte-region carrier.
//!
//! `collect` does not run Clippy: it needs the project's build, so it runs
//! on the client (D-LX-08), where `sloptopop local evidence --rerun`
//! (LX-CAR-4) will run it. This converter reads its recorded output.
//!
//! Positions: a span's `byte_start` and `byte_end` are bytes of the file as
//! committed: rustc maps them back over its CRLF normalisation and the
//! byte order mark it skips (the LX-CAR-1 probe: a byte order mark moves
//! them by 3, CRLF by one byte per line). `column_start` counts characters
//! and is only the finding's column.
//!
//! Which suggestions: a diagnostic's child (`help`) whose spans carry
//! `suggested_replacement` is one fix, its spans the edits. Only a fix
//! whose every span says `MachineApplicable` is carried (`safe`: what
//! `cargo fix` and `cargo clippy --fix` apply); `MaybeIncorrect`,
//! `HasPlaceholders` and `Unspecified` ones are dropped as unconverted,
//! and several machine-applicable children of one diagnostic are
//! alternatives.
//!
//! Standards: SCF TDA-12 (Input Data Validation), SCF TDA-17.1 (Static Code
//! Analysis: each finding keeps its tool's rule and its fix).

use std::collections::BTreeSet;

use serde_json::Value;

use super::fix::{Drop, Edit, Files, Fix, Level, one_per_level};
use super::native::Context;
use super::{Finding, items, number, relative, text};

/// The rustc diagnostic one line of output holds, if it holds one.
fn diagnostic(line: &Value) -> Option<&Value> {
    match line.get("reason").and_then(Value::as_str) {
        Some("compiler-message") => line.get("message"),
        Some(_) => None,
        None => line.get("spans").is_some().then_some(line),
    }
}

/// The SARIF level of a rustc level.
fn level(rustc: &str) -> &'static str {
    match rustc {
        "error" | "error: internal compiler error" => "error",
        "warning" => "warning",
        _ => "note",
    }
}

/// Every diagnostic with a lint or error code in `output`, with its fix.
pub(super) fn findings(
    output: &str,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Result<BTreeSet<Finding>, String> {
    let mut findings = BTreeSet::new();
    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let line: Value = serde_json::from_str(line)
            .map_err(|e| format!("rustc printed no readable JSON on line {}: {e}", index + 1))?;
        let Some(message) = diagnostic(&line) else {
            continue;
        };
        let rule = message
            .pointer("/code/code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if rule.is_empty() {
            continue;
        }
        let spans = message.get("spans").map_or(&[][..], items);
        let primary = spans
            .iter()
            .find(|span| span.get("is_primary").and_then(Value::as_bool) == Some(true))
            .unwrap_or(&Value::Null);
        let mut fixes = Vec::new();
        for child in message.get("children").map_or(&[][..], items) {
            fixes.extend(fix(&rule, child, context, store));
        }
        findings.insert(Finding {
            uri: relative(&text(primary, "file_name")),
            line: number(primary, "line_start"),
            column: number(primary, "column_start"),
            level: level(&text(message, "level")),
            message: text(message, "message"),
            fixes: one_per_level(store, &rule, fixes),
            rule,
        });
    }
    Ok(findings)
}

/// The fix `child`'s suggestions make, if it has any and they convert.
fn fix(rule: &str, child: &Value, context: &Context<'_>, store: &mut Files<'_>) -> Option<Fix> {
    let suggested: Vec<&Value> = child
        .get("spans")
        .map_or(&[][..], items)
        .iter()
        .filter(|span| {
            span.get("suggested_replacement")
                .is_some_and(Value::is_string)
        })
        .collect();
    let first = suggested.first()?;
    let path = relative(&text(first, "file_name"));
    let files: BTreeSet<String> = suggested
        .iter()
        .map(|span| text(span, "file_name"))
        .collect();
    let applicability: BTreeSet<String> = suggested
        .iter()
        .map(|span| text(span, "suggestion_applicability"))
        .collect();
    if files.len() > 1 {
        store.drop(rule, &path, Drop::MultiFile);
        return None;
    }
    if applicability.len() != 1 || !applicability.contains("MachineApplicable") {
        store.drop(rule, &path, Drop::Unconverted);
        return None;
    }
    store.keep(rule, &path, |source| {
        let edits = suggested
            .iter()
            .map(|span| {
                Ok(Edit {
                    start: source.byte(number(span, "byte_start"))?,
                    end: source.byte(number(span, "byte_end"))?,
                    text: text(span, "suggested_replacement"),
                })
            })
            .collect::<Result<Vec<_>, Drop>>()?;
        Ok(context.fix(
            rule,
            Level::Safe,
            "MachineApplicable",
            &text(child, "message"),
            "rustc-json",
            &path,
            edits,
        ))
    })
}

#[cfg(test)]
mod tests;
