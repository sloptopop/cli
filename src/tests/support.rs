//! Helpers shared by more than one module's unit tests: a temporary
//! directory, the committed fake engine, and an `initialize` request.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use crate::collect_catalogue::Inputs;
use crate::collect_managed::{Managed, Registry, TOOLS_DIR};
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

/// Rendering inputs for argument lists: `rules`, with `TOOLS` as the
/// managed tools directory and `ESLINT` as the `ESLint` script.
pub(crate) fn inputs(rules: &[String]) -> Inputs<'_> {
    Inputs {
        rules,
        tools: "TOOLS",
        eslint: "ESLINT",
    }
}

/// Installs, as far as `collect` can tell, every pinned package of every
/// managed tool in `root`: the npm manifests at the pinned versions and the
/// `NuGet` package's recorded hash.
pub(crate) fn install_managed(root: &Path) {
    for managed in [Managed::EslintSonarjs, Managed::SonarCsharp] {
        let dir = root.join(TOOLS_DIR).join(managed.dir());
        for pin in managed.pins() {
            let (file, text) = match pin.registry {
                Registry::Npm => (
                    dir.join("node_modules")
                        .join(pin.package)
                        .join("package.json"),
                    format!(r#"{{"version":"{}"}}"#, pin.version),
                ),
                Registry::Nuget => {
                    let id = pin.package.to_ascii_lowercase();
                    (
                        dir.join("packages")
                            .join(&id)
                            .join(pin.version)
                            .join(format!("{id}.{}.nupkg.sha512", pin.version)),
                        pin.integrity.trim_start_matches("sha512-").to_owned(),
                    )
                }
            };
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }
    }
}
