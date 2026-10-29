//! The commit range in the upload (decision D-0915): `check`, `fix` and
//! `transform` add the range [`history_git`](crate::history_git) reads to
//! the request `sloptopop-local` prepared, as its `history` member, and
//! the manifest says so before anything is sent (`--no-history` leaves it
//! out). Every removed and added line is masked
//! ([`history_mask`](crate::history_mask)); the hunks of a file go only
//! when the file is part of the uploaded snapshot (its path, status and
//! old path always go), so nothing the snapshot leaves out is sent through
//! the range. The document is what the server's Detector reads
//! (`.sloptopop/history/commits.json`, schema version 1):
//!
//! ```json
//! { "schemaVersion": 1, "base": "<merge base>", "truncated": false,
//!   "commits": [ { "sha": "<commit>", "agents": ["claude"],
//!     "files": [ { "path": "a.test.ts", "status": "modified",
//!       "hunks": [ { "oldStart": 3, "newStart": 3,
//!         "removed": ["…"], "added": ["…"] } ] } ] } ] }
//! ```
//!
//! It stays under [`MAX_BYTES`] and leaves the whole request under the
//! server's body limit: the oldest commits are left out until it fits
//! (`truncated`), and with none left there is no history.
//!
//! Standards: SCF PRI-02 (notice before upload), PRI-05 (data
//! minimisation: masked lines, no names or e-mail addresses, no hunks of
//! files outside the snapshot), CHG-02; OWASP ASVS 5.0 V13.3.1; GDPR
//! Art. 25.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::collect_lookup::find;
use crate::history_git::{Commit, MAX_COMMITS, Range, range};
use crate::history_mask::mask;

/// The largest history sent, in bytes (the server reads at most 32 MiB of
/// its canonical form).
pub(crate) const MAX_BYTES: usize = 31 * 1024 * 1024;

/// The largest whole request sent, in bytes (the server's default body
/// limit is 64 MiB).
pub(crate) const MAX_REQUEST: usize = 63 * 1024 * 1024;

/// The history added to a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attached {
    /// How many commits it holds.
    pub(crate) commits: usize,
    /// Its size, in bytes.
    pub(crate) bytes: usize,
    /// The default branch it starts from.
    pub(crate) branch: String,
    /// Whether older commits were left out.
    pub(crate) truncated: bool,
}

/// Adds the commit range of the repository at `root` (git found on
/// `path`) to the request file `request`; what was added, or `None` when
/// there is no range or the request cannot be read or written.
pub(crate) fn attach(path: Option<&OsStr>, root: &Path, request: &Path) -> Option<Attached> {
    let git = find(path, "git")?;
    let range = range(&git, root, MAX_COMMITS)?;
    let text = fs::read_to_string(request).ok()?;
    let mut value: Value = serde_json::from_str(&text).ok()?;
    let snapshot: BTreeSet<&str> = value
        .get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let budget = MAX_BYTES.min(MAX_REQUEST.saturating_sub(text.len()));
    let made = document(&range, &snapshot, budget);
    made.and_then(|(document, attached)| {
        value
            .as_object_mut()?
            .insert("history".to_owned(), document);
        fs::write(request, value.to_string())
            .is_ok()
            .then_some(attached)
    })
}

/// The history document of `range` for the snapshot's paths `snapshot`,
/// at most `budget` bytes, and what it holds; `None` when not one commit
/// fits.
fn document(range: &Range, snapshot: &BTreeSet<&str>, budget: usize) -> Option<(Value, Attached)> {
    let mut commits: Vec<(Value, usize)> = range
        .commits
        .iter()
        .map(|each| {
            let value = commit(each, snapshot);
            let len = value.to_string().len() + 1;
            (value, len)
        })
        .collect();
    let mut truncated = range.truncated;
    let head = json!({"schemaVersion": 1, "base": range.base, "truncated": true, "commits": []})
        .to_string()
        .len();
    let mut total = head + commits.iter().map(|(_, len)| len).sum::<usize>();
    while total > budget && !commits.is_empty() {
        let (_, len) = commits.remove(0);
        total -= len;
        truncated = true;
    }
    (!commits.is_empty()).then(|| {
        let count = commits.len();
        let document = json!({
            "schemaVersion": 1,
            "base": range.base,
            "truncated": truncated,
            "commits": commits.into_iter().map(|(value, _)| value).collect::<Vec<_>>(),
        });
        let bytes = document.to_string().len();
        (
            document,
            Attached {
                commits: count,
                bytes,
                branch: range.branch.clone(),
                truncated,
            },
        )
    })
}

/// One commit as the server reads it, masked; hunks only for the files
/// in `snapshot`.
fn commit(commit: &Commit, snapshot: &BTreeSet<&str>) -> Value {
    let files: Vec<Value> = commit
        .files
        .iter()
        .map(|change| {
            let mut file = Map::new();
            file.insert("path".to_owned(), json!(change.path));
            file.insert("status".to_owned(), json!(change.status.as_str()));
            if let Some(from) = &change.from {
                file.insert("from".to_owned(), json!(from));
            }
            if snapshot.contains(change.path.as_str()) {
                let hunks = change
                    .hunks
                    .iter()
                    .map(|hunk| {
                        json!({
                            "oldStart": hunk.old_start,
                            "newStart": hunk.new_start,
                            "removed": mask(&hunk.removed),
                            "added": mask(&hunk.added),
                        })
                    })
                    .collect();
                file.insert("hunks".to_owned(), Value::Array(hunks));
            }
            Value::Object(file)
        })
        .collect();
    json!({"sha": commit.sha, "agents": commit.agents, "files": files})
}

/// The manifest's line for `attached`.
pub(crate) fn line(attached: &Attached) -> String {
    let newest = if attached.truncated {
        ", the newest only"
    } else {
        ""
    };
    format!(
        "  commit range: {} commit(s) since the merge base with {}{newest}, {} bytes, lines masked (--no-history leaves it out)\n",
        attached.commits, attached.branch, attached.bytes
    )
}

#[cfg(test)]
mod tests;
