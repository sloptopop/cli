//! Unit tests for [`super`].

use super::*;
use crate::tests::support::TempDir;

#[test]
fn root_patterns_that_ignore_the_directory() {
    for text in [
        ".sloptopop",
        ".sloptopop/",
        "/.sloptopop",
        "/.sloptopop/",
        ".sloptopop/*",
        ".sloptopop/**",
        "  .sloptopop/reports/  ",
        "target\n.sloptopop\n",
        "!.sloptopop\n.sloptopop/",
    ] {
        assert!(ignores_sloptopop(text), "{text:?}");
    }
    for text in [
        "",
        "target/",
        ".sloptopop/rules",
        ".sloptopop/reports/x.sarif",
        "sloptopop",
        "# .sloptopop",
        ".sloptopop\n!.sloptopop/",
        "!.sloptopop",
    ] {
        assert!(!ignores_sloptopop(text), "{text:?}");
    }
}

fn reports(root: &TempDir) -> std::path::PathBuf {
    let reports = root.0.join(".sloptopop/reports");
    fs::create_dir_all(&reports).unwrap();
    reports
}

#[test]
fn a_star_gitignore_is_created_or_completed() {
    let root = TempDir::new();
    let reports = reports(&root);
    let path = reports.join(".gitignore");
    ensure_ignored(&root.0, &reports).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "*\n", "created");
    ensure_ignored(&root.0, &reports).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "*\n", "unchanged");
    fs::write(&path, "!keep.txt").unwrap();
    ensure_ignored(&root.0, &reports).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "!keep.txt\n*\n",
        "completed after a line without a newline"
    );
    fs::write(&path, "# local\n").unwrap();
    ensure_ignored(&root.0, &reports).unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "# local\n*\n",
        "appended"
    );
    fs::write(&path, " * \n").unwrap();
    ensure_ignored(&root.0, &reports).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), " * \n", "already there");
}

#[test]
fn nothing_is_written_when_the_root_ignores_sloptopop() {
    let root = TempDir::new();
    let reports = reports(&root);
    fs::write(root.0.join(".gitignore"), "/.sloptopop/\n").unwrap();
    ensure_ignored(&root.0, &reports).unwrap();
    assert!(!reports.join(".gitignore").exists(), "not created");
}

#[test]
fn unreadable_gitignore_is_an_error() {
    let root = TempDir::new();
    let reports = reports(&root);
    fs::create_dir(reports.join(".gitignore")).unwrap();
    assert!(ensure_ignored(&root.0, &reports).is_err(), "a directory");
}

#[test]
fn an_unreadable_gitignore_is_never_overwritten() {
    let root = TempDir::new();
    let reports = reports(&root);
    fs::write(reports.join(".gitignore"), [0xff, b'\n']).unwrap();
    assert!(ensure_ignored(&root.0, &reports).is_err(), "not UTF-8");
    assert_eq!(
        fs::read(reports.join(".gitignore")).unwrap(),
        [0xff, b'\n'],
        "left as it was"
    );
}
