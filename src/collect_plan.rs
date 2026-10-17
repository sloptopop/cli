//! Deciding, for each catalogue entry, whether and how `collect` runs it:
//! the argument list, the program (the tool on `PATH`, or `docker` with a
//! digest-pinned image) and where its report goes. Nothing runs here.
//!
//! Standards: SCF VPM-12.2 (Breadth / Depth of Coverage For
//! Vulnerability Scanning), CFG-11 (Software Usage Restrictions).

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::collect_catalogue::{Inputs, Kind, Network, REPORTS_DIR, Sink, Tool};
use crate::collect_detect::Ecosystem;
use crate::collect_lookup::find;
use crate::collect_managed::{TOOLS_DIR, eslint_script};

/// Where the repository is mounted (read-only) inside a container.
pub(crate) const CONTAINER_ROOT: &str = "/src";

/// Where the rule directories are mounted (read-only) inside a
/// container: the first at `/rules/0`, the second at `/rules/1`.
pub(crate) const CONTAINER_RULES: &str = "/rules";

/// The rule directories Opengrep and Semgrep get, as absolute paths. There
/// is always a first one, so neither tool can fall back to registry rules.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RuleDirs<'a> {
    /// The given `--rules` directory, or sloptopop's own pack.
    pub(crate) first: &'a Path,
    /// With the pack: `.sloptopop/rules/`, if it exists.
    pub(crate) second: Option<&'a Path>,
}

impl<'a> RuleDirs<'a> {
    /// The directories, in order.
    fn iter(self) -> impl Iterator<Item = &'a Path> {
        std::iter::once(self.first).chain(self.second)
    }
}

/// Everything the plan depends on.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Settings<'a> {
    /// The repository root, as an absolute path.
    pub(crate) root: &'a Path,
    /// The ecosystems the root uses.
    pub(crate) ecosystems: &'a BTreeSet<Ecosystem>,
    /// Only these entries (empty: all that apply).
    pub(crate) only: &'a [String],
    /// The rule directories.
    pub(crate) rules: RuleDirs<'a>,
    /// Whether the root is a git repository.
    pub(crate) git: bool,
    /// Run tools from pinned container images.
    pub(crate) container: bool,
    /// The value of `PATH`.
    pub(crate) path: Option<&'a OsStr>,
}

/// What happens to one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// Run `program` with the planned argv (after its first element).
    Run {
        /// The executable found on `PATH`.
        program: PathBuf,
        /// The arguments that print the tool's version.
        version: Vec<String>,
        /// The file the tool's stdout goes to, if it prints its report.
        stdout: Option<PathBuf>,
    },
    /// The executable is not installed.
    Missing(String),
    /// Not run, for this reason.
    Skipped(String),
}

/// One entry of the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Planned<'t> {
    /// The catalogue entry.
    pub(crate) tool: &'t Tool,
    /// The command line, program name first.
    pub(crate) argv: Vec<String>,
    /// What happens to it.
    pub(crate) step: Step,
}

/// Plans every entry of `catalogue` that applies and was selected, in
/// catalogue order.
pub(crate) fn plan<'t>(catalogue: &'t [Tool], settings: &Settings<'_>) -> Vec<Planned<'t>> {
    let rules: Vec<String> = settings
        .rules
        .iter()
        .enumerate()
        .map(|(index, dir)| {
            if settings.container {
                format!("{CONTAINER_RULES}/{index}")
            } else {
                dir.to_string_lossy().into_owned()
            }
        })
        .collect();
    let tools = settings.root.join(TOOLS_DIR);
    let tools_text = tools.to_string_lossy();
    let eslint = eslint_script(settings.root);
    let eslint_text = eslint.to_string_lossy();
    let inputs = Inputs {
        rules: &rules,
        tools: &tools_text,
        eslint: &eslint_text,
    };
    catalogue
        .iter()
        .filter(|tool| tool.applies(settings.ecosystems))
        .filter(|tool| {
            settings.only.is_empty() || settings.only.iter().any(|name| name == tool.name)
        })
        .map(|tool| plan_one(tool, settings, &inputs, &tools))
        .collect()
}

fn plan_one<'t>(
    tool: &'t Tool,
    settings: &Settings<'_>,
    inputs: &Inputs<'_>,
    tools: &Path,
) -> Planned<'t> {
    let argv = tool.argv(inputs);
    let skipped = |reason: &str| Planned {
        tool,
        argv: argv.clone(),
        step: Step::Skipped(reason.to_owned()),
    };
    match tool.kind {
        Kind::Documented => return skipped("documented only: run it after your tests"),
        Kind::NeedsGit if !settings.git => return skipped("not a git repository"),
        Kind::Managed(managed) if !settings.container && !managed.installed(tools) => {
            let reason = format!(
                "not installed in {TOOLS_DIR}/{}: run `{}`",
                managed.dir(),
                managed.install()
            );
            return Planned {
                tool,
                argv,
                step: Step::Missing(reason),
            };
        }
        Kind::Run | Kind::NeedsGit | Kind::Managed(_) => {}
    }
    let stdout = match tool.sink {
        Sink::Stdout => Some(settings.root.join(REPORTS_DIR).join(tool.output)),
        Sink::PathArgument | Sink::Config => None,
    };
    if !settings.container {
        let step = match find(settings.path, tool.command) {
            Some(program) => Step::Run {
                program,
                version: tool.version_args(inputs),
                stdout,
            },
            None => Step::Missing("not found on PATH".to_owned()),
        };
        return Planned { tool, argv, step };
    }
    let Some(image) = tool.image else {
        return skipped("no container image catalogued");
    };
    let Some(reference) = image.reference() else {
        return skipped("container image digest not pinned");
    };
    let argv = docker_run(tool, &reference, argv.into_iter().skip(1), settings);
    let step = match find(settings.path, "docker") {
        Some(program) => Step::Run {
            program,
            version: docker_version(tool, &reference),
            stdout,
        },
        None => Step::Missing("docker not found on PATH".to_owned()),
    };
    Planned { tool, argv, step }
}

/// The `docker run` command line that runs `tool` from `reference` with
/// `tool_args`: the root mounted read-only, the reports directory read-write,
/// the rule directories read-only for a tool that takes them, no network
/// unless the tool downloads a database, and no published ports.
fn docker_run(
    tool: &Tool,
    reference: &str,
    tool_args: impl Iterator<Item = String>,
    settings: &Settings<'_>,
) -> Vec<String> {
    let network = match tool.network {
        Network::DatabaseDownload => "bridge",
        Network::Offline => "none",
    };
    let mut argv: Vec<String> = ["docker", "run", "--rm", "--network", network]
        .map(str::to_owned)
        .into();
    argv.extend(user(settings.root));
    argv.extend(
        [
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--env",
            "HOME=/tmp",
        ]
        .map(str::to_owned),
    );
    let mut mount = |source: &Path, target: &str, readonly: bool| {
        let readonly = if readonly { ",readonly" } else { "" };
        argv.push("--mount".to_owned());
        argv.push(format!(
            "type=bind,source={},target={target}{readonly}",
            source.to_string_lossy()
        ));
    };
    mount(settings.root, CONTAINER_ROOT, true);
    mount(
        &settings.root.join(REPORTS_DIR),
        &format!("{CONTAINER_ROOT}/{REPORTS_DIR}"),
        false,
    );
    if tool.takes_rules() {
        for (index, dir) in settings.rules.iter().enumerate() {
            mount(dir, &format!("{CONTAINER_RULES}/{index}"), true);
        }
    }
    argv.extend(
        [
            "--workdir",
            CONTAINER_ROOT,
            "--entrypoint",
            tool.command,
            reference,
        ]
        .map(str::to_owned),
    );
    argv.extend(tool_args);
    argv
}

/// The arguments after `docker` that print the version of `tool` in
/// `reference`: no mounts, no network.
fn docker_version(tool: &Tool, reference: &str) -> Vec<String> {
    [
        "run",
        "--rm",
        "--network",
        "none",
        "--entrypoint",
        tool.command,
        reference,
        "--version",
    ]
    .map(str::to_owned)
    .into()
}

/// `--user <uid>:<gid>` of the root's owner, so reports written from a
/// container belong to the user rather than to root. Elsewhere than on
/// Unix the container engine decides.
fn user(root: &Path) -> Vec<String> {
    #[cfg(unix)]
    let owner = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(root)
            .ok()
            .map(|meta| format!("{}:{}", meta.uid(), meta.gid()))
    };
    #[cfg(not(unix))]
    let owner: Option<String> = {
        let _unused = root;
        None
    };
    owner
        .map(|owner| vec!["--user".to_owned(), owner])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
