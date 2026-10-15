//! `sloptopop collect`: runs the installed analyzers from the catalogue
//! with fixed flags and writes their reports to `.sloptopop/reports/`,
//! where `sloptopop check` (the engine) reads them.

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf, absolute};

use crate::args::CollectOptions;
use crate::collect_catalogue::{CATALOGUE, REPORTS_DIR};
use crate::collect_detect::detect;
use crate::collect_gitignore::ensure_ignored;
use crate::collect_plan::{Settings, plan};
use crate::collect_record::{RECORD_FILE, record, summary};
use crate::collect_run::run;
use crate::diagnostic::report;

/// The default rules directory, relative to the root.
pub(crate) const RULES_DIR: &str = ".sloptopop/rules";

/// Exit code when the reports cannot be written or stdout is broken.
pub(crate) const WRITE_ERROR: u8 = 1;

/// Runs `collect` with `path` as the value of `PATH`. Returns the exit
/// code: 0, or [`WRITE_ERROR`] if nothing could be recorded. A tool that
/// is missing or fails never changes it. `Err` holds a usage message for
/// invalid arguments.
pub(crate) fn collect(
    options: &CollectOptions,
    path: Option<&OsStr>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<u8, String> {
    if let Some(unknown) = options
        .only
        .iter()
        .find(|name| !CATALOGUE.iter().any(|tool| tool.name == *name))
    {
        return Err(format!(
            "unknown tool: {unknown} (docs/collect.md lists them)"
        ));
    }
    let root =
        directory(&options.root).ok_or_else(|| format!("not a directory: {}", options.root))?;
    let rules = match &options.rules {
        Some(dir) => {
            Some(directory(dir).ok_or_else(|| format!("rules directory not found: {dir}"))?)
        }
        None => Some(root.join(RULES_DIR)).filter(|dir| dir.is_dir()),
    };
    let ecosystems = detect(&root).unwrap_or_default();
    let planned = plan(
        CATALOGUE,
        &Settings {
            root: &root,
            ecosystems: &ecosystems,
            only: &options.only,
            rules: rules.as_deref(),
            git: root.join(".git").exists(),
            container: options.container,
            path,
        },
    );
    if options.dry_run {
        return Ok(print(
            stdout,
            stderr,
            &crate::collect_record::plan(&ecosystems, &planned),
        ));
    }
    let reports = root.join(REPORTS_DIR);
    if let Err(e) = fs::create_dir_all(&reports).and_then(|()| ensure_ignored(&root, &reports)) {
        return Ok(failure(stderr, &reports, &e));
    }
    let outcomes: Vec<_> = planned.into_iter().map(|entry| run(entry, &root)).collect();
    let record_path = reports.join(RECORD_FILE);
    if let Err(e) = fs::write(
        &record_path,
        record(options.container, &ecosystems, &outcomes),
    ) {
        return Ok(failure(stderr, &record_path, &e));
    }
    Ok(print(stdout, stderr, &summary(&ecosystems, &outcomes)))
}

/// `path` as an absolute path, if it names a directory.
fn directory(path: &str) -> Option<PathBuf> {
    absolute(path).ok().filter(|dir| dir.is_dir())
}

fn failure(stderr: &mut dyn Write, path: &Path, error: &std::io::Error) -> u8 {
    report(
        stderr,
        &format!("error: cannot write {}: {error}\n", path.display()),
        WRITE_ERROR,
    )
}

fn print(stdout: &mut dyn Write, stderr: &mut dyn Write, text: &str) -> u8 {
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => 0,
        Err(e) => report(
            stderr,
            &format!("error: cannot write output: {e}\n"),
            WRITE_ERROR,
        ),
    }
}

#[cfg(test)]
mod tests;
