//! Unit tests for [`super`].

use std::fs;

use super::*;
use crate::tests::support::TempDir;

#[test]
fn explicit_engine_wins() {
    let locator = Locator {
        explicit: Some(OsString::from("/opt/engine")),
        exe_dir: Some(env::temp_dir()),
        path: Some(OsString::from("/usr/bin")),
    };
    assert_eq!(
        locator.locate(),
        Some(PathBuf::from("/opt/engine")),
        "explicit"
    );
}

#[test]
fn next_to_the_executable_comes_before_path() {
    let exe_dir = TempDir::new();
    let on_path = TempDir::new();
    let name = format!("{ENGINE_NAME}{}", env::consts::EXE_SUFFIX);
    fs::write(on_path.0.join(&name), "").unwrap();
    let path = env::join_paths([on_path.0.clone()]).unwrap();
    let locator = Locator {
        explicit: None,
        exe_dir: Some(exe_dir.0.clone()),
        path: Some(path),
    };
    assert_eq!(
        locator.locate(),
        Some(on_path.0.join(&name)),
        "found on PATH"
    );
    fs::write(exe_dir.0.join(&name), "").unwrap();
    assert_eq!(
        locator.locate(),
        Some(exe_dir.0.join(&name)),
        "next to the executable"
    );
    fs::create_dir(exe_dir.0.join("sub")).unwrap();
    let directory = Locator {
        exe_dir: Some(exe_dir.0.join("sub")),
        ..Locator::default()
    };
    assert_eq!(directory.locate(), None, "nothing anywhere");
}

#[test]
fn process_locator_reads_the_environment() {
    let locator = Locator::from_process();
    assert_eq!(locator.path, env::var_os("PATH"), "PATH");
    assert!(locator.exe_dir.is_some(), "test binary directory");
}
