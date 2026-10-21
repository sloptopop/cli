//! The `run` command line of one hardened analyzer container: read-only
//! root filesystem and source, no network (except a prepare step that
//! fetches the project's own dependencies), no capabilities, no new
//! privileges, the user's own ids, process, memory and CPU limits, a fixed
//! environment, and only the mounts listed. Nothing here runs anything.
//!
//! Standards: SCF CFG-02 (Secure Baseline Configurations: a fixed,
//! hardened container configuration), CFG-03 (Least Functionality: no
//! network, no capabilities, no host sockets), SEA-04 (Process Isolation).

use std::path::Path;

use crate::collect_catalogue::CONTAINER_ROOT;
use crate::collect_runtime::Runtime;

/// The scratch filesystem every container gets at `/tmp` (its `HOME`)
/// (`exec`, because some tools, Opengrep among them, unpack themselves
/// there and run what they unpacked; the root filesystem stays read-only.)
const TMPFS: &str = "/tmp:rw,exec,nosuid,nodev,size=1g";

/// The most processes a container may start.
const PIDS: &str = "1024";

/// What a container gets mounted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mount {
    /// A host directory.
    Bind {
        /// The host path.
        source: String,
        /// Where it appears in the container.
        target: String,
        /// Whether the container may only read it.
        readonly: bool,
    },
    /// A named volume of the runtime.
    Volume {
        /// The volume's name.
        name: String,
        /// Where it appears in the container.
        target: String,
        /// Whether the container may only read it.
        readonly: bool,
    },
}

impl Mount {
    /// A bind mount of `source` at `target`.
    pub(crate) fn bind(source: &Path, target: &str, readonly: bool) -> Self {
        Self::Bind {
            source: source.to_string_lossy().into_owned(),
            target: target.to_owned(),
            readonly,
        }
    }

    fn flag(&self) -> String {
        let (kind, source, target, readonly) = match self {
            Self::Bind {
                source,
                target,
                readonly,
            } => ("bind", source, target, *readonly),
            Self::Volume {
                name,
                target,
                readonly,
            } => ("volume", name, target, *readonly),
        };
        let readonly = if readonly { ",readonly" } else { "" };
        format!("type={kind},source={source},target={target}{readonly}")
    }
}

/// The resource limits of every container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Limits {
    /// `--memory` (and `--memory-swap`, so no swap), e.g. `4g`.
    pub(crate) memory: String,
    /// `--cpus`, if capped.
    pub(crate) cpus: Option<String>,
}

/// One container to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Spec<'a> {
    /// The runtime.
    pub(crate) runtime: &'a Runtime,
    /// The container's name, unique per run, so it can be stopped.
    pub(crate) name: String,
    /// Whether it gets the default network (a prepare step only).
    pub(crate) network: bool,
    /// The repository owner's `uid:gid`, where the platform has one.
    pub(crate) owner: Option<String>,
    /// The resource limits.
    pub(crate) limits: &'a Limits,
    /// Environment variables beyond the fixed ones.
    pub(crate) env: Vec<(String, String)>,
    /// The mounts, in order.
    pub(crate) mounts: Vec<Mount>,
    /// The executable in the image.
    pub(crate) entrypoint: &'a str,
    /// The image reference or ID to run.
    pub(crate) image: &'a str,
    /// The arguments after the image.
    pub(crate) args: Vec<String>,
}

/// The full command line, program name first.
pub(crate) fn run_argv(spec: &Spec<'_>) -> Vec<String> {
    let mut argv: Vec<String> = [
        spec.runtime.engine.name(),
        "run",
        "--rm",
        "--pull",
        "never",
        "--name",
        &spec.name,
        "--network",
        if spec.network { "bridge" } else { "none" },
        "--read-only",
        "--tmpfs",
        TMPFS,
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
    ]
    .map(str::to_owned)
    .into();
    argv.extend(spec.runtime.user(spec.owner.as_deref()));
    if spec.runtime.limits {
        argv.extend(
            [
                "--pids-limit",
                PIDS,
                "--memory",
                &spec.limits.memory,
                "--memory-swap",
                &spec.limits.memory,
            ]
            .map(str::to_owned),
        );
        if let Some(cpus) = &spec.limits.cpus {
            argv.extend(["--cpus".to_owned(), cpus.clone()]);
        }
    }
    argv.extend(["--hostname".to_owned(), "sloptopop".to_owned()]);
    let fixed = [
        ("HOME", "/tmp"),
        ("TZ", "UTC"),
        ("LANG", "C.UTF-8"),
        ("LC_ALL", "C.UTF-8"),
    ];
    for (name, value) in fixed
        .iter()
        .map(|&(n, v)| (n.to_owned(), v.to_owned()))
        .chain(spec.env.iter().cloned())
    {
        argv.extend(["--env".to_owned(), format!("{name}={value}")]);
    }
    for mount in &spec.mounts {
        argv.extend(["--mount".to_owned(), mount.flag()]);
    }
    argv.extend(
        [
            "--workdir",
            CONTAINER_ROOT,
            "--entrypoint",
            spec.entrypoint,
            spec.image,
        ]
        .map(str::to_owned),
    );
    argv.extend(spec.args.iter().cloned());
    argv
}

#[cfg(test)]
mod tests;
