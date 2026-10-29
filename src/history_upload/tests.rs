//! Unit tests for [`super`]; `attach` runs the real `git`.

use std::env;
use std::fs;

use serde_json::{Value, json};

use super::*;
use crate::history_git::{Change, Commit, Hunk, Range, Status};
use crate::tests::support::{TempDir, commit, repository};

fn sha(digit: char) -> String {
    digit.to_string().repeat(40)
}

/// A range of three commits; the first changes a test (with a secret), a
/// file outside the snapshot, and renames another.
fn sample() -> Range {
    let change = |path: &str, status, from: Option<&str>, hunks| Change {
        path: path.to_owned(),
        status,
        from: from.map(str::to_owned),
        hunks,
    };
    let plain = |sha| Commit {
        sha,
        agents: Vec::new(),
        files: Vec::new(),
    };
    Range {
        base: sha('b'),
        branch: "origin/main".to_owned(),
        truncated: false,
        commits: vec![
            Commit {
                sha: sha('1'),
                agents: vec!["claude".to_owned()],
                files: vec![
                    change(
                        "a.test.ts",
                        Status::Modified,
                        None,
                        vec![Hunk {
                            old_start: 3,
                            new_start: 4,
                            removed: vec!["const token = 'hunter22'".to_owned()],
                            added: vec!["expect(a).toBeDefined()".to_owned()],
                        }],
                    ),
                    change(
                        "left-out.env",
                        Status::Modified,
                        None,
                        vec![Hunk {
                            added: vec!["X=1".to_owned()],
                            ..Hunk::default()
                        }],
                    ),
                    change("b.ts", Status::Renamed, Some("a.ts"), Vec::new()),
                ],
            },
            plain(sha('2')),
            plain(sha('3')),
        ],
    }
}

fn snapshot() -> BTreeSet<&'static str> {
    BTreeSet::from(["a.test.ts", "b.ts"])
}

#[test]
fn the_document_is_what_the_detector_reads_masked() {
    let (document, attached) = document(&sample(), &snapshot(), usize::MAX).unwrap();
    assert_eq!(
        document,
        json!({"schemaVersion": 1, "base": sha('b'), "truncated": false, "commits": [
            {"sha": sha('1'), "agents": ["claude"], "files": [
                {"path": "a.test.ts", "status": "modified", "hunks": [
                    {"oldStart": 3, "newStart": 4, "removed": ["const token = 'xxxxxxxx'"],
                     "added": ["expect(a).toBeDefined()"]}]},
                {"path": "left-out.env", "status": "modified"},
                {"path": "b.ts", "status": "renamed", "from": "a.ts", "hunks": []}]},
            {"sha": sha('2'), "agents": [], "files": []},
            {"sha": sha('3'), "agents": [], "files": []}]}),
        "masked, no hunks outside the snapshot"
    );
    assert_eq!(
        attached,
        Attached {
            commits: 3,
            bytes: document.to_string().len(),
            branch: "origin/main".to_owned(),
            truncated: false,
        },
        "what it holds"
    );
}

#[test]
fn the_oldest_commits_go_first_when_it_does_not_fit() {
    let range = sample();
    let full = document(&range, &snapshot(), usize::MAX).unwrap().1.bytes;
    let (_, exact) = document(&range, &snapshot(), full).unwrap();
    assert_eq!((exact.commits, exact.truncated), (3, false), "exactly fits");
    let (smaller, less) = document(&range, &snapshot(), full - 1).unwrap();
    assert_eq!((less.commits, less.truncated), (2, true), "one left out");
    assert_eq!(smaller["commits"][0]["sha"], sha('2'), "the oldest went");
    assert_eq!(smaller["truncated"], true, "said so");
    assert_eq!(document(&range, &snapshot(), 10), None, "nothing fits");
    let mut cut = sample();
    cut.truncated = true;
    assert!(
        document(&cut, &snapshot(), usize::MAX).unwrap().1.truncated,
        "git's cut stays"
    );
}

#[test]
fn the_manifest_line_names_the_range() {
    let mut attached = Attached {
        commits: 3,
        bytes: 120,
        branch: "origin/main".to_owned(),
        truncated: false,
    };
    assert_eq!(
        line(&attached),
        "  commit range: 3 commit(s) since the merge base with origin/main, 120 bytes, lines masked (--no-history leaves it out)\n",
        "whole"
    );
    attached.truncated = true;
    assert!(
        line(&attached).contains("origin/main, the newest only, 120"),
        "cut"
    );
}

#[test]
fn the_range_is_added_to_the_request_file() {
    let dir = TempDir::new();
    let root = repository(&dir);
    commit(root, "a.test.ts", "expect(a).toBeDefined()\n", "Weaken");
    let request = dir.0.join("request.json");
    fs::write(
        &request,
        r#"{"schemaVersion":2,"files":[{"path":"a.test.ts","text":"x"}]}"#,
    )
    .unwrap();
    let path = env::var_os("PATH");
    let attached = attach(path.as_deref(), root, &request).unwrap();
    assert_eq!(
        (attached.commits, attached.branch.as_str()),
        (1, "main"),
        "one commit"
    );
    let written: Value = serde_json::from_str(&fs::read_to_string(&request).unwrap()).unwrap();
    assert_eq!(written["schemaVersion"], 2, "the request kept");
    assert_eq!(
        written["history"]["commits"][0]["files"][0]["hunks"][0]["added"],
        json!(["expect(a).toBeDefined()"]),
        "{written}"
    );
    assert_eq!(attach(None, root, &request), None, "no git");
    let other = TempDir::new();
    assert_eq!(
        attach(path.as_deref(), &other.0, &request),
        None,
        "no range"
    );
    for (text, why) in [("not json", "garbage"), ("[]", "not an object")] {
        fs::write(&request, text).unwrap();
        assert_eq!(attach(path.as_deref(), root, &request), None, "{why}");
        assert_eq!(
            fs::read_to_string(&request).unwrap(),
            text,
            "{why}: unchanged"
        );
    }
    fs::remove_file(&request).unwrap();
    assert_eq!(attach(path.as_deref(), root, &request), None, "no request");
}

#[test]
fn the_limits_stay_one_mebibyte_under_the_servers() {
    // 31 MiB of history under the server's 32 MiB, 63 MiB of request under
    // its 64 MiB body limit.
    assert_eq!(MAX_BYTES, 32_505_856);
    assert_eq!(MAX_REQUEST, 66_060_288);
    assert_eq!(MAX_BYTES + 1_048_576, 32 << 20);
    assert_eq!(MAX_REQUEST + 1_048_576, 64 << 20);
}
