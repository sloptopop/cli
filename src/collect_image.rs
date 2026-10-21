//! Making sure the pinned image of an analyzer is in the runtime's image
//! store, and is the image the catalogue names, before anything runs from
//! it. A missing image is pulled by its digest (after one printed line
//! saying what is fetched, how big it is, from where, under which
//! licence), and the runtime checks every byte against that digest. The
//! image found is then checked again: the store must list it under the
//! digest (or as the image whose ID is the digest, for an image loaded
//! from an archive), and its labels must name the catalogue's tool and
//! version.
//!
//! Standards: SCF TDA-06.5 (Software Release Integrity Verification: by
//! digest before use), CFG-13 (Open Source Software: the licence is shown
//! before the fetch), PRI-01 (nothing is fetched unannounced).

use std::io::Write;
use std::path::Path;

use serde_json::Value;

use crate::collect_catalogue::Image;
use crate::collect_runtime::{Runtime, reason};

/// `bytes` in megabytes with one decimal, e.g. `10.8 MB`.
pub(crate) fn megabytes(bytes: u64) -> String {
    let tenths = (bytes + 50_000) / 100_000;
    format!("{}.{} MB", tenths / 10, tenths % 10)
}

/// What the store says about an image: its repository digests, its ID and
/// its labels.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Found {
    repo_digests: Vec<String>,
    id: String,
    labels: Value,
}

/// The output of `<runtime> image inspect` for `reference`, if the store
/// has it.
fn inspect(runtime: &Runtime, reference: &str, cwd: &Path) -> Option<Found> {
    let output = runtime
        .output(
            &[
                "image",
                "inspect",
                "--format",
                "{{json .RepoDigests}}|{{.Id}}|{{json .Config.Labels}}",
                reference,
            ],
            cwd,
        )
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let mut fields = text.splitn(3, '|');
    let repo_digests = serde_json::from_str(fields.next().unwrap_or_default()).unwrap_or_default();
    let id = fields.next().unwrap_or_default().to_owned();
    let labels = serde_json::from_str(fields.next().unwrap_or_default()).unwrap_or(Value::Null);
    Some(Found {
        repo_digests,
        id,
        labels,
    })
}

/// The reference to run, if `found` is `image` (`reference` is
/// `<registry>/<name>@<digest>`): the store lists it under the digest, or
/// its ID is the digest. Otherwise why not.
fn verify(found: &Found, image: &Image, reference: &str) -> Result<String, String> {
    let hex = image.digest.trim_start_matches("sha256:");
    let by_digest = found.repo_digests.iter().any(|entry| entry == reference);
    let by_id = found.id.trim_start_matches("sha256:") == hex;
    let run = if by_digest {
        reference.to_owned()
    } else if by_id {
        image.digest.to_owned()
    } else {
        return Err(format!(
            "image digest mismatch: the store has no {reference}"
        ));
    };
    let label = |name: &str| {
        found
            .labels
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let (tool, version) = (
        label("io.sloptopop.tool"),
        label("org.opencontainers.image.version"),
    );
    if tool != image.tool_label() || version != image.version {
        return Err(format!(
            "image labels do not match the catalogue: tool {tool:?}, version {version:?} (expected {:?}, {:?})",
            image.tool_label(),
            image.version
        ));
    }
    Ok(run)
}

/// Makes sure `image` is in the store and verified, pulling it from
/// `registry` unless `offline`; `licence` is shown on the fetch line
/// written to `log`. Returns the reference or ID to run, or why the image
/// cannot be used.
pub(crate) fn ensure(
    runtime: &Runtime,
    registry: &str,
    image: &Image,
    licence: &str,
    offline: bool,
    cwd: &Path,
    log: &mut dyn Write,
) -> Result<String, String> {
    let reference = image.reference(registry);
    let present = inspect(runtime, &reference, cwd).or_else(|| inspect(runtime, image.digest, cwd));
    if let Some(found) = present {
        return verify(&found, image, &reference);
    }
    if offline {
        return Err(format!(
            "image not in the local store, and --offline forbids pulling it: {reference}"
        ));
    }
    let bytes = image.pull_bytes(std::env::consts::ARCH);
    // A log that cannot be written must not stop the run: the record
    // still lists every image.
    let _logged = writeln!(
        log,
        "fetch    {:<16} {:<16} {:>10}  {reference}  {licence}",
        image.tool_label(),
        image.version,
        megabytes(bytes)
    );
    let pulled = runtime
        .output(&["pull", "--quiet", reference.as_str()], cwd)
        .map_err(|e| format!("cannot run {} pull: {e}", runtime.engine.name()))?;
    if !pulled.status.success() {
        return Err(format!("pull failed: {}", reason(&pulled)));
    }
    let found = inspect(runtime, &reference, cwd)
        .ok_or_else(|| format!("pulled, but the store does not list {reference}"))?;
    verify(&found, image, &reference)
}

#[cfg(test)]
mod tests;
