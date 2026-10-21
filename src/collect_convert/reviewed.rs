//! The reviewed fix table, the tools' own fix kinds where a report does not
//! carry them (Biome), and the configuration guard: whether the project's
//! own configuration can change a tool's fix safety, in which case its safe
//! fixes are not kept (WP-21).
//!
//! Standards: SCF CHG-03 (Security Impact Analysis for Changes: only fixes
//! a reviewer accepted are marked reviewed), SCF TDA-12 (the tool's claim
//! is checked against its own pinned metadata).

use std::fs;
use std::path::Path;

use super::fix::Level;

/// Tool, version, rule, level, state, note (`reviewed-fixes.tsv`).
const REVIEWED: &str = include_str!("reviewed-fixes.tsv");

/// Diagnostic category and kind of every Biome rule with a fix.
const BIOME_KINDS: &str = include_str!("biome-fix-kinds.tsv");

/// The Biome version [`BIOME_KINDS`] was generated from.
const BIOME_VERSION: &str = "2.5.15";

/// The rows of a tab-separated table, comments and blank lines skipped.
fn rows(table: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    table
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.split('\t').collect())
}

/// The upstream version of an image version: `0.17.1-1` is `0.17.1`.
pub(crate) fn upstream(version: &str) -> &str {
    version
        .rsplit_once('-')
        .map_or(version, |(upstream, _)| upstream)
}

/// Whether the table lists `tool` at `version`, `rule` and `level`, and
/// the row is switched on.
pub(crate) fn reviewed(tool: &str, version: &str, rule: &str, level: Level) -> bool {
    let wanted = [tool, upstream(version), rule, level.as_str(), "on"];
    rows(REVIEWED).any(|row| row.get(..wanted.len()) == Some(wanted.as_slice()))
}

/// Biome's own fix kind for the rule `category`, if Biome is the version
/// the kinds come from.
pub(crate) fn biome_kind(version: &str, category: &str) -> Option<Level> {
    if upstream(version) != BIOME_VERSION {
        return None;
    }
    rows(BIOME_KINDS).find_map(|row| match row.as_slice() {
        [rule, "safe"] if *rule == category => Some(Level::Safe),
        [rule, "unsafe"] if *rule == category => Some(Level::Unsafe),
        _ => None,
    })
}

/// A tool whose project configuration can change its fix safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Guard {
    /// Ruff: `extend-safe-fixes` promotes an unsafe fix to safe.
    Ruff,
    /// Biome: a rule's `fix` option sets its fix kind.
    Biome,
}

impl Guard {
    /// Whether the file `name` holding `text` may change the fix safety.
    fn changes(self, name: &str, text: &str) -> bool {
        match self {
            Self::Ruff => {
                Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("toml"))
                    && text.contains("extend-safe-fixes")
            }
            Self::Biome => matches!(name, "biome.json" | "biome.jsonc") && text.contains("\"fix\""),
        }
    }
}

/// The first configuration file below `root` (in path order; hidden
/// directories and `node_modules` skipped) that may change `guard`'s fix
/// safety.
pub(crate) fn promoting_config(root: &Path, guard: Guard) -> Option<String> {
    walk(root, "", guard)
}

fn walk(dir: &Path, prefix: &str, guard: Guard) -> Option<String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| (entry.file_name().to_string_lossy().into_owned(), entry))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries.into_iter().find_map(|(name, entry)| {
        let path = format!("{prefix}{name}");
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            if name.starts_with('.') || name == "node_modules" {
                return None;
            }
            walk(&entry.path(), &format!("{path}/"), guard)
        } else {
            let text = fs::read_to_string(entry.path()).ok()?;
            guard.changes(&name, &text).then_some(path)
        }
    })
}

#[cfg(test)]
mod tests;
