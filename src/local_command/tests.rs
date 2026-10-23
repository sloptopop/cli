//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use std::io;
use std::path::Path;

use super::*;
use crate::tests::support::{FAKE_LOCAL, TempDir, explicit};

/// A writer that always fails.
struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("broken"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("broken"))
    }
}

fn run(locator: &Locator, args: &[&str]) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let code = local(locator, &args, &mut out, &mut err);
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[test]
fn arguments_output_and_exit_codes_pass_through() {
    let fake = explicit(Path::new(FAKE_LOCAL));
    let (code, out, err) = run(&fake, &["version"]);
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "sloptopop-local 0.1.0\n", "fake-local: version\n"),
        "version"
    );
    assert_eq!(run(&fake, &["broken"]).0, 3, "its failure");
    assert_eq!(run(&fake, &["other"]).0, 2, "a usage error");
    assert_eq!(run(&fake, &["killed"]).0, 3, "a signal");
    let mut sink = Vec::new();
    assert_eq!(
        local(&fake, &["version".to_owned()], &mut Broken, &mut sink),
        0,
        "the exit code survives a broken stdout"
    );
}

#[test]
fn a_missing_or_unstartable_local_exits_3() {
    let (code, _, err) = run(&Locator::default(), &["version"]);
    assert_eq!((code, err.as_str()), (3, NOT_FOUND), "missing");
    let dir = TempDir::new();
    let (status, _, message) = run(&explicit(&dir.0.join("nope")), &["version"]);
    assert!(
        status == 3 && message.starts_with("error: cannot run "),
        "{message}"
    );
}
