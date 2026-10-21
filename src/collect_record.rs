//! What `collect` writes: `.sloptopop/reports/.collect.json` (hidden, so
//! the engine never reads it as a report) and the human summary. Both are
//! deterministic: catalogue order, no timestamps, no durations.
//!
//! Standards: SCF MON-09 (Content of Event Logs: what ran, from which
//! pinned image and data, and how it ended, deterministic).

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use crate::collect_catalogue::Image;
use crate::collect_data::Resolved;
use crate::collect_detect::Ecosystem;
use crate::collect_image::megabytes;
use crate::collect_plan::{NotRun, Planned, Step};
use crate::collect_run::{Outcome, Status};
use crate::collect_runtime::Runtime;

/// The file name of the run record in the reports directory.
pub(crate) const RECORD_FILE: &str = ".collect.json";

/// Where the tools ran from, as recorded.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Mode<'a> {
    /// The pinned images, in this runtime (none if no runtime answered),
    /// from this registry.
    Images {
        /// The runtime.
        runtime: Option<&'a Runtime>,
        /// The registry.
        registry: &'a str,
    },
    /// `--host-tools`: unpinned tools from `PATH`.
    Host,
}

/// The run record as pretty-printed JSON with a final newline.
pub(crate) fn record(
    mode: Mode<'_>,
    ecosystems: &BTreeSet<Ecosystem>,
    databases: &[Resolved],
    outcomes: &[Outcome<'_>],
) -> String {
    let mut document = Map::new();
    document.insert("schemaVersion".to_owned(), 2.into());
    match mode {
        Mode::Images { runtime, registry } => {
            document.insert("mode".to_owned(), "container".into());
            document.insert("registry".to_owned(), registry.into());
            if let Some(runtime) = runtime {
                document.insert(
                    "runtime".to_owned(),
                    json!({
                        "name": runtime.engine.name(),
                        "version": runtime.version,
                        "rootless": runtime.rootless,
                    }),
                );
            }
        }
        Mode::Host => {
            document.insert("mode".to_owned(), "host-unpinned".into());
        }
    }
    document.insert(
        "ecosystems".to_owned(),
        ecosystems
            .iter()
            .map(|e| e.name())
            .collect::<Vec<_>>()
            .into(),
    );
    document.insert(
        "databases".to_owned(),
        databases
            .iter()
            .map(|db| {
                json!({
                    "name": db.snapshot.name,
                    "reference": db.reference,
                    "version": db.version,
                })
            })
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
    tool.insert(
        "licenceFiles".to_owned(),
        outcome.tool.licence_files.to_vec().into(),
    );
    if let Some(version) = &outcome.version {
        tool.insert("version".to_owned(), version.as_str().into());
    }
    if let Some(image) = &outcome.image {
        tool.insert(
            "image".to_owned(),
            json!({ "reference": image.reference, "version": image.version }),
        );
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
    format!("  {status:<8} {name:<20} {rest}\n")
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

/// The plan printed by `--dry-run`: one line per tool with what it would
/// run from (`describe` renders a [`Step::Run`] with the command line) and
/// where its report would go, or why it would not run.
pub(crate) fn plan<R>(
    ecosystems: &BTreeSet<Ecosystem>,
    planned: &[Planned<'_, R>],
    describe: impl Fn(&R, &str) -> String,
) -> String {
    let mut out = format!(
        "sloptopop collect: dry run, nothing fetched or run (ecosystems: {})\n",
        ecosystem_list(ecosystems)
    );
    for entry in planned {
        let command = entry.argv.join(" ");
        let target = match &entry.stdout {
            Some(_) => format!("{command} > {}", entry.tool.written_path()),
            None => command,
        };
        let (status, rest) = match &entry.step {
            Step::Run(from) => ("run", describe(from, &target)),
            Step::NotRun(NotRun::Missing(reason)) => ("missing", format!("{target}  ({reason})")),
            Step::NotRun(NotRun::Skipped(reason)) => ("skipped", format!("{target}  ({reason})")),
        };
        out.push_str(&line(status, entry.tool.name, &rest));
    }
    out
}

/// A dry-run line's description of an image run: the reference pulled
/// from `registry`, its version, its size for `arch`, and the command line.
pub(crate) fn image_line(image: &Image, registry: &str, arch: &str, target: &str) -> String {
    let size = megabytes(image.pull_bytes(arch));
    format!(
        "{} ({}, {size}): {target}",
        image.reference(registry),
        image.version
    )
}

#[cfg(test)]
mod tests;
