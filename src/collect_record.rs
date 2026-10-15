//! What `collect` writes: `.sloptopop/reports/.collect.json` (hidden, so
//! the engine never reads it as a report) and the human summary. Both are
//! deterministic: catalogue order, no timestamps, no durations.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::collect_detect::Ecosystem;
use crate::collect_plan::{Planned, Step};
use crate::collect_run::{Outcome, Status};

/// The file name of the run record in the reports directory.
pub(crate) const RECORD_FILE: &str = ".collect.json";

/// The run record as pretty-printed JSON with a final newline.
pub(crate) fn record(
    container: bool,
    ecosystems: &BTreeSet<Ecosystem>,
    outcomes: &[Outcome<'_>],
) -> String {
    let mut document = Map::new();
    document.insert("schemaVersion".to_owned(), 1.into());
    document.insert(
        "mode".to_owned(),
        if container { "container" } else { "host" }.into(),
    );
    document.insert(
        "ecosystems".to_owned(),
        ecosystems
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>()
            .into(),
    );
    document.insert(
        "tools".to_owned(),
        outcomes.iter().map(tool_record).collect::<Vec<_>>().into(),
    );
    format!("{:#}\n", Value::Object(document))
}

fn tool_record(outcome: &Outcome<'_>) -> Value {
    let mut tool = Map::new();
    tool.insert("name".to_owned(), outcome.tool.name.into());
    tool.insert("licence".to_owned(), outcome.tool.licence.into());
    if let Some(version) = &outcome.version {
        tool.insert("version".to_owned(), version.as_str().into());
    }
    tool.insert("argv".to_owned(), outcome.argv.clone().into());
    tool.insert("output".to_owned(), outcome.tool.output_path().into());
    tool.insert("status".to_owned(), outcome.status.name().into());
    if let Some(code) = outcome.status.exit_code() {
        tool.insert("exitCode".to_owned(), code.into());
    }
    if let Some(reason) = outcome.status.reason() {
        tool.insert("reason".to_owned(), reason.into());
    }
    Value::Object(tool)
}

fn ecosystem_list(ecosystems: &BTreeSet<Ecosystem>) -> String {
    if ecosystems.is_empty() {
        return "none".to_owned();
    }
    ecosystems
        .iter()
        .map(|e| e.name())
        .collect::<Vec<_>>()
        .join(", ")
}

fn line(status: &str, name: &str, rest: &str) -> String {
    format!("  {status:<8} {name:<16} {rest}\n")
}

/// The summary printed after a run: a count line, then one line per tool.
pub(crate) fn summary(ecosystems: &BTreeSet<Ecosystem>, outcomes: &[Outcome<'_>]) -> String {
    let count = |name: &str| outcomes.iter().filter(|o| o.status.name() == name).count();
    let mut out = format!(
        "sloptopop collect: {} ran, {} failed, {} missing, {} skipped (ecosystems: {})\n",
        count("ran"),
        count("failed"),
        count("missing"),
        count("skipped"),
        ecosystem_list(ecosystems)
    );
    for outcome in outcomes {
        let rest = match &outcome.status {
            Status::Ran(code) => format!("exit {code}  {}", outcome.tool.output_path()),
            Status::Failed {
                code: Some(code),
                detail,
            } => format!("exit {code}  {detail}"),
            Status::Failed { code: None, detail } => detail.clone(),
            Status::Missing(reason) | Status::Skipped(reason) => reason.clone(),
        };
        out.push_str(&line(outcome.status.name(), outcome.tool.name, &rest));
    }
    out.push_str(
        "Details: .sloptopop/reports/.collect.json; next, `sloptopop check` reads the reports.\n",
    );
    out
}

/// The plan printed by `--dry-run`: one line per tool with the command
/// line, and where its report would go or why it would not run.
pub(crate) fn plan(ecosystems: &BTreeSet<Ecosystem>, planned: &[Planned<'_>]) -> String {
    let mut out = format!(
        "sloptopop collect: dry run, nothing run (ecosystems: {})\n",
        ecosystem_list(ecosystems)
    );
    for entry in planned {
        let command = entry.argv.join(" ");
        let (status, rest) = match &entry.step {
            Step::Run {
                stdout: Some(_), ..
            } => ("run", format!("{command} > {}", entry.tool.output_path())),
            Step::Run { stdout: None, .. } => ("run", command),
            Step::Missing(reason) => ("missing", format!("{command}  ({reason})")),
            Step::Skipped(reason) => ("skipped", format!("{command}  ({reason})")),
        };
        out.push_str(&line(status, entry.tool.name, &rest));
    }
    out
}

#[cfg(test)]
mod tests;
