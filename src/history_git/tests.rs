//! Unit tests for [`super`]; the range tests run the real `git`.

use std::path::Path;

use super::*;
use crate::tests::support::{TempDir, commit, git, repository};

#[test]
fn agents_are_named_by_their_words() {
    for (text, named) in [
        ("Co-authored-by: Claude <agent>", &["claude"][..]),
        ("Co-authored-by: Cursor Agent <cursoragent>", &["cursor"]),
        ("devin-ai-integration[bot]", &["devin"]),
        ("Co-authored-by: Devin Smith <d>", &[]),
        ("Reviewed-by: someone who declined", &[]),
        ("Assisted-by: Amazon Q", &["amazon-q"]),
        (
            "Co-authored-by: Copilot\nGenerated-by: Claude\nX: claude",
            &["claude", "copilot"],
        ),
        ("", &[]),
    ] {
        assert_eq!(agents(text), named, "{text:?}");
    }
}

#[test]
fn log_records_are_ids_and_agents() {
    let log = "\u{1e}abc\u{1f}Tester\u{1f}tester\u{1f}Co-authored-by: Claude <agent>\n\n\
               \u{1e}def\u{1f}copilot[bot]\u{1f}x\u{1f}\n\u{1e}\u{1f}\u{1f}\u{1f}\n";
    assert_eq!(
        commits(log),
        [
            ("abc".to_owned(), vec!["claude".to_owned()]),
            ("def".to_owned(), vec!["copilot".to_owned()]),
        ],
        "an empty record is skipped"
    );
}

#[test]
fn changes_line_up_with_their_hunks() {
    let names = "M\0a.ts\0R090\0old.ts\0new.ts\0A\0b.md\0D\0c.py\0C075\0e0.py\0e.py\0T\0link\0";
    let patch = "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n\
                 @@ -3,2 +4 @@ fn x\n-old one\n-old two\n+new\n\\ No newline at end of file\n\
                 @@ -9 +10,0 @@\n-gone\n\
                 diff --git a/old.ts b/new.ts\nsimilarity index 90%\n@@ -1 +1 @@\n-a\n+b\n\
                 diff --git a/b.md b/b.md\nnew file mode 100644\n@@ -0,0 +1 @@\n+# B\n\
                 diff --git a/c.py b/c.py\ndeleted file mode 100644\nBinary files a/c.py and /dev/null differ\n\
                 diff --git a/e0.py b/e.py\n\
                 diff --git a/link b/link\n";
    let found = changes(names, patch);
    let shape: Vec<(&str, Status, Option<&str>, usize)> = found
        .iter()
        .map(|c| (c.path.as_str(), c.status, c.from.as_deref(), c.hunks.len()))
        .collect();
    assert_eq!(
        shape,
        [
            ("a.ts", Status::Modified, None, 2),
            ("new.ts", Status::Renamed, Some("old.ts"), 1),
            ("b.md", Status::Added, None, 1),
            ("c.py", Status::Deleted, None, 0),
            ("e.py", Status::Copied, Some("e0.py"), 0),
            ("link", Status::Modified, None, 0),
        ],
        "statuses and paths"
    );
    assert_eq!(
        found[0].hunks,
        [
            Hunk {
                old_start: 3,
                new_start: 4,
                removed: vec!["old one".to_owned(), "old two".to_owned()],
                added: vec!["new".to_owned()],
            },
            Hunk {
                old_start: 9,
                new_start: 10,
                removed: vec!["gone".to_owned()],
                added: Vec::new(),
            },
        ],
        "hunks"
    );
    assert_eq!(
        (found[2].hunks[0].old_start, found[2].hunks[0].new_start),
        (0, 1),
        "a new file"
    );
    let short = changes(names, "diff --git a/a.ts b/a.ts\n@@ -1 +1 @@\n-a\n+b\n");
    assert!(
        short.iter().all(|change| change.hunks.is_empty()),
        "listings that do not line up give no hunks"
    );
    assert_eq!(
        sections("@@ -1 +1 @@\n-a\n+b\n"),
        Vec::<Vec<Hunk>>::new(),
        "no file"
    );
    assert_eq!(
        sections("diff --git a b\n@@ -x +y @@\n@@ -1\n"),
        [vec![
            Hunk::default(),
            Hunk {
                old_start: 1,
                ..Hunk::default()
            }
        ]],
        "unreadable starts are 0"
    );
    assert_eq!(Status::Added.as_str(), "added", "added");
    assert_eq!(Status::Deleted.as_str(), "deleted", "deleted");
    assert_eq!(Status::Modified.as_str(), "modified", "modified");
    assert_eq!(Status::Renamed.as_str(), "renamed", "renamed");
    assert_eq!(Status::Copied.as_str(), "copied", "copied");
}

#[test]
fn the_range_runs_from_the_merge_base_oldest_first() {
    let dir = TempDir::new();
    let root = repository(&dir);
    let base = git(root, &["rev-parse", "HEAD"]);
    commit(
        root,
        "a.test.ts",
        "expect(a).toBeDefined()\n",
        "Weaken\n\nCo-authored-by: Claude <agent>",
    );
    git(root, &["mv", "a.test.ts", "b.test.ts"]);
    git(root, &["commit", "-q", "-m", "Move"]);
    commit(root, "notes.md", "# Notes\n", "Notes");
    let shas: Vec<String> = git(root, &["rev-list", "--reverse", &format!("{base}..HEAD")])
        .lines()
        .map(str::to_owned)
        .collect();
    let git_program = Path::new("git");
    let range = range(git_program, root, MAX_COMMITS).unwrap();
    assert_eq!(range.base, base, "the merge base");
    assert_eq!(range.branch, "main", "the default branch");
    assert!(!range.truncated, "all of it");
    assert_eq!(
        range
            .commits
            .iter()
            .map(|c| c.sha.clone())
            .collect::<Vec<_>>(),
        shas,
        "oldest first"
    );
    let first = &range.commits[0];
    assert_eq!(first.agents, ["claude"], "the trailer");
    assert_eq!(
        first.files,
        [Change {
            path: "a.test.ts".to_owned(),
            status: Status::Modified,
            from: None,
            hunks: vec![Hunk {
                old_start: 1,
                new_start: 1,
                removed: vec!["expect(a).toBe(2)".to_owned()],
                added: vec!["expect(a).toBeDefined()".to_owned()],
            }],
        }],
        "the change"
    );
    let moved = &range.commits[1].files[0];
    assert_eq!(
        (moved.path.as_str(), moved.status, moved.from.as_deref()),
        ("b.test.ts", Status::Renamed, Some("a.test.ts")),
        "the rename"
    );
    assert!(range.commits[1].agents.is_empty(), "no agent");
    let three = super::range(git_program, root, 3).unwrap();
    assert!(!three.truncated, "exactly the limit");
    let two = super::range(git_program, root, 2).unwrap();
    assert!(two.truncated, "over the limit");
    assert_eq!(
        two.commits
            .iter()
            .map(|c| c.sha.clone())
            .collect::<Vec<_>>(),
        shas[1..],
        "the newest"
    );
}

#[test]
fn no_git_no_branch_no_commits_or_no_history_is_no_range() {
    let git_program = Path::new("git");
    let dir = TempDir::new();
    assert_eq!(
        range(git_program, &dir.0, MAX_COMMITS),
        None,
        "not a repository"
    );
    let root = repository(&dir);
    assert_eq!(
        range(&dir.0.join("no-git"), root, MAX_COMMITS),
        None,
        "git does not run"
    );
    assert_eq!(
        range(git_program, root, MAX_COMMITS),
        None,
        "no commit since the base"
    );
    commit(root, "b.ts", "b\n", "B");
    assert!(range(git_program, root, MAX_COMMITS).is_some(), "a commit");
    git(root, &["checkout", "-q", "--detach"]);
    assert_eq!(range(git_program, root, MAX_COMMITS), None, "detached");
    git(root, &["checkout", "-q", "feature"]);
    git(root, &["branch", "-q", "-m", "main", "trunk"]);
    assert_eq!(
        range(git_program, root, MAX_COMMITS),
        None,
        "no default branch"
    );
    let shallow = TempDir::new();
    let url = format!("file://{}", root.display());
    git(
        &shallow.0,
        &[
            "clone", "-q", "--depth", "1", "--branch", "feature", &url, "copy",
        ],
    );
    assert_eq!(
        range(git_program, &shallow.0.join("copy"), MAX_COMMITS),
        None,
        "shallow"
    );
}
