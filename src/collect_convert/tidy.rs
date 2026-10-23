//! clang-tidy's exported fixes (`--export-fixes`, the YAML the image's
//! wrapper prints after the diagnostics), put on the findings they belong
//! to in the byte-region carrier.
//!
//! Positions: `Offset` and `Length` are bytes of the file as the compiler
//! read it, the byte order mark included (the LX-CAR-1 probe: a byte order
//! mark moves every offset by 3, CRLF by one byte per line, `é😀` by 6).
//! A diagnostic's `FileOffset` is turned into the line and byte column
//! clang prints, which is how a fix finds its finding.
//!
//! Which fixes: those of the reported checks that the reviewed fix table
//! lists (a check that is only reported keeps its findings, and its fixes
//! are dropped as unreviewed), and only for a diagnostic the report printed
//! (a finding at the same file, line, column and check). A warning's
//! replacements are what `clang-tidy --fix` applies (`safe`, described by
//! the diagnostic's message); a note's are applied only with `--fix-notes`
//! (`unsafe`, described by the note), and several notes with replacements
//! are alternatives. A fix whose replacements name more than one file is
//! dropped. The paths are the mounted checkout's (`/src`), or the host
//! checkout's where a compile database names it.
//!
//! Standards: SCF TDA-12 (Input Data Validation), SCF CHG-03 (only reviewed
//! checks' fixes are carried).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::defect::Rule;
use super::fix::{Drop, Edit, Files, Fix, Level, one_per_level};
use super::native::Context;
use super::reviewed::reviewed_rule;
use super::{Finding, items, relative, text, yaml};

/// The line the wrapper prints before the exported fixes.
pub(super) const MARKER: &str = "# sloptopop-clang-tidy: exported fixes\n";

/// Where a finding is: file, line, column, rule.
type Key = (String, u64, u64, String);

/// The diagnostics before the wrapper's marker line, and the exported
/// fixes after it (none without one).
pub(super) fn split(body: &str) -> (&str, &str) {
    if let Some(exported) = body.strip_prefix(MARKER) {
        return ("", exported);
    }
    body.rsplit_once(&format!("\n{MARKER}"))
        .unwrap_or((body, ""))
}

/// `findings` with the fixes `exported` holds for them.
pub(super) fn attach(
    findings: BTreeSet<Finding>,
    exported: &str,
    rules: &[Rule],
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> BTreeSet<Finding> {
    if exported.trim().is_empty() {
        return findings;
    }
    let Ok(documents) = yaml::documents(exported) else {
        store.drop("", "", Drop::Unconverted);
        return findings;
    };
    let wanted: BTreeSet<Key> = findings.iter().map(key).collect();
    let mut found: BTreeMap<Key, BTreeSet<Fix>> = BTreeMap::new();
    for diagnostic in documents
        .iter()
        .flat_map(|document| document.get("Diagnostics").map_or(&[][..], items))
    {
        let rule = text(diagnostic, "DiagnosticName");
        if Rule::reported(rules, &rule).is_none() || text(diagnostic, "Level") != "Warning" {
            continue;
        }
        let message = diagnostic.get("DiagnosticMessage").unwrap_or(&Value::Null);
        let path = path(&text(message, "FilePath"), context);
        let mut candidates = Vec::new();
        if replacements(message).next().is_some() {
            candidates.push((Level::Safe, "warning", text(message, "Message"), message));
        }
        for note in diagnostic.get("Notes").map_or(&[][..], items) {
            if replacements(note).next().is_some() {
                candidates.push((Level::Unsafe, "note", text(note, "Message"), note));
            }
        }
        if candidates.is_empty() {
            continue;
        }
        let Some((line, column)) = store
            .get(&path)
            .ok()
            .and_then(|source| source.position(offset(message, "FileOffset")))
        else {
            store.drop(&rule, &path, Drop::Path);
            continue;
        };
        let key = (path, line, column, rule.clone());
        if !wanted.contains(&key) {
            continue;
        }
        if !reviewed_rule(context.tool, context.version, &rule) {
            store.drop(&rule, &key.0, Drop::Unreviewed);
            continue;
        }
        let fixes = found.entry(key).or_default();
        for (level, tool_level, description, holder) in candidates {
            fixes.extend(fix(
                &rule,
                level,
                tool_level,
                &description,
                holder,
                context,
                store,
            ));
        }
    }
    findings
        .into_iter()
        .map(|finding| match found.remove(&key(&finding)) {
            Some(fixes) => Finding {
                fixes: one_per_level(store, &finding.rule, fixes.into_iter().collect()),
                ..finding
            },
            None => finding,
        })
        .collect()
}

/// Where `finding` is.
fn key(finding: &Finding) -> Key {
    (
        finding.uri.clone(),
        finding.line,
        finding.column,
        finding.rule.clone(),
    )
}

/// The replacements a diagnostic message or note holds.
fn replacements(holder: &Value) -> impl Iterator<Item = &Value> {
    holder.get("Replacements").map_or(&[][..], items).iter()
}

/// A number the YAML holds as text (0 when it is not one).
fn offset(value: &Value, key: &str) -> u64 {
    text(value, key).parse().unwrap_or_default()
}

/// `path` relative to the root: below the mounted checkout, or below the
/// host checkout a compile database names.
fn path(path: &str, context: &Context<'_>) -> String {
    let host = format!("{}/", context.root.display());
    path.strip_prefix(&host)
        .map_or_else(|| relative(path), str::to_owned)
}

/// The fix of `holder`'s replacements, if they convert.
fn fix(
    rule: &str,
    level: Level,
    tool_level: &str,
    description: &str,
    holder: &Value,
    context: &Context<'_>,
    store: &mut Files<'_>,
) -> Option<Fix> {
    let files: BTreeSet<String> = replacements(holder)
        .map(|replacement| path(&text(replacement, "FilePath"), context))
        .collect();
    let listed: Vec<&String> = files.iter().collect();
    let [file] = listed.as_slice() else {
        store.drop(
            rule,
            files.first().map_or("", String::as_str),
            Drop::MultiFile,
        );
        return None;
    };
    store.keep(rule, file, |source| {
        let edits = replacements(holder)
            .map(|replacement| {
                let start = offset(replacement, "Offset");
                Ok(Edit {
                    start: source.byte(start)?,
                    end: source.byte(start.saturating_add(offset(replacement, "Length")))?,
                    text: text(replacement, "ReplacementText"),
                })
            })
            .collect::<Result<Vec<_>, Drop>>()?;
        Ok(context.fix(
            rule,
            level,
            tool_level,
            description,
            "clang-tidy-yaml",
            file,
            edits,
        ))
    })
}

#[cfg(test)]
mod tests;
