//! Unit tests for [`super`]: the built-in pack is exactly `rules/`, every
//! rule follows the pack's conventions, and writing it replaces the
//! directory.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::tests::support::TempDir;

fn rules_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("rules")
}

/// Every file under `rules/<area>/`, keyed by (area, file name).
fn files_on_disk() -> BTreeMap<(String, String), PathBuf> {
    let mut files = BTreeMap::new();
    for area in fs::read_dir(rules_dir()).unwrap() {
        let area = area.unwrap().path();
        if !area.is_dir() {
            continue;
        }
        for file in fs::read_dir(&area).unwrap() {
            let file = file.unwrap().path();
            let key = (
                area.file_name().unwrap().to_string_lossy().into_owned(),
                file.file_name().unwrap().to_string_lossy().into_owned(),
            );
            files.insert(key, file);
        }
    }
    files
}

#[test]
fn the_pack_is_every_rule_file_in_the_rules_directory() {
    let on_disk: Vec<(String, String)> = files_on_disk()
        .into_keys()
        .filter(|(_, file)| Path::new(file).extension().is_some_and(|ext| ext == "yaml"))
        .collect();
    let built_in: Vec<(String, String)> = PACK
        .iter()
        .map(|rule| (rule.area.to_owned(), rule.file.to_owned()))
        .collect();
    assert_eq!(built_in, on_disk, "PACK lists rules/*/*.yaml, sorted");
    for rule in PACK {
        let text = fs::read_to_string(rules_dir().join(rule.area).join(rule.file)).unwrap();
        assert_eq!(rule.text, text, "{}/{}", rule.area, rule.file);
    }
}

#[test]
fn every_rule_has_a_stable_id_cwe_metadata_and_a_test_file() {
    let on_disk = files_on_disk();
    for rule in PACK {
        let stem = rule.file.strip_suffix(".yaml").unwrap();
        let id = format!("  - id: sloptopop.{}.{stem}\n", rule.area);
        assert!(
            rule.text.starts_with(&format!("rules:\n{id}")),
            "{}/{}: one rule, id sloptopop.<area>.<file name>",
            rule.area,
            rule.file
        );
        assert_eq!(
            rule.text.matches("\n  - id: ").count(),
            1,
            "{}: one rule per file",
            rule.file
        );
        assert!(
            rule.text.contains("\n      cwe:\n        - \"CWE-"),
            "{}: CWE metadata",
            rule.file
        );
        assert!(
            rule.text.contains("\n    message: "),
            "{}: message",
            rule.file
        );
        let tests: Vec<&(String, String)> = on_disk
            .keys()
            .filter(|(area, file)| {
                area == rule.area
                    && file != rule.file
                    && !file.contains(".fixed.")
                    && file.split('.').next() == Some(stem)
            })
            .collect();
        assert!(
            !tests.is_empty(),
            "{}: annotated test file next to the rule",
            rule.file
        );
        let has_autofix =
            rule.text.contains("\n    fix:") || rule.text.contains("\n    fix-regex:");
        let has_fixed_test = on_disk
            .keys()
            .any(|(area, file)| area == rule.area && file.starts_with(&format!("{stem}.fixed.")));
        assert_eq!(
            has_autofix, has_fixed_test,
            "{}: an autofix has a .fixed. test",
            rule.file
        );
    }
}

#[test]
fn writing_replaces_the_directory() {
    let root = TempDir::new();
    let dir = root.0.join("pack");
    write(&dir).unwrap();
    for rule in PACK {
        assert_eq!(
            fs::read_to_string(dir.join(rule.area).join(rule.file)).unwrap(),
            rule.text,
            "{}",
            rule.file
        );
    }
    let stale = dir.join("errors/dropped-rule.yaml");
    fs::write(&stale, "rules: []\n").unwrap();
    write(&dir).unwrap();
    assert!(!stale.exists(), "a stale rule is removed");
    assert!(
        dir.join(PACK[0].area).join(PACK[0].file).is_file(),
        "written again"
    );
}

#[test]
fn writing_fails_when_the_directory_cannot_be_replaced() {
    let root = TempDir::new();
    let file = root.0.join("pack");
    fs::write(&file, "").unwrap();
    assert!(write(&file).is_err(), "a file in the way");
    let below = file.join("pack");
    assert!(
        write(&below).is_err(),
        "nothing can be created below a file"
    );
}
