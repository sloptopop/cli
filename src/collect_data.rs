//! The vulnerability data the scanners read offline: daily snapshots the
//! component library serves as small data images (`db-osv-<ecosystem>`,
//! `db-govulndb`). The one input `collect` does not pin per release:
//! each run uses the newest snapshot (the `latest` tag, or the date given
//! with `--vuln-db`), records it by digest, and unpacks it once into the
//! user's cache, from where it is mounted read-only. `--offline` uses the
//! newest snapshot already cached.
//!
//! Standards: SCF VPM-12 (Vulnerability Scanning: current data),
//! TDA-06.5 (Software Release Integrity Verification: by digest), MON-09
//! (the snapshot used is recorded).

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::collect_catalogue::Data;
use crate::collect_detect::Ecosystem;
use crate::collect_runtime::{Runtime, reason};

/// One snapshot image and where its data goes in the container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Snapshot {
    /// The image name below `analyzers/`, e.g. `db-osv-pypi`.
    pub(crate) name: &'static str,
    /// The directory in the image (below `/db`) and in the container.
    pub(crate) path: &'static str,
}

/// The OSV snapshot of each ecosystem that has OSV data.
fn osv(ecosystem: Ecosystem) -> Option<Snapshot> {
    let (name, path) = match ecosystem {
        Ecosystem::Python => ("db-osv-pypi", "osv-scalibr/PyPI"),
        Ecosystem::Node => ("db-osv-npm", "osv-scalibr/npm"),
        Ecosystem::Go => ("db-osv-go", "osv-scalibr/Go"),
        Ecosystem::Rust => ("db-osv-crates-io", "osv-scalibr/crates.io"),
        Ecosystem::Jvm => ("db-osv-maven", "osv-scalibr/Maven"),
        Ecosystem::Php => ("db-osv-packagist", "osv-scalibr/Packagist"),
        Ecosystem::Dotnet => ("db-osv-nuget", "osv-scalibr/NuGet"),
        Ecosystem::Swift => ("db-osv-swifturl", "osv-scalibr/SwiftURL"),
        Ecosystem::Cpp | Ecosystem::Powershell | Ecosystem::Shell => return None,
    };
    Some(Snapshot { name, path })
}

/// The snapshots `data` needs for a repository using `ecosystems`.
pub(crate) fn snapshots(data: Data, ecosystems: &BTreeSet<Ecosystem>) -> Vec<Snapshot> {
    match data {
        Data::Osv => ecosystems.iter().filter_map(|e| osv(*e)).collect(),
        Data::GoVuln => vec![Snapshot {
            name: "db-govulndb",
            path: "govulndb",
        }],
    }
}

/// A snapshot ready to mount.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved {
    /// Which snapshot.
    pub(crate) snapshot: Snapshot,
    /// `<registry>/analyzers/<name>@<digest>`.
    pub(crate) reference: String,
    /// The snapshot's date, `YYYYMMDD`.
    pub(crate) version: String,
    /// The unpacked directory to mount at `/db/<path>`.
    pub(crate) dir: PathBuf,
}

/// What resolving needs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Context<'a> {
    /// The runtime.
    pub(crate) runtime: &'a Runtime,
    /// The registry.
    pub(crate) registry: &'a str,
    /// The user's cache directory for sloptopop.
    pub(crate) cache: &'a Path,
    /// Use only what is cached.
    pub(crate) offline: bool,
    /// The snapshot date to use instead of the newest.
    pub(crate) date: Option<&'a str>,
    /// Where to run the runtime from.
    pub(crate) cwd: &'a Path,
    /// A unique suffix for the temporary container's name.
    pub(crate) nonce: &'a str,
}

/// The cache directory of `snapshot`'s unpacked copies.
fn store(cache: &Path, snapshot: Snapshot) -> PathBuf {
    cache.join("db").join(snapshot.name)
}

/// The newest complete copy of `snapshot` in the cache (by its date), if
/// any: `(version, digest hex, directory)`.
fn newest_cached(cache: &Path, snapshot: Snapshot) -> Option<(String, String, PathBuf)> {
    fs::read_dir(store(cache, snapshot))
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let version = fs::read_to_string(dir.join(".version")).ok()?;
            let hex = entry.file_name().to_string_lossy().into_owned();
            Some((version.trim().to_owned(), hex, dir))
        })
        .max()
}

/// Makes `snapshot` available, writing a fetch line to `log` before any
/// download.
pub(crate) fn resolve(
    context: &Context<'_>,
    snapshot: Snapshot,
    log: &mut dyn Write,
) -> Result<Resolved, String> {
    let repository = format!("{}/analyzers/{}", context.registry, snapshot.name);
    if context.offline {
        let (version, hex, dir) = newest_cached(context.cache, snapshot).ok_or_else(|| {
            format!(
                "no cached {} snapshot, and --offline forbids fetching one",
                snapshot.name
            )
        })?;
        return Ok(Resolved {
            snapshot,
            reference: format!("{repository}@sha256:{hex}"),
            version,
            dir: dir.join(snapshot.path),
        });
    }
    let tag = format!("{repository}:{}", context.date.unwrap_or("latest"));
    let _logged = writeln!(
        log,
        "fetch    {:<16} {tag}  CC-BY-4.0 (vulnerability data)",
        snapshot.name
    );
    let run = |args: &[&str]| run(context, args);
    run(&["pull", "--quiet", &tag]).map_err(|e| format!("cannot fetch {tag}: {e}"))?;
    let inspected = run(&[
        "image",
        "inspect",
        "--format",
        "{{json .RepoDigests}}|{{index .Config.Labels \"org.opencontainers.image.version\"}}",
        &tag,
    ])
    .map_err(|e| format!("cannot inspect {tag}: {e}"))?;
    let (digests, version) = inspected.split_once('|').unwrap_or((&inspected, ""));
    let digests: Vec<String> = serde_json::from_str(digests).unwrap_or_default();
    let reference = digests
        .into_iter()
        .find(|entry| entry.starts_with(&format!("{repository}@sha256:")))
        .ok_or_else(|| format!("the store lists no digest of {repository} for {tag}"))?;
    let hex = reference.rsplit(':').next().unwrap_or_default().to_owned();
    let dir = store(context.cache, snapshot).join(&hex);
    if !dir.join(".version").is_file() {
        unpack(context, &reference, &dir, version)
            .map_err(|e| format!("cannot unpack {reference}: {e}"))?;
    }
    Ok(Resolved {
        snapshot,
        reference,
        version: version.to_owned(),
        dir: dir.join(snapshot.path),
    })
}

/// Runs the runtime with `args`: its trimmed stdout, or why it failed.
fn run(context: &Context<'_>, args: &[&str]) -> Result<String, String> {
    let runtime = context.runtime;
    let output = runtime
        .output(args, context.cwd)
        .map_err(|e| format!("cannot run {}: {e}", runtime.engine.name()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(reason(&output))
    }
}

/// Copies `/db` out of the snapshot image `reference` into `dir` (through a
/// created, never started container), then marks it complete.
fn unpack(context: &Context<'_>, reference: &str, dir: &Path, version: &str) -> io::Result<()> {
    let partial = dir.with_extension("partial");
    let _stale = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial)?;
    let name = format!("sloptopop-db-{}", context.nonce);
    let step = |args: &[&str]| run(context, args).map(drop).map_err(io::Error::other);
    step(&["create", "--name", &name, reference, "none"])?;
    let target = partial.to_string_lossy().into_owned();
    let copied = step(&["cp", &format!("{name}:/db/."), &target]);
    let removed = step(&["rm", "--force", &name]);
    copied.and(removed)?;
    fs::write(partial.join(".version"), version)?;
    let _replaced = fs::remove_dir_all(dir);
    fs::rename(&partial, dir)
}

/// The number of whole days from snapshot `version` (`YYYYMMDD`) to
/// `today` (days since 1970-01-01), if the version is a date.
pub(crate) fn age(version: &str, today: i64) -> Option<i64> {
    let (year, rest) = version.split_at_checked(4)?;
    let (month, day) = rest.split_at_checked(2)?;
    let year: i64 = year.parse().ok()?;
    let month: i64 = month.parse().ok()?;
    let day: i64 = day.parse().ok()?;
    if version.len() != 8 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(today - (era * 146_097 + doe - 719_468))
}

#[cfg(test)]
mod tests;
