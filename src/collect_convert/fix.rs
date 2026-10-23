//! The fix carrier (WP-21): an analyzer's own fix, kept in a SARIF report
//! as `result.fixes` with **byte regions only** (`byteOffset`,
//! `byteLength`, SARIF 2.1.0 §3.30.3, §3.30.11–12, §3.57), so no reader can
//! mistake the unit. Each tool counts positions its own way; [`Source`]
//! turns each convention into byte offsets of the committed file, exactly,
//! or refuses with a [`Drop`] reason. A fix is never re-derived: its edits
//! keep their extent, order and text; only their coordinates change.
//!
//! Standards: SCF TDA-12 (Input Data Validation: a tool's output is read by
//! a fixed grammar and a fix that does not convert is dropped, never
//! guessed), SCF TDA-06.5 (integrity: the digest of the bytes the tool
//! read travels with the fix), SCF TDA-20 (reproducible artefacts).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use serde_json::{Map, Value, json};

/// The byte order mark a UTF-8 file may start with.
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// How safe the tool says a fix is, normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    /// The tool applies it in its default fix mode, with no opt-in.
    Safe,
    /// The tool applies it only with an opt-in.
    Unsafe,
    /// The tool never applies it by itself.
    Suggestion,
}

impl Level {
    /// `safe`, `unsafe` or `suggestion`.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Unsafe => "unsafe",
            Self::Suggestion => "suggestion",
        }
    }
}

/// How a tool counts columns on a 1-based line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Columns {
    /// Unicode code points; a byte order mark is not a column (Ruff).
    CodePointsBomSkipped,
    /// Unicode code points; a byte order mark is column 1 of line 1
    /// (Biome).
    CodePointsBomCounted,
    /// UTF-8 bytes (staticcheck's SARIF, whatever its `columnKind`).
    Utf8Bytes,
}

/// Why a fix was not kept. The finding always stays; only its fix goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Drop {
    /// The file is not UTF-8.
    NotUtf8,
    /// The file has a carriage return without a line feed, which tools
    /// count as a line end differently.
    LoneCr,
    /// A position past the end of its line or of the file.
    OutOfRange,
    /// An offset inside a UTF-8 character, or between `\r` and `\n`.
    Boundary,
    /// A UTF-16 offset inside a surrogate pair.
    Surrogate,
    /// An edit that removes the byte order mark.
    Bom,
    /// A fix inside a notebook cell (cell-relative rows).
    Notebook,
    /// Edits out of order or overlapping, or two insertions at one point.
    Overlap,
    /// Several fixes of one level for one result: alternatives.
    Alternatives,
    /// A fix that edits more than one file.
    MultiFile,
    /// A fix without a file or without any edit.
    Placeholder,
    /// The file is not the one the tool read.
    Stale,
    /// The path is not a regular file below the root.
    Path,
    /// The project's configuration can change this tool's fix safety.
    Config,
    /// A level, or a format, this converter does not know.
    Unconverted,
    /// A fix of a rule the reviewed fix table does not list, from a tool
    /// whose fixes are carried only for reviewed rules (clang-tidy).
    Unreviewed,
}

impl Drop {
    /// The reason as it appears in the report.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotUtf8 => "fix.not-utf8",
            Self::LoneCr => "fix.lone-cr",
            Self::OutOfRange => "fix.out-of-range",
            Self::Boundary => "fix.boundary",
            Self::Surrogate => "fix.surrogate",
            Self::Bom => "fix.bom",
            Self::Notebook => "fix.notebook",
            Self::Overlap => "fix.overlap-within",
            Self::Alternatives => "fix.alternatives",
            Self::MultiFile => "fix.multi-file",
            Self::Placeholder => "fix.placeholder",
            Self::Stale => "fix.stale",
            Self::Path => "fix.path",
            Self::Config => "fix.config",
            Self::Unconverted => "fix.unconverted",
            Self::Unreviewed => "fix.unreviewed",
        }
    }
}

/// A file's committed bytes, with the byte offset of each line's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    bytes: Vec<u8>,
    starts: Vec<usize>,
}

impl Source {
    /// The file's bytes as a source, or why its fixes cannot be kept.
    pub(crate) fn new(bytes: Vec<u8>) -> Result<Self, Drop> {
        if std::str::from_utf8(&bytes).is_err() {
            return Err(Drop::NotUtf8);
        }
        let lone = bytes
            .iter()
            .enumerate()
            .any(|(i, &b)| b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
        if lone {
            return Err(Drop::LoneCr);
        }
        let starts = std::iter::once(0)
            .chain(
                bytes
                    .iter()
                    .enumerate()
                    .filter(|&(_, &b)| b == b'\n')
                    .map(|(i, _)| i + 1),
            )
            .collect();
        Ok(Self { bytes, starts })
    }

    /// The file's bytes.
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The text of the file (checked UTF-8 in [`Self::new`]).
    fn text(&self, from: usize) -> &str {
        self.bytes
            .get(from..)
            .and_then(|tail| std::str::from_utf8(tail).ok())
            .unwrap_or_default()
    }

    /// The byte offset of 1-based `line` and `column`, counted in `columns`.
    /// The column may point just past the line's last character (before
    /// its line end), never beyond.
    pub(crate) fn line_column(
        &self,
        columns: Columns,
        line: u64,
        column: u64,
    ) -> Result<usize, Drop> {
        let index = usize::try_from(line)
            .ok()
            .and_then(|line| line.checked_sub(1))
            .ok_or(Drop::OutOfRange)?;
        let start = *self.starts.get(index).ok_or(Drop::OutOfRange)?;
        let mut from = start;
        if columns == Columns::CodePointsBomSkipped && index == 0 && self.bytes.starts_with(BOM) {
            from += BOM.len();
        }
        let skip = usize::try_from(column)
            .ok()
            .and_then(|column| column.checked_sub(1))
            .ok_or(Drop::OutOfRange)?;
        let rest = self.text(from);
        let line_text = rest.split('\n').next().unwrap_or_default();
        let offset = match columns {
            Columns::Utf8Bytes => (skip <= line_text.len()).then_some(skip),
            Columns::CodePointsBomSkipped | Columns::CodePointsBomCounted => line_text
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(line_text.len()))
                .nth(skip),
        }
        .ok_or(Drop::OutOfRange)?;
        self.boundary(from + offset)
    }

    /// The byte offset of a UTF-16 code-unit offset into the text without
    /// its byte order mark (`ESLint`'s `range`).
    pub(crate) fn utf16(&self, offset: i64) -> Result<usize, Drop> {
        let units = usize::try_from(offset).map_err(|_| Drop::Bom)?;
        let base = if self.bytes.starts_with(BOM) {
            BOM.len()
        } else {
            0
        };
        let mut counted = 0;
        for (i, c) in self.text(base).char_indices() {
            if counted == units {
                return Ok(base + i);
            }
            counted += c.len_utf16();
            if counted > units {
                return Err(Drop::Surrogate);
            }
        }
        if counted == units {
            Ok(self.bytes.len())
        } else {
            Err(Drop::OutOfRange)
        }
    }

    /// The 1-based line and byte column of the 0-based byte `offset` (how
    /// clang counts), if it is in the file.
    pub(crate) fn position(&self, offset: u64) -> Option<(u64, u64)> {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset > self.bytes.len() {
            return None;
        }
        // `starts` begins with line 1's 0, so `line` is at least 1 and its
        // start at most `offset`.
        let line = self.starts.partition_point(|&start| start <= offset);
        let start = self
            .starts
            .get(..line)
            .and_then(<[usize]>::last)
            .copied()
            .unwrap_or_default();
        Some((
            u64::try_from(line).unwrap_or(u64::MAX),
            u64::try_from(offset - start)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        ))
    }

    /// A 0-based byte offset, checked against the file.
    pub(crate) fn byte(&self, offset: u64) -> Result<usize, Drop> {
        let offset = usize::try_from(offset).unwrap_or(usize::MAX);
        if offset > self.bytes.len() {
            return Err(Drop::OutOfRange);
        }
        self.boundary(offset)
    }

    /// `offset` if it is on a character boundary and not between `\r` and
    /// `\n`.
    fn boundary(&self, offset: usize) -> Result<usize, Drop> {
        let inside = self.bytes.get(offset).is_some_and(|&b| {
            b & 0xC0 == 0x80
                || (b == b'\n' && offset > 0 && self.bytes.get(offset - 1) == Some(&b'\r'))
        });
        if inside {
            Err(Drop::Boundary)
        } else {
            Ok(offset)
        }
    }
}

/// One replacement: the bytes `start..end` of the file become `text`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Edit {
    /// The first byte replaced.
    pub(crate) start: usize,
    /// The byte after the last one replaced (`start` for an insertion).
    pub(crate) end: usize,
    /// The text inserted in their place.
    pub(crate) text: String,
}

/// One fix, in byte offsets of one file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Fix {
    /// The normalised level.
    pub(crate) level: Level,
    /// The tool's own word for it (`safe`, `fix`, `MachineApplicable`, …).
    pub(crate) tool_level: String,
    /// The tool's description of the fix, verbatim (may be empty).
    pub(crate) description: String,
    /// The tool output it came from (`ruff-json`, `eslint-json`, …).
    pub(crate) source: &'static str,
    /// The file, relative to the root.
    pub(crate) path: String,
    /// The edits, in the tool's order.
    pub(crate) edits: Vec<Edit>,
    /// Whether the reviewed fix table lists this tool, version, rule and
    /// level as reviewed and switched on.
    pub(crate) reviewed: bool,
}

impl Fix {
    /// Checks the edits as the tool gave them: each in order, none
    /// overlapping the next, no two insertions at one point (their order
    /// would decide the text), none touching or moving the byte order mark.
    pub(crate) fn checked(self, source: &Source) -> Result<Self, Drop> {
        if self.edits.is_empty() {
            return Err(Drop::Placeholder);
        }
        let bom = source.bytes().starts_with(BOM);
        for edit in &self.edits {
            if edit.end < edit.start {
                return Err(Drop::Overlap);
            }
            if bom && edit.start < BOM.len() {
                return Err(Drop::Bom);
            }
        }
        let ordered = self
            .edits
            .iter()
            .zip(self.edits.iter().skip(1))
            .all(|(a, b)| {
                a.end <= b.start && !(a.start == a.end && b.start == b.end && a.end == b.start)
            });
        if ordered {
            Ok(self)
        } else {
            Err(Drop::Overlap)
        }
    }

    /// The SARIF `fix` object (§3.55) in byte regions.
    pub(crate) fn sarif(&self) -> Value {
        let replacements: Vec<Value> = self
            .edits
            .iter()
            .map(|edit| {
                let mut replacement = Map::new();
                replacement.insert(
                    "deletedRegion".to_owned(),
                    json!({ "byteOffset": edit.start, "byteLength": edit.end - edit.start }),
                );
                if !edit.text.is_empty() {
                    replacement.insert("insertedContent".to_owned(), json!({ "text": edit.text }));
                }
                Value::Object(replacement)
            })
            .collect();
        json!({
            "description": { "text": self.description },
            "artifactChanges": [{
                "artifactLocation": { "uri": self.path },
                "replacements": replacements,
            }],
            "properties": {
                "sloptopop/applicability": self.level.as_str(),
                "sloptopop/toolApplicability": self.tool_level,
                "sloptopop/source": self.source,
                "sloptopop/converter": 1,
                "sloptopop/reviewed": self.reviewed,
            },
        })
    }
}

/// Whether `fix` (a SARIF `fix` object) is one of sloptopop's carriers.
pub(crate) fn is_carrier(fix: &Value) -> bool {
    fix.pointer("/properties/sloptopop~1converter").is_some()
}

/// Whether `path` is a plain relative path: no root, no `..`, not empty.
fn plain(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// The files fixes name, read once each, and what was dropped.
#[derive(Debug)]
pub(crate) struct Files<'r> {
    root: &'r Path,
    read: BTreeMap<String, Result<Source, Drop>>,
    used: BTreeMap<String, (usize, String)>,
    dropped: BTreeSet<(String, String, Drop)>,
}

impl<'r> Files<'r> {
    /// No file read yet, below `root`.
    pub(crate) fn new(root: &'r Path) -> Self {
        Self {
            root,
            read: BTreeMap::new(),
            used: BTreeMap::new(),
            dropped: BTreeSet::new(),
        }
    }

    /// The file at `path` (relative to the root), read once.
    pub(crate) fn get(&mut self, path: &str) -> Result<&Source, Drop> {
        let root = self.root;
        self.read
            .entry(path.to_owned())
            .or_insert_with(|| {
                let full = root.join(path);
                let regular = plain(path)
                    && fs::symlink_metadata(&full).is_ok_and(|meta| meta.file_type().is_file());
                if !regular {
                    return Err(Drop::Path);
                }
                fs::read(&full)
                    .map_err(|_| Drop::Path)
                    .and_then(Source::new)
            })
            .as_ref()
            .map_err(|drop| *drop)
    }

    /// Keeps the fix `make` builds from the file it names, or records
    /// why not, for `rule`.
    pub(crate) fn keep(
        &mut self,
        rule: &str,
        path: &str,
        make: impl FnOnce(&Source) -> Result<Fix, Drop>,
    ) -> Option<Fix> {
        let built = self.get(path).and_then(|source| {
            let fix = make(source).and_then(|fix| fix.checked(source))?;
            let digest = blake3::hash(source.bytes()).to_hex().to_string();
            Ok((fix, source.bytes().len(), digest))
        });
        match built {
            Ok((fix, length, digest)) => {
                self.used.insert(path.to_owned(), (length, digest));
                Some(fix)
            }
            Err(drop) => {
                self.drop(rule, path, drop);
                None
            }
        }
    }

    /// Records that a fix for `rule` in `path` was dropped.
    pub(crate) fn drop(&mut self, rule: &str, path: &str, drop: Drop) {
        self.dropped
            .insert((rule.to_owned(), path.to_owned(), drop));
    }

    /// The SARIF `artifacts` (§3.24) of every file a kept fix names: its
    /// length and the BLAKE3 digest of the bytes the tool read.
    pub(crate) fn artifacts(&self) -> Vec<Value> {
        self.used
            .iter()
            .map(|(path, (length, digest))| {
                json!({
                    "location": { "uri": path },
                    "length": length,
                    "hashes": { "blake3": digest },
                })
            })
            .collect()
    }

    /// The dropped fixes, sorted: `{rule, path, reason}`.
    pub(crate) fn dropped(&self) -> Vec<Value> {
        self.dropped
            .iter()
            .map(
                |(rule, path, drop)| json!({ "rule": rule, "path": path, "reason": drop.as_str() }),
            )
            .collect()
    }

    /// Adds the artifacts and the dropped fixes to a SARIF `run`.
    pub(crate) fn record(&self, run: &mut Map<String, Value>) {
        let artifacts = self.artifacts();
        if !artifacts.is_empty() {
            let list = run
                .entry("artifacts")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Value::Array(list) = list {
                list.extend(artifacts);
            }
        }
        let dropped = self.dropped();
        if !dropped.is_empty() {
            let properties = run
                .entry("properties")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Value::Object(properties) = properties {
                properties.insert("sloptopop/droppedFixes".to_owned(), Value::Array(dropped));
            }
        }
    }
}

/// At most one fix per level for one result: several of one level are
/// alternatives, and none of them is kept.
pub(crate) fn one_per_level(store: &mut Files<'_>, rule: &str, mut fixes: Vec<Fix>) -> Vec<Fix> {
    fixes.sort();
    let mut counts: BTreeMap<Level, usize> = BTreeMap::new();
    for fix in &fixes {
        *counts.entry(fix.level).or_default() += 1;
    }
    let mut kept = Vec::new();
    for fix in fixes {
        if counts.get(&fix.level).copied().unwrap_or_default() > 1 {
            store.drop(rule, &fix.path, Drop::Alternatives);
        } else {
            kept.push(fix);
        }
    }
    kept
}

#[cfg(test)]
mod tests;
