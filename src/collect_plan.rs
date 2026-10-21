//! Deciding, for each catalogue entry, whether and how `collect` runs it:
//! from its pinned image (the default), or, under the explicit
//! `--host-tools`, from `PATH`; with which arguments, and where its output
//! goes. Nothing runs here.
//!
//! Standards: SCF VPM-12.2 (Breadth / Depth of Coverage For
//! Vulnerability Scanning), CFG-11 (Software Usage Restrictions).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::collect_catalogue::{CONTAINER_RULES, Host, Image, Kind, Sink, Tool};
use crate::collect_detect::Ecosystem;
use crate::collect_lookup::find;

/// The rule directories Opengrep and Semgrep get, as absolute paths. There
/// is always a first one, so neither tool can fall back to registry rules.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuleDirs<'a> {
    /// The given `--rules` directory, or sloptopop's own pack.
    pub(crate) first: &'a Path,
    /// With the pack: `.sloptopop/rules/`, if it exists.
    pub(crate) second: Option<&'a Path>,
}

impl<'a> RuleDirs<'a> {
    /// The directories, in order.
    pub(crate) fn iter(self) -> impl Iterator<Item = &'a Path> {
        std::iter::once(self.first).chain(self.second)
    }
}

/// Everything the plan depends on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings<'a> {
    /// The repository root, as an absolute path.
    pub(crate) root: &'a Path,
    /// The ecosystems the root uses.
    pub(crate) ecosystems: &'a BTreeSet<Ecosystem>,
    /// Only these entries (empty: all that apply and run by default).
    pub(crate) only: &'a [String],
    /// The rule directories.
    pub(crate) rules: RuleDirs<'a>,
    /// Whether the root is a git repository.
    pub(crate) git: bool,
}

/// Why an entry does not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotRun {
    /// It cannot: its executable or runtime is not there.
    Missing(String),
    /// It is not meant to, here.
    Skipped(String),
}

/// What happens to one entry: it runs from `R` (its [`Image`], or under
/// `--host-tools` the program found on `PATH`), or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step<R> {
    /// Run it from this.
    Run(R),
    /// Do not.
    NotRun(NotRun),
}

/// One entry of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Planned<'t, R> {
    /// The catalogue entry.
    pub(crate) tool: &'t Tool,
    /// The tool's command line, command first (inside the container, or on
    /// the host).
    pub(crate) argv: Vec<String>,
    /// The file the tool's stdout goes to, if it prints its output.
    pub(crate) stdout: Option<PathBuf>,
    /// What happens to it.
    pub(crate) step: Step<R>,
}

/// Plans every entry of `catalogue` that applies and was selected, to run
/// from its pinned image.
pub(crate) fn images<'t>(
    catalogue: &'t [Tool],
    settings: &Settings<'_>,
) -> Vec<Planned<'t, Image>> {
    plan(catalogue, settings, false, |_, image| Step::Run(image))
}

/// Plans every entry of `catalogue` that applies and was selected, to run
/// from `PATH` (`path` is its value) under `--host-tools`.
pub(crate) fn host<'t>(
    catalogue: &'t [Tool],
    settings: &Settings<'_>,
    path: Option<&OsStr>,
) -> Vec<Planned<'t, PathBuf>> {
    plan(catalogue, settings, true, |tool, _| match tool.host {
        Host::ImageOnly => Step::NotRun(NotRun::Missing(
            "runs only from its pinned image; --host-tools cannot run it".to_owned(),
        )),
        Host::Same | Host::Args(_) => find(path, tool.command).map_or_else(
            || Step::NotRun(NotRun::Missing("not found on PATH".to_owned())),
            Step::Run,
        ),
    })
}

/// Plans the entries that apply and were selected, in catalogue order:
/// without `--only`, every entry that runs by default; with it, exactly
/// the entries it names. `run` decides how an entry that would run does.
fn plan<'t, R>(
    catalogue: &'t [Tool],
    settings: &Settings<'_>,
    host: bool,
    run: impl Fn(&Tool, Image) -> Step<R>,
) -> Vec<Planned<'t, R>> {
    let rules = rule_args(settings.rules, host);
    catalogue
        .iter()
        .filter(|tool| tool.applies(settings.ecosystems))
        .filter(|tool| {
            if settings.only.is_empty() {
                tool.default
            } else {
                settings.only.iter().any(|name| name == tool.name)
            }
        })
        .map(|tool| Planned {
            tool,
            argv: tool.argv(&rules, host),
            stdout: match tool.sink {
                Sink::Stdout => Some(settings.root.join(tool.written_path())),
                Sink::PathArgument | Sink::Config => None,
            },
            step: match tool.kind {
                Kind::Documented => Step::NotRun(NotRun::Skipped(
                    "documented only: run it after your tests".to_owned(),
                )),
                Kind::NeedsGit(_) if !settings.git => {
                    Step::NotRun(NotRun::Skipped("not a git repository".to_owned()))
                }
                Kind::Run(image) | Kind::NeedsGit(image) => run(tool, image),
            },
        })
        .collect()
}

/// The rule directories as the tools are given them: on the host, their
/// paths; in a container, where they are mounted. (Not generic, unlike
/// [`plan`], so each branch is one region however many plans there are.)
fn rule_args(rules: RuleDirs<'_>, host: bool) -> Vec<String> {
    rules
        .iter()
        .enumerate()
        .map(|(index, dir)| {
            if host {
                dir.to_string_lossy().into_owned()
            } else {
                format!("{CONTAINER_RULES}/{index}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
