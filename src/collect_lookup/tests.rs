//! Unit tests for [`super`].

use std::fs;

use super::*;
use crate::tests::support::TempDir;

#[test]
fn the_first_file_on_path_wins() {
    let first = TempDir::new();
    let second = TempDir::new();
    let name = format!("tool{}", env::consts::EXE_SUFFIX);
    fs::create_dir(first.0.join(&name)).unwrap();
    fs::write(second.0.join(&name), "").unwrap();
    let path = env::join_paths([first.0.clone(), second.0.clone()]).unwrap();
    assert_eq!(
        find(Some(&path), "tool"),
        Some(second.0.join(&name)),
        "directories are skipped"
    );
    fs::write(first.0.join("other"), "").unwrap();
    assert_eq!(
        find(Some(&path), "other"),
        Some(first.0.join("other")),
        "first"
    );
    assert_eq!(find(Some(&path), "absent"), None, "absent");
    assert_eq!(find(None, "tool"), None, "no PATH");
}

#[test]
fn empty_path_entries_do_not_mean_the_current_directory() {
    let dir = TempDir::new();
    // The tests run in the crate root, which holds a Cargo.toml.
    let path = env::join_paths([dir.0.join("missing"), PathBuf::new()]).unwrap();
    assert_eq!(find(Some(&path), "Cargo.toml"), None, "empty entry skipped");
}
