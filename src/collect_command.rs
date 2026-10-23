//! `sloptopop collect`: runs the analyzers that apply to the repository
//! from sloptopop's pinned container images, with fixed flags, and writes
//! their reports to `.sloptopop/reports/`, where `sloptopop check` (the
//! engine) reads them. Every image, and every vulnerability snapshot, is
//! announced before it is fetched, then verified by digest. Without a
//! container runtime nothing runs and the exit code is 3: there is never a
//! fallback to the tools on `PATH`, which only the explicit, unsupported
//! `--host-tools` uses.
//!
//! Standards: SCF VPM-12 (Vulnerability Scanning), TDA-17.1 (Static
//! Code Analysis), TDA-06.5 (Software Release Integrity Verification),
//! CFG-11 (Software Usage Restrictions: pinned versions only, unless the
//! user opts out explicitly).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf, absolute};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::args::{CollectOptions, Source, registry};
use crate::collect_catalogue::{
    CATALOGUE, CONTAINER_ROOT, CONTAINER_RULES, Image, Prepare, RAW_DIR, REPORTS_DIR, Tool,
};
use crate::collect_container::{Limits, Mount};
use crate::collect_data::{Context, Resolved, Snapshot, age, resolve, snapshots};
use crate::collect_detect::{Ecosystem, detect};
use crate::collect_gitignore::ensure_ignored;
use crate::collect_image::ensure;
use crate::collect_pack::{self, PACK_DIR};
use crate::collect_plan::{self as planning, Planned, RuleDirs, Settings, Step};
use crate::collect_record::{self, Mode, RECORD_FILE, image_line, record, summary};
use crate::collect_run::{Job, Outcome, Shared, Status, Used, run_host, run_image, run_job};
use crate::collect_runtime::{Engine, Runtime, detect as detect_runtime};
use crate::diagnostic::report;
use crate::engine_locate::Locator;

/// The local rules directory used next to the pack, relative to the root.
pub(crate) const RULES_DIR: &str = ".sloptopop/rules";

/// The component library `collect` pulls from unless `--registry` or
/// `SLOPTOPOP_REGISTRY` names another (decision O2: the hostname is the
/// owner's to change).
pub(crate) const DEFAULT_REGISTRY: &str = "components.sloptopop.dev";

/// Exit code when the reports cannot be written or stdout is broken.
pub(crate) const WRITE_ERROR: u8 = 1;

/// Exit code when no container runtime answers.
pub(crate) const NO_RUNTIME: u8 = 3;

/// The time limit of each tool, unless `--timeout` sets another.
const TIMEOUT: Duration = Duration::from_mins(30);

/// The memory limit of each container, unless `SLOPTOPOP_COLLECT_MEMORY`
/// sets another.
const MEMORY: &str = "4g";

/// The age, in days, from which a vulnerability snapshot gets a warning.
const STALE_DAYS: i64 = 7;

/// The shared Go module cache volume.
const GO_VOLUME: &str = "sloptopop-gomodcache";

/// Where it is mounted.
const GO_TARGET: &str = "/gomodcache";

/// Where the .NET work volume is mounted.
const DOTNET_TARGET: &str = "/work";

/// The Go settings the module download takes from the user's environment.
const GO_VARS: [&str; 5] = [
    "GOPRIVATE",
    "GONOPROXY",
    "GONOSUMDB",
    "GONOSUMCHECK",
    "GOINSECURE",
];

/// Runs `collect` in the environment `env`. Returns the exit code: 0, 3
/// without a container runtime, or [`WRITE_ERROR`] if nothing could be
/// recorded. A tool that fails never changes it. `Err` holds a usage
/// message for invalid arguments.
pub(crate) fn collect(
    options: &CollectOptions,
    env: &Locator,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<u8, String> {
    if let Some(unknown) = options
        .only
        .iter()
        .find(|name| !CATALOGUE.iter().any(|tool| tool.name == *name))
    {
        return Err(format!(
            "unknown tool: {unknown} (docs/collect.md lists them)"
        ));
    }
    let settings = Configured::new(options, env)?;
    let root =
        directory(&options.root).ok_or_else(|| format!("not a directory: {}", options.root))?;
    let given = options
        .rules
        .as_deref()
        .map(|dir| directory(dir).ok_or_else(|| format!("rules directory not found: {dir}")))
        .transpose()?;
    // Notes on stderr are best effort: the run and its record go on.
    if options.source == Source::ContainerFlag {
        let _noted = writeln!(
            stderr,
            "note: containers are the default now; --container has no effect"
        );
    }
    if options.source == Source::HostTools {
        let _warned = writeln!(
            stderr,
            "warning: --host-tools runs unpinned tool versions from PATH; results are not reproducible and not supported"
        );
    }
    let pack = root.join(PACK_DIR);
    let local = Some(root.join(RULES_DIR)).filter(|dir| dir.is_dir());
    let rules = match &given {
        Some(dir) => RuleDirs {
            first: dir,
            second: None,
        },
        None => RuleDirs {
            first: &pack,
            second: local.as_deref(),
        },
    };
    let ecosystems = detect(&root).unwrap_or_default();
    let run = Run {
        root: &root,
        rules,
        ecosystems: &ecosystems,
        settings: &settings,
        plan: Settings {
            root: &root,
            ecosystems: &ecosystems,
            only: &options.only,
            rules,
            git: root.join(".git").exists(),
        },
        pack: given.is_none().then_some(pack.as_path()),
        plan_only: options.dry_run,
    };
    Ok(if options.source == Source::HostTools {
        run.host_tools(env, stdout, stderr)
    } else {
        run.pinned(env, stdout, stderr)
    })
}

/// Creates the reports directory (and its raw-output directory), makes
/// sure git ignores it, and writes the rule pack unless `--rules` replaces
/// it. The error is the exit code after reporting what failed.
fn prepare_reports(root: &Path, pack: Option<&Path>, stderr: &mut dyn Write) -> Result<(), u8> {
    let reports = root.join(REPORTS_DIR);
    if let Err(e) =
        fs::create_dir_all(root.join(RAW_DIR)).and_then(|()| ensure_ignored(root, "reports"))
    {
        return Err(failure(stderr, &reports, &e));
    }
    if let Some(pack) = pack
        && let Err(e) = collect_pack::write(pack)
    {
        return Err(failure(stderr, pack, &e));
    }
    Ok(())
}

/// Writes the record, prints the summary, and returns `code` unless one
/// of them failed.
fn finish(
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    root: &Path,
    record: &str,
    summary: &str,
    code: u8,
) -> u8 {
    let record_path = root.join(REPORTS_DIR).join(RECORD_FILE);
    if let Err(e) = fs::write(&record_path, record) {
        return failure(stderr, &record_path, &e);
    }
    match print(stdout, stderr, summary) {
        0 => code,
        failed => failed,
    }
}

/// An entry when no runtime answered: what would have run is missing.
fn no_runtime(planned: Planned<'_, Image>) -> Outcome<'_> {
    let status = match &planned.step {
        Step::Run(_) => Status::Missing("no container runtime".to_owned()),
        Step::NotRun(not_run) => Status::from(not_run.clone()),
    };
    Outcome::without_run(planned, None, status)
}

/// What to say when no runtime answered: what was tried, and what to do.
fn no_runtime_message(tried: &str) -> String {
    format!(
        "error: no container runtime found; sloptopop runs every analyzer in a pinned container image.\n  \
         Tried: {tried}.\n  \
         Install one of: Docker Engine (Linux), Podman, Colima (macOS), Rancher Desktop; Docker Desktop also works if your organisation is licensed for it.\n  \
         Then run `sloptopop collect` again. To use your own, unpinned tools instead: `sloptopop collect --host-tools` (unsupported)."
    )
}

/// The settings that come from the options and the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Configured {
    registry: String,
    engine: Option<Engine>,
    limits: Limits,
    timeout: Duration,
    offline: bool,
    vuln_db: Option<String>,
    cache: Option<PathBuf>,
    go_proxy: String,
    go_vars: Vec<(String, String)>,
}

impl Configured {
    fn new(options: &CollectOptions, env: &Locator) -> Result<Self, String> {
        let text = |name: &str| {
            env.var(name)
                .map(|value| value.to_string_lossy().into_owned())
        };
        let registry = match (&options.registry, text("SLOPTOPOP_REGISTRY")) {
            (Some(given), _) => given.clone(),
            (None, Some(from_env)) => {
                registry(&from_env).map_err(|e| format!("SLOPTOPOP_REGISTRY: {e}"))?
            }
            (None, None) => DEFAULT_REGISTRY.to_owned(),
        };
        let engine = match options
            .runtime
            .clone()
            .or_else(|| text("SLOPTOPOP_CONTAINER_RUNTIME"))
        {
            None => None,
            Some(name) => Some(Engine::parse(&name).ok_or_else(|| {
                format!("unknown container runtime: {name} (expected docker or podman)")
            })?),
        };
        let memory = text("SLOPTOPOP_COLLECT_MEMORY").unwrap_or_else(|| MEMORY.to_owned());
        if !quantity(&memory, true) {
            return Err(format!(
                "SLOPTOPOP_COLLECT_MEMORY: not a size: {memory} (e.g. 4g)"
            ));
        }
        let cpus = text("SLOPTOPOP_COLLECT_CPUS");
        if let Some(cpus) = &cpus
            && !quantity(cpus, false)
        {
            return Err(format!(
                "SLOPTOPOP_COLLECT_CPUS: not a number: {cpus} (e.g. 2 or 1.5)"
            ));
        }
        let cache = env
            .var("SLOPTOPOP_CACHE_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                env.var("XDG_CACHE_HOME")
                    .map(|dir| Path::new(dir).join("sloptopop"))
            })
            .or_else(|| {
                env.var("HOME")
                    .map(|dir| Path::new(dir).join(".cache").join("sloptopop"))
            })
            .or_else(|| {
                env.var("LOCALAPPDATA")
                    .map(|dir| Path::new(dir).join("sloptopop").join("cache"))
            });
        Ok(Self {
            registry,
            engine,
            limits: Limits { memory, cpus },
            timeout: options.timeout.map_or(TIMEOUT, Duration::from_secs),
            offline: options.offline,
            vuln_db: options.vuln_db.clone(),
            cache,
            go_proxy: text("GOPROXY")
                .unwrap_or_else(|| "https://proxy.golang.org,direct".to_owned()),
            go_vars: GO_VARS
                .iter()
                .filter_map(|name| text(name).map(|value| ((*name).to_owned(), value)))
                .collect(),
        })
    }
}

/// Whether `value` is a positive number (with `unit`, a whole number
/// optionally followed by `b`, `k`, `m` or `g`).
fn quantity(value: &str, unit: bool) -> bool {
    let number = if unit {
        value.strip_suffix(['b', 'k', 'm', 'g']).unwrap_or(value)
    } else {
        value
    };
    let valid = if unit {
        !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    } else {
        number.parse::<f64>().is_ok_and(f64::is_finite)
    };
    valid && number.bytes().any(|b| matches!(b, b'1'..=b'9'))
}

/// An entry on its way through the fetch phase.
#[derive(Debug)]
enum Stage<'t> {
    /// It does not run.
    Done(Outcome<'t>),
    /// It runs once what it needs is fetched.
    Ready(Box<Ready<'t>>),
}

impl<'t> Stage<'t> {
    fn ready(&mut self) -> Option<&mut Ready<'t>> {
        match self {
            Self::Ready(ready) => Some(ready),
            Self::Done(_) => None,
        }
    }
}

/// What an entry that runs needs: its image (the reference to run, or
/// why it cannot be used), its vulnerability snapshots, and its prepare
/// step's result.
#[derive(Debug)]
struct Ready<'t> {
    entry: Planned<'t, Image>,
    image: Image,
    run: Result<String, String>,
    data: Vec<(Snapshot, Result<Resolved, String>)>,
    prepared: Option<Result<(), String>>,
}

/// One collect run's fixed inputs.
#[derive(Debug, Clone, Copy)]
struct Run<'a> {
    root: &'a Path,
    rules: RuleDirs<'a>,
    ecosystems: &'a BTreeSet<Ecosystem>,
    settings: &'a Configured,
    plan: Settings<'a>,
    /// Where to write the rule pack, unless `--rules` replaces it.
    pack: Option<&'a Path>,
    /// `--dry-run`: print the plan, fetch and run nothing.
    plan_only: bool,
}

impl Run<'_> {
    /// `--host-tools`: plans, then runs the tools found on `PATH`.
    fn host_tools(&self, env: &Locator, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
        let planned = planning::host(CATALOGUE, &self.plan, env.path.as_deref());
        if self.plan_only {
            let text =
                collect_record::plan(self.ecosystems, &planned, |_, target| target.to_owned());
            return print(stdout, stderr, &text);
        }
        if let Err(code) = prepare_reports(self.root, self.pack, stderr) {
            return code;
        }
        let outcomes: Vec<_> = planned
            .into_iter()
            .map(|entry| run_host(entry, self.root))
            .collect();
        let text = record(Mode::Host, self.ecosystems, &[], &outcomes);
        finish(
            stdout,
            stderr,
            self.root,
            &text,
            &summary(self.ecosystems, &outcomes),
            0,
        )
    }

    /// The default: plans, finds the runtime, then fetches, verifies and
    /// runs the images.
    fn pinned(&self, env: &Locator, stdout: &mut dyn Write, stderr: &mut dyn Write) -> u8 {
        let settings = self.settings;
        let planned = planning::images(CATALOGUE, &self.plan);
        if self.plan_only {
            let arch = std::env::consts::ARCH;
            let text = collect_record::plan(self.ecosystems, &planned, |image, target| {
                image_line(image, &settings.registry, arch, target)
            });
            return print(stdout, stderr, &text);
        }
        if let Err(code) = prepare_reports(self.root, self.pack, stderr) {
            return code;
        }
        let (runtime, databases, outcomes, code) =
            match detect_runtime(env.path.as_deref(), settings.engine, self.root) {
                Err(tried) => {
                    let _reported = writeln!(stderr, "{}", no_runtime_message(&tried));
                    let outcomes: Vec<_> = planned.into_iter().map(no_runtime).collect();
                    (None, Vec::new(), outcomes, NO_RUNTIME)
                }
                Ok(runtime) => {
                    let (databases, outcomes) = self.images(&runtime, planned, stdout, stderr);
                    (Some(runtime), databases, outcomes, 0)
                }
            };
        let text = record(
            Mode::Images {
                runtime: runtime.as_ref(),
                registry: &settings.registry,
            },
            self.ecosystems,
            &databases,
            &outcomes,
        );
        finish(
            stdout,
            stderr,
            self.root,
            &text,
            &summary(self.ecosystems, &outcomes),
            code,
        )
    }

    /// Fetches and verifies what the planned entries need, runs their
    /// prepare steps, then runs them. Returns the snapshots used and the
    /// outcomes.
    fn images<'t>(
        &self,
        runtime: &Runtime,
        planned: Vec<Planned<'t, Image>>,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> (Vec<Resolved>, Vec<Outcome<'t>>) {
        let owner = owner(self.root);
        let shared = Shared {
            runtime,
            limits: &self.settings.limits,
            owner: owner.as_deref(),
            root: self.root,
            timeout: self.settings.timeout,
        };
        // Names unique per repository, and the same on every run, so the
        // record stays deterministic (two runs on one repository at once
        // would clash on the reports anyway).
        let nonce = root_key(self.root);
        // Every image once, in plan order, each announced before its fetch.
        let mut images = BTreeMap::new();
        let mut stages: Vec<Stage<'t>> = planned
            .into_iter()
            .map(|entry| match entry.step.clone() {
                Step::NotRun(not_run) => {
                    Stage::Done(Outcome::without_run(entry, None, Status::from(not_run)))
                }
                Step::Run(image) => {
                    let run = images
                        .entry(image.digest)
                        .or_insert_with(|| {
                            ensure(
                                runtime,
                                &self.settings.registry,
                                &image,
                                entry.tool.licence,
                                self.settings.offline,
                                self.root,
                                stdout,
                            )
                        })
                        .clone();
                    Stage::Ready(Box::new(Ready {
                        entry,
                        image,
                        run,
                        data: Vec::new(),
                        prepared: None,
                    }))
                }
            })
            .collect();
        // Every vulnerability snapshot once.
        let mut data: BTreeMap<&'static str, Result<Resolved, String>> = BTreeMap::new();
        for ready in stages.iter_mut().filter_map(Stage::ready) {
            let Some(need) = ready.entry.tool.data else {
                continue;
            };
            for snapshot in snapshots(need, self.ecosystems) {
                let resolved = data
                    .entry(snapshot.name)
                    .or_insert_with(|| self.snapshot(runtime, snapshot, &nonce, stdout, stderr))
                    .clone();
                ready.data.push((snapshot, resolved));
            }
        }
        // Every prepare step once, in the first image that needs it (an
        // entry whose image failed prepares nothing).
        let mut prepared = BTreeMap::new();
        for ready in stages.iter_mut().filter_map(Stage::ready) {
            if let (Some(step), Ok(run)) = (ready.entry.tool.prepare, &ready.run) {
                ready.prepared = Some(
                    prepared
                        .entry(step)
                        .or_insert_with(|| self.prepare(&shared, step, run, &nonce))
                        .clone(),
                );
            }
        }
        let outcomes = stages
            .into_iter()
            .enumerate()
            .map(|(index, stage)| match stage {
                Stage::Done(outcome) => outcome,
                Stage::Ready(ready) => self.outcome(index, *ready, &shared, &nonce),
            })
            .collect();
        let used: Vec<Resolved> = data.into_values().filter_map(Result::ok).collect();
        (used, outcomes)
    }

    /// Fetches (or finds) one vulnerability snapshot and warns when it is
    /// old.
    fn snapshot(
        &self,
        runtime: &Runtime,
        snapshot: Snapshot,
        nonce: &str,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> Result<Resolved, String> {
        let settings = self.settings;
        let cache = settings
            .cache
            .as_ref()
            .ok_or_else(|| "no cache directory: set SLOPTOPOP_CACHE_DIR".to_owned())?;
        let context = Context {
            runtime,
            registry: &settings.registry,
            cache,
            offline: settings.offline,
            date: settings.vuln_db.as_deref(),
            cwd: self.root,
            nonce: &format!("{nonce}-{}", snapshot.name),
        };
        let resolved = resolve(&context, snapshot, stdout)?;
        stale(&resolved, today(), stderr);
        Ok(resolved)
    }

    /// Runs one entry whose image, data and prepare step were fetched.
    fn outcome<'t>(
        &self,
        index: usize,
        ready: Ready<'t>,
        shared: &Shared<'_>,
        nonce: &str,
    ) -> Outcome<'t> {
        let Ready {
            entry,
            image,
            run,
            data,
            prepared,
        } = ready;
        let used = Used {
            reference: image.reference(&self.settings.registry),
            version: image.version,
        };
        let mounts = run.and_then(|reference| {
            let mut mounts = self.mounts(entry.tool);
            for (snapshot, resolved) in data {
                let resolved = resolved?;
                mounts.push(Mount::bind(
                    &resolved.dir,
                    &format!("/db/{}", snapshot.path),
                    true,
                ));
            }
            if let Some(step) = entry.tool.prepare {
                prepared.unwrap_or(Ok(()))?;
                mounts.push(self.volume(step, step == Prepare::DotnetRestore));
            }
            Ok((reference, mounts))
        });
        match mounts {
            Err(reason) => Outcome::without_run(entry, Some(used), Status::failure(reason)),
            Ok((reference, mounts)) => {
                let job = Job {
                    name: format!("sloptopop-collect-{nonce}-{index}"),
                    network: false,
                    env: entry
                        .tool
                        .env
                        .iter()
                        .map(|&(n, v)| (n.to_owned(), v.to_owned()))
                        .chain(entry.tool.root_variable.map(|name| {
                            (name.to_owned(), self.root.to_string_lossy().into_owned())
                        }))
                        .collect(),
                    mounts,
                    entrypoint: entry.tool.command,
                    image: &reference,
                    args: entry.argv.iter().skip(1).cloned().collect(),
                };
                run_image(&entry, &image, used, &job, shared)
            }
        }
    }

    /// The repository, reports and (for tools that take them) rule mounts.
    fn mounts(&self, tool: &Tool) -> Vec<Mount> {
        let mut mounts = vec![
            Mount::bind(self.root, CONTAINER_ROOT, true),
            Mount::bind(
                &self.root.join(REPORTS_DIR),
                &format!("{CONTAINER_ROOT}/{REPORTS_DIR}"),
                false,
            ),
        ];
        if tool.takes_rules() {
            for (index, dir) in self.rules.iter().enumerate() {
                mounts.push(Mount::bind(
                    dir,
                    &format!("{CONTAINER_RULES}/{index}"),
                    true,
                ));
            }
        }
        mounts
    }

    /// The volume a prepare step fills, writable or read-only.
    fn volume(&self, step: Prepare, writable: bool) -> Mount {
        match step {
            Prepare::GoModules => Mount::Volume {
                name: GO_VOLUME.to_owned(),
                target: GO_TARGET.to_owned(),
                readonly: !writable,
            },
            Prepare::DotnetRestore => Mount::Volume {
                name: format!("sloptopop-dotnet-{}", root_key(self.root)),
                target: DOTNET_TARGET.to_owned(),
                readonly: !writable,
            },
        }
    }

    /// Runs prepare step `step` in the image `run`.
    fn prepare(
        &self,
        shared: &Shared<'_>,
        step: Prepare,
        run: &str,
        nonce: &str,
    ) -> Result<(), String> {
        let (entrypoint, args, env, what): (&str, Vec<&str>, Vec<(String, String)>, &str) =
            match step {
                Prepare::GoModules => {
                    if !self.root.join("go.mod").is_file()
                        || self.root.join("vendor/modules.txt").is_file()
                    {
                        return Ok(());
                    }
                    let mut env = vec![
                        ("GOPROXY".to_owned(), self.settings.go_proxy.clone()),
                        ("GOFLAGS".to_owned(), "-mod=mod".to_owned()),
                    ];
                    env.extend(self.settings.go_vars.iter().cloned());
                    ("go", vec!["mod", "download"], env, "go mod download")
                }
                Prepare::DotnetRestore => (
                    "dotnet",
                    vec![
                        "restore",
                        "-p:DirectoryBuildPropsPath=/opt/sonaranalyzer-csharp/Directory.Build.props",
                    ],
                    Vec::new(),
                    "dotnet restore",
                ),
            };
        let job = Job {
            name: format!(
                "sloptopop-prepare-{nonce}-{}",
                what.split(' ').next().unwrap_or(what)
            ),
            network: true,
            env,
            mounts: vec![
                Mount::bind(self.root, CONTAINER_ROOT, true),
                self.volume(step, true),
            ],
            entrypoint,
            image: run,
            args: args.into_iter().map(str::to_owned).collect(),
        };
        let status = run_job(shared, &job, None, &[]);
        match status.reason() {
            None => Ok(()),
            Some(reason) => Err(format!("prepare step ({what}) failed: {reason}")),
        }
    }
}

/// Today, in days since 1970-01-01 (UTC).
fn today() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs() / 86_400).unwrap_or(0)
        })
}

/// Warns on `stderr` when a snapshot is older than [`STALE_DAYS`] on day
/// `today`.
fn stale(resolved: &Resolved, today: i64, stderr: &mut dyn Write) {
    if let Some(days) = age(&resolved.version, today)
        && days > STALE_DAYS
    {
        let _warned = writeln!(
            stderr,
            "warning: the {} snapshot is {days} days old ({}); the component library has not been updated",
            resolved.snapshot.name, resolved.version
        );
    }
}

/// A short, stable key of the root's path, for its volume's name.
fn root_key(root: &Path) -> String {
    blake3::hash(root.to_string_lossy().as_bytes())
        .to_hex()
        .chars()
        .take(16)
        .collect()
}

/// `uid:gid` of the root's owner, so reports written from a container
/// belong to the user. Elsewhere than on Unix the runtime decides.
fn owner(root: &Path) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(root)
            .ok()
            .map(|meta| format!("{}:{}", meta.uid(), meta.gid()))
    }
    #[cfg(not(unix))]
    {
        let _unused = root;
        None
    }
}

/// `path` as an absolute path, if it names a directory.
fn directory(path: &str) -> Option<PathBuf> {
    absolute(path).ok().filter(|dir| dir.is_dir())
}

fn failure(stderr: &mut dyn Write, path: &Path, error: &std::io::Error) -> u8 {
    report(
        stderr,
        &format!("error: cannot write {}: {error}\n", path.display()),
        WRITE_ERROR,
    )
}

fn print(stdout: &mut dyn Write, stderr: &mut dyn Write, text: &str) -> u8 {
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => 0,
        Err(e) => report(
            stderr,
            &format!("error: cannot write output: {e}\n"),
            WRITE_ERROR,
        ),
    }
}

#[cfg(test)]
mod tests;
