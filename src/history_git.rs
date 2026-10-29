//! The snapshot's commit range, read from the local git (decision D-0915):
//! the commits from the merge base of `HEAD` with the default branch
//! (`origin/HEAD`, else `origin/main`, `origin/master`, `main`, `master`)
//! to `HEAD`, the newest [`MAX_COMMITS`] of them, oldest first. Per commit:
//! its id, the coding agents its trailers or its author name (only the
//! agent's id is kept, never a name or an e-mail address), and per changed
//! file the status, the old path of a rename or copy, and the hunks
//! without context (`-U0`): their removed and added lines.
//!
//! No git, a repository git cannot read, a shallow clone, a detached
//! `HEAD`, no default branch, or no commit since the merge base: no range.
//! That is a normal state, not an error. git runs with argument vectors
//! only, `LC_ALL=C`, no pager, no colours, no external diff or text
//! conversion and no signature check.
//!
//! Standards: SCF CHG-02 (change history as evidence), PRI-05 (only what
//! the rules read: no names, no e-mail addresses); OWASP ASVS 5.0 V1.2.5
//! (argument vectors, never a shell); CWE-78.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

/// The most commits of a range (D-0915).
pub(crate) const MAX_COMMITS: usize = 200;

/// The default branch's candidates, in order.
const DEFAULT_BRANCHES: [&str; 5] = [
    "refs/remotes/origin/HEAD",
    "refs/remotes/origin/main",
    "refs/remotes/origin/master",
    "refs/heads/main",
    "refs/heads/master",
];

/// The coding agents, by id, and the word sequences that name each one
/// in a trailer or an author (lower case, split at anything but letters
/// and digits).
const AGENTS: [(&str, &[&[&str]]); 13] = [
    ("aider", &[&["aider"]]),
    ("amazon-q", &[&["amazon", "q"], &["amazonq"]]),
    ("claude", &[&["claude"], &["anthropic"]]),
    ("cline", &[&["cline"]]),
    ("codex", &[&["codex"], &["chatgpt"], &["openai"]]),
    ("copilot", &[&["copilot"]]),
    (
        "cursor",
        &[&["cursoragent"], &["cursor", "agent"], &["cursor", "com"]],
    ),
    ("devin", &[&["devin", "ai"]]),
    ("gemini", &[&["gemini"]]),
    ("jules", &[&["labs", "jules"]]),
    ("junie", &[&["junie"]]),
    ("openhands", &[&["openhands"]]),
    ("windsurf", &[&["windsurf"], &["codeium"]]),
];

/// How a commit changed a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    /// New.
    Added,
    /// Deleted.
    Deleted,
    /// Changed in place (also a type change).
    Modified,
    /// Renamed.
    Renamed,
    /// Copied.
    Copied,
}

impl Status {
    /// The name the server reads.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Deleted => "deleted",
            Self::Modified => "modified",
            Self::Renamed => "renamed",
            Self::Copied => "copied",
        }
    }
}

/// One hunk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Hunk {
    /// Its first line in the old file (0 for none).
    pub(crate) old_start: u64,
    /// Its first line in the new file (0 for none).
    pub(crate) new_start: u64,
    /// The removed lines.
    pub(crate) removed: Vec<String>,
    /// The added lines.
    pub(crate) added: Vec<String>,
}

/// One changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Change {
    /// Its path after the commit (before it, for a deletion).
    pub(crate) path: String,
    /// How it changed.
    pub(crate) status: Status,
    /// The old path of a rename or copy.
    pub(crate) from: Option<String>,
    /// The hunks (none for a binary file, or when git's two listings of
    /// the commit do not line up).
    pub(crate) hunks: Vec<Hunk>,
}

/// One commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Commit {
    /// The commit id.
    pub(crate) sha: String,
    /// The coding agents its trailers or its author name, sorted.
    pub(crate) agents: Vec<String>,
    /// The files it changed.
    pub(crate) files: Vec<Change>,
}

/// The commit range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Range {
    /// The merge base the range starts after.
    pub(crate) base: String,
    /// The default branch it was taken against.
    pub(crate) branch: String,
    /// Whether older commits were left out.
    pub(crate) truncated: bool,
    /// The commits, oldest first.
    pub(crate) commits: Vec<Commit>,
}

/// Runs `git` in `root` with `args`: its output if it succeeded.
fn run(git: &Path, root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(git)
        .arg("-C")
        .arg(root)
        .args(args)
        .env("LC_ALL", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The range of the repository at `root`, read with the `git` program,
/// the newest `max` commits; `None` when there is none.
pub(crate) fn range(git: &Path, root: &Path, max: usize) -> Option<Range> {
    let git = |args: &[&str]| run(git, root, args);
    git(&["rev-parse", "--is-shallow-repository"]).filter(|out| out.trim() == "false")?;
    git(&["symbolic-ref", "-q", "HEAD"])?;
    let (branch, base) = DEFAULT_BRANCHES.iter().find_map(|branch| {
        let base = git(&["merge-base", "HEAD", branch])?;
        Some((*branch, base.trim().to_owned()))
    })?;
    let limit = format!("--max-count={}", max.saturating_add(1));
    let log = git(&[
        "log",
        "--no-show-signature",
        "--no-color",
        &limit,
        "--format=%x1e%H%x1f%an%x1f%ae%x1f%(trailers:only,unfold)",
        &format!("{base}..HEAD"),
    ])
    .unwrap_or_default();
    let mut listed = commits(&log);
    let truncated = listed.len() > max;
    listed.truncate(max);
    listed.reverse();
    let commits: Vec<Commit> = listed
        .into_iter()
        .map(|(sha, agents)| {
            let names = git(&[
                "diff-tree",
                "--root",
                "-r",
                "-z",
                "-M",
                "-C",
                "--no-commit-id",
                "--name-status",
                &sha,
            ])
            .unwrap_or_default();
            let patch = git(&[
                "diff-tree",
                "--root",
                "-r",
                "-M",
                "-C",
                "-p",
                "-U0",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--no-commit-id",
                &sha,
            ])
            .unwrap_or_default();
            Commit {
                files: changes(&names, &patch),
                sha,
                agents,
            }
        })
        .collect();
    (!commits.is_empty()).then(|| Range {
        base,
        branch: branch
            .trim_start_matches("refs/remotes/")
            .trim_start_matches("refs/heads/")
            .to_owned(),
        truncated,
        commits,
    })
}

/// The commits of `git log`'s output in the format [`range`] asks for,
/// newest first: each id and the agents it names.
fn commits(log: &str) -> Vec<(String, Vec<String>)> {
    log.split('\u{1e}')
        .filter_map(|record| {
            let mut fields = record.split('\u{1f}');
            let sha = fields.next().unwrap_or_default().trim();
            let named = fields.collect::<Vec<_>>().join("\n");
            (!sha.is_empty()).then(|| (sha.to_owned(), agents(&named)))
        })
        .collect()
}

/// The agents `text` (trailers and an author) names, sorted.
pub(crate) fn agents(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    let named: BTreeSet<&str> = AGENTS
        .iter()
        .filter(|(_, names)| {
            names
                .iter()
                .any(|name| words.windows(name.len()).any(|window| window == *name))
        })
        .map(|(id, _)| *id)
        .collect();
    named.into_iter().map(str::to_owned).collect()
}

/// The changed files of `names` (`diff-tree -z --name-status`) with the
/// hunks of `diff` (`diff-tree -p -U0`), which lists them in the same
/// order; without hunks when the two do not line up.
fn changes(names: &str, diff: &str) -> Vec<Change> {
    let mut fields = names.split('\0').filter(|field| !field.is_empty());
    let mut out = Vec::new();
    while let Some(code) = fields.next() {
        let moved = code.starts_with('R') || code.starts_with('C');
        let first = fields.next().unwrap_or_default().to_owned();
        let (path, from) = if moved {
            (fields.next().unwrap_or_default().to_owned(), Some(first))
        } else {
            (first, None)
        };
        let status = match code.chars().next() {
            Some('A') => Status::Added,
            Some('D') => Status::Deleted,
            Some('R') => Status::Renamed,
            Some('C') => Status::Copied,
            _ => Status::Modified,
        };
        out.push(Change {
            path,
            status,
            from,
            hunks: Vec::new(),
        });
    }
    let sections = sections(diff);
    if sections.len() == out.len() {
        for (change, hunks) in out.iter_mut().zip(sections) {
            change.hunks = hunks;
        }
    }
    out
}

/// The hunks of each file section of `patch`, in order.
fn sections(patch: &str) -> Vec<Vec<Hunk>> {
    let mut out: Vec<Vec<Hunk>> = Vec::new();
    for line in patch.split('\n') {
        if line.starts_with("diff --git ") {
            out.push(Vec::new());
        } else if let Some(header) = line.strip_prefix("@@ -") {
            let start = |text: &str| {
                text.split([',', ' '])
                    .next()
                    .and_then(|digits| digits.parse().ok())
                    .unwrap_or(0)
            };
            let new_start = header.split_once(" +").map_or(0, |(_, rest)| start(rest));
            if let Some(hunks) = out.last_mut() {
                hunks.push(Hunk {
                    old_start: start(header),
                    new_start,
                    ..Hunk::default()
                });
            }
        } else if let Some(hunk) = out.last_mut().and_then(|hunks| hunks.last_mut()) {
            if let Some(removed) = line.strip_prefix('-') {
                hunk.removed.push(removed.to_owned());
            } else if let Some(added) = line.strip_prefix('+') {
                hunk.added.push(added.to_owned());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
