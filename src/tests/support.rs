//! Helpers shared by more than one module's unit tests: a temporary
//! directory, the committed fake engine, and an `initialize` request.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use std::ffi::OsString;

use crate::collect_catalogue::{CATALOGUE, Kind};
use crate::engine_locate::Locator;
use crate::handshake::PROTOCOL_VERSION;

/// A directory removed again when dropped.
pub(crate) struct TempDir(pub(crate) PathBuf);

impl TempDir {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = env::temp_dir().join(format!(
            "sloptopop-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _stale = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

/// The committed fake `sloptopop-local`.
pub(crate) const FAKE_LOCAL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fake-local/sloptopop-local"
);

/// The directory of the committed fake curl.
pub(crate) const CURL_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-curl");

/// A locator with the fake `sloptopop-local` and the fake curl on `PATH`.
pub(crate) fn fake_local() -> Locator {
    Locator {
        explicit: Some(OsString::from(FAKE_LOCAL)),
        exe_dir: None,
        path: Some(OsString::from(CURL_DIR)),
        ..Locator::default()
    }
}

/// A locator that only knows `engine`.
pub(crate) fn explicit(engine: &Path) -> Locator {
    Locator {
        explicit: Some(engine.as_os_str().to_owned()),
        ..Locator::default()
    }
}

/// An `initialize` request for protocol version [`PROTOCOL_VERSION`].
pub(crate) fn initialize(id: i64) -> Value {
    json!({
        "jsonrpc": "2.0", "id": id, "method": "initialize",
        "params": { "clientName": "test", "clientVersion": "0", "protocolVersion": PROTOCOL_VERSION },
    })
}

/// Directory of the committed fake analyzers in `mode` (`findings`,
/// `broken` or `killed`); see `tests/fake-tools/fake-tool`.
pub(crate) fn fake_tools(mode: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake-tools")
        .join(mode)
}

/// `PATH` for the fake container runtimes in `mode` (see
/// `tests/fake-runtime/runtime`): their directory, then the system's.
pub(crate) fn runtime_path(mode: &str) -> OsString {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fake-runtime")
        .join(mode);
    let mut path = dir.into_os_string();
    path.push(":/bin:/usr/bin");
    path
}

/// Writes `.fake-versions` in `root`: every catalogue image's tool label
/// and version, which the fake runtime puts in its labels.
pub(crate) fn fake_versions(root: &Path) {
    let lines: String = CATALOGUE
        .iter()
        .filter_map(|tool| match tool.kind {
            Kind::Run(image) | Kind::NeedsGit(image) => {
                Some(format!("{} {}\n", image.tool_label(), image.version))
            }
            Kind::Documented => None,
        })
        .collect();
    fs::write(root.join(".fake-versions"), lines).unwrap();
}
