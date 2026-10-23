//! Unit tests for [`super`]: the reviewed table, Biome's kinds and the
//! configuration guard.

use std::fs;

use super::*;
use crate::tests::support::TempDir;

#[test]
fn image_versions_have_an_upstream_version() {
    assert_eq!(upstream("0.17.1-1"), "0.17.1");
    assert_eq!(upstream("9.32.0.97167-1"), "9.32.0.97167");
    assert_eq!(upstream("ruff 0.17.1"), "ruff 0.17.1");
}

#[test]
fn a_fix_is_reviewed_only_for_its_tool_version_rule_and_level() {
    assert!(reviewed("ruff", "0.17.1-1", "UP045", Level::Safe));
    assert!(
        reviewed("ruff", "0.17.1-1", "I001", Level::Safe),
        "owner default O-1"
    );
    assert!(reviewed(
        "biome",
        "2.5.15-1",
        "lint/style/useImportType",
        Level::Safe
    ));
    assert!(reviewed(
        "biome",
        "2.5.15-1",
        "lint/suspicious/noTsIgnore",
        Level::Safe
    ));
    assert!(reviewed(
        "eslint-sonarjs",
        "2.0.4-1",
        "sonarjs/no-var",
        Level::Safe
    ));
    assert!(
        !reviewed("ruff", "0.17.1-1", "UP045", Level::Unsafe),
        "another level"
    );
    assert!(
        !reviewed("ruff", "0.18.0-1", "UP045", Level::Safe),
        "another version"
    );
    assert!(
        !reviewed("ruff", "0.17.1-1", "UP030", Level::Safe),
        "not reviewed"
    );
    assert!(
        !reviewed("biome", "0.17.1-1", "UP045", Level::Safe),
        "another tool"
    );
}

#[test]
fn a_rule_is_reviewed_at_any_level_for_its_tool_and_version() {
    assert!(reviewed_rule(
        "clang-tidy",
        "19.1.0-3",
        "bugprone-misplaced-operator-in-strlen-in-alloc"
    ));
    assert!(reviewed_rule("shellcheck", "0.11.0-1", "SC2086"));
    assert!(
        !reviewed_rule(
            "clang-tidy",
            "19.1.0-3",
            "bugprone-suspicious-string-compare"
        ),
        "a style fix: not reviewed (Q-6)"
    );
    assert!(
        !reviewed_rule("clang-tidy", "20.1.0-1", "bugprone-suspicious-semicolon"),
        "another version"
    );
    assert!(
        !reviewed_rule("shellcheck", "0.11.0-1", "bugprone-suspicious-semicolon"),
        "another tool"
    );
}

#[test]
fn every_table_row_is_well_formed() {
    for row in rows(REVIEWED) {
        let [tool, version, rule, level, state, note] = row.as_slice() else {
            panic!("{row:?}");
        };
        assert!(
            [
                "ruff",
                "eslint-sonarjs",
                "biome",
                "shellcheck",
                "clang-tidy"
            ]
            .contains(tool),
            "{row:?}"
        );
        assert!(!version.is_empty() && !rule.is_empty() && !note.is_empty());
        assert_eq!(*level, "safe", "only safe fixes are reviewed: {row:?}");
        assert!(["on", "off"].contains(state), "{row:?}");
    }
    assert_eq!(rows(REVIEWED).count(), 45);
    for row in rows(BIOME_KINDS) {
        assert!(
            matches!(row.as_slice(), [rule, "safe" | "unsafe"] if rule.starts_with("lint/")),
            "{row:?}"
        );
    }
}

#[test]
fn biome_kinds_come_from_the_pinned_version() {
    assert_eq!(
        biome_kind("2.5.15-1", "lint/style/useImportType"),
        Some(Level::Safe)
    );
    assert_eq!(
        biome_kind("2.5.15-1", "lint/style/noNonNullAssertion"),
        Some(Level::Unsafe)
    );
    assert_eq!(
        biome_kind("2.5.15-1", "lint/suspicious/noExplicitAny"),
        None
    );
    assert_eq!(biome_kind("2.6.0-1", "lint/style/useImportType"), None);
}

#[test]
fn configurations_that_can_change_fix_safety_are_found() {
    let root = TempDir::new();
    assert_eq!(promoting_config(&root.0, Guard::Ruff), None);
    fs::create_dir_all(root.0.join("pkg/inner")).unwrap();
    fs::create_dir_all(root.0.join(".hidden")).unwrap();
    fs::create_dir_all(root.0.join("node_modules")).unwrap();
    fs::write(root.0.join(".hidden/ruff.toml"), "extend-safe-fixes = []").unwrap();
    fs::write(root.0.join("node_modules/biome.json"), r#"{"fix": "safe"}"#).unwrap();
    fs::write(root.0.join("pyproject.toml"), "[tool.ruff]\n").unwrap();
    fs::write(root.0.join("notes.txt"), "extend-safe-fixes").unwrap();
    fs::write(root.0.join("biome.json"), r#"{"linter": {}}"#).unwrap();
    fs::write(root.0.join("pkg/x.json"), r#"{"fix": "safe"}"#).unwrap();
    assert_eq!(
        promoting_config(&root.0, Guard::Ruff),
        None,
        "hidden and other files"
    );
    assert_eq!(promoting_config(&root.0, Guard::Biome), None);
    fs::write(
        root.0.join("pkg/inner/.ruff.TOML"),
        "extend-safe-fixes = [\"UP\"]",
    )
    .unwrap();
    fs::write(root.0.join("pkg/inner/biome.jsonc"), r#"{ "fix": "safe" }"#).unwrap();
    fs::write(root.0.join("pkg/biome.json"), r#"{"x": {"fix": "unsafe"}}"#).unwrap();
    assert_eq!(
        promoting_config(&root.0, Guard::Ruff),
        Some("pkg/inner/.ruff.TOML".to_owned())
    );
    assert_eq!(
        promoting_config(&root.0, Guard::Biome),
        Some("pkg/biome.json".to_owned()),
        "the first in path order"
    );
    fs::write(root.0.join("pkg/biome.json"), [0xFF]).unwrap();
    assert_eq!(
        promoting_config(&root.0, Guard::Biome),
        Some("pkg/inner/biome.jsonc".to_owned()),
        "an unreadable file decides nothing"
    );
}
