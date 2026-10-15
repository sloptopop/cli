//! Helpers shared by more than one module's unit tests: a temporary
//! directory, the committed fake engine, and an `initialize` request.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

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

/// The committed fake engine.
pub(crate) const FAKE_ENGINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fake-engine/sloptopop-engine"
);

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
