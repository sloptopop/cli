//! The network calls of `sloptopop check`, `fix` and `transform`, all made
//! with the system's `curl` (no TLS stack of our own, and every command is
//! visible; decision D-0900): the request file is posted to the server's
//! `/v1/jobs`, which queues it (owner decision D-0904) and answers where
//! to poll; the job is polled every 2 seconds, backing off to 5, while
//! one updating line on stderr says how many projects are ahead; the
//! answer is written to the response file, and the job is then deleted on
//! the server. A small watcher process deletes the job too when
//! `sloptopop` is stopped (Ctrl-C) while it waits. Nothing else is ever
//! sent: no telemetry, no update check, a generic user agent (decision
//! D-0179).
//!
//! Standards: SCF NET-14 (encrypted transmission: https, or http inside the
//! customer's own network), PRI-05 (a cancelled run leaves nothing queued
//! on the server); OWASP ASVS 5.0 V12.1.1 (TLS for data in transit); GDPR
//! Art. 5(1)(c).

use std::fs;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::value::RawValue;

/// The user agent of every call: the tool and its version, nothing else.
pub(crate) const USER_AGENT: &str = concat!("sloptopop/", env!("CARGO_PKG_VERSION"));

/// The jobs route under a server's base URL.
pub(crate) fn endpoint(backend: &str) -> String {
    format!("{}/v1/jobs", backend.trim_end_matches('/'))
}

/// Runs `curl` with the options every call shares and `args`, writing the
/// answer to `output` (an earlier answer there is removed first).
///
/// # Errors
///
/// Why it failed: curl could not run, could not connect, or the server
/// answered with an error (whose body is quoted).
fn call(curl: &Path, url: &str, args: &[&str], output: &Path) -> Result<(), String> {
    let _stale = fs::remove_file(output);
    let ran = Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--fail-with-body",
            "--proto",
            "=https,http",
            "--user-agent",
            USER_AGENT,
        ])
        .args(args)
        .arg("--output")
        .arg(output)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run {}: {error}", curl.display()))?;
    if ran.status.success() {
        return Ok(());
    }
    let body = fs::read_to_string(output).unwrap_or_default();
    let answer = if body.trim().is_empty() {
        String::new()
    } else {
        format!(" ({})", body.trim())
    };
    Err(format!(
        "the backend at {url} did not answer: {}{answer}",
        String::from_utf8_lossy(&ran.stderr).trim()
    ))
}

/// The server's answer to a submitted job.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Submitted {
    /// Where to poll, under the server's base URL.
    status_url: String,
}

/// Posts `request` to the jobs route `url` (under the server's base URL
/// `backend`), writing the answer to `answer`: the job's status URL.
///
/// # Errors
///
/// As [`call`], or when the answer is not a job.
pub(crate) fn submit(
    curl: &Path,
    backend: &str,
    request: &Path,
    answer: &Path,
) -> Result<String, String> {
    let url = endpoint(backend);
    let data = format!("@{}", request.display());
    call(
        curl,
        &url,
        &[
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            &data,
        ],
        answer,
    )?;
    let text = fs::read_to_string(answer).unwrap_or_default();
    serde_json::from_str::<Submitted>(&text)
        .map(|job| format!("{}{}", backend.trim_end_matches('/'), job.status_url))
        .map_err(|error| format!("the backend at {url} answered no job: {error}"))
}

/// A job's state document.
#[derive(Deserialize)]
struct Polled<'a> {
    /// `queued`, `running`, `done` or `failed`.
    state: &'a str,
    /// Queued: the jobs ahead (queued before it plus running).
    #[serde(default)]
    position: u64,
    /// Queued: the jobs running now.
    #[serde(default)]
    running: u64,
    /// Done: the response document.
    #[serde(borrow, default)]
    response: Option<&'a RawValue>,
    /// Failed: the error.
    #[serde(default)]
    error: Option<Failure>,
}

/// A failed job's error.
#[derive(Deserialize)]
struct Failure {
    /// The error code.
    code: String,
    /// Why.
    message: String,
}

/// Where a job is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    /// Waiting behind `ahead` projects, `queued` of them queued.
    Queued {
        /// Every project ahead.
        ahead: u64,
        /// The queued ones among them.
        queued: u64,
    },
    /// In a slot.
    Running,
    /// Answered: the response is written.
    Done,
}

/// Polls `url` once, writing the state to `answer` and a done job's
/// response to `response`.
///
/// # Errors
///
/// As [`call`]; a failed job's error; an answer that is not a state.
fn poll(curl: &Path, url: &str, answer: &Path, response: &Path) -> Result<State, String> {
    call(curl, url, &[], answer)?;
    let text = fs::read_to_string(answer).unwrap_or_default();
    let polled: Polled<'_> = serde_json::from_str(&text)
        .map_err(|error| format!("the backend at {url} answered no job state: {error}"))?;
    match (polled.state, polled.response, polled.error) {
        ("queued", _, _) => Ok(State::Queued {
            ahead: polled.position,
            queued: polled.position.saturating_sub(polled.running),
        }),
        ("running", _, _) => Ok(State::Running),
        ("done", Some(document), _) => fs::write(response, document.get())
            .map(|()| State::Done)
            .map_err(|error| format!("cannot write {}: {error}", response.display())),
        ("failed", _, Some(failure)) => Err(format!(
            "the server refused the request: {}: {}",
            failure.code, failure.message
        )),
        (other, _, _) => Err(format!(
            "the backend at {url} answered an unknown job state `{other}`"
        )),
    }
}

/// The line that says where a job is.
fn line(state: &State) -> String {
    match state {
        State::Queued { queued: 0, .. } => "waiting: the server is busy, you are next".to_owned(),
        State::Queued { ahead: 1, .. } => "waiting: 1 project ahead of you".to_owned(),
        State::Queued { ahead, .. } => format!("waiting: {ahead} projects ahead of you"),
        State::Running | State::Done => "running…".to_owned(),
    }
}

/// The progress line on stderr: on a terminal one line rewritten in
/// place, otherwise a new line whenever it changes.
pub(crate) struct Progress<'a> {
    /// Where it goes.
    stderr: &'a mut dyn Write,
    /// Whether stderr is a terminal.
    terminal: bool,
    /// The line shown last.
    shown: Option<String>,
}

impl<'a> Progress<'a> {
    /// Progress on `stderr`, a terminal or not.
    pub(crate) fn new(stderr: &'a mut dyn Write, terminal: bool) -> Self {
        Self {
            stderr,
            terminal,
            shown: None,
        }
    }

    /// Shows `text` unless it is shown already.
    fn show(&mut self, text: String) {
        if self.shown.as_ref() == Some(&text) {
            return;
        }
        let _shown = if self.terminal {
            write!(self.stderr, "\r\x1b[K{text}")
        } else {
            writeln!(self.stderr, "{text}")
        }
        .and_then(|()| self.stderr.flush());
        self.shown = Some(text);
    }

    /// Ends the line on a terminal.
    fn end(&mut self) {
        if self.terminal && self.shown.is_some() {
            let _ended = writeln!(self.stderr);
        }
    }
}

/// Polls the job at `url` until it is done or failed, waiting `poll`
/// first and then half of it more each time up to two and a half times
/// it (2, 3, 4, 5, 5… seconds for 2), showing where it is on `progress`;
/// `files` are the answer and the response files.
///
/// # Errors
///
/// As [`poll`].
pub(crate) fn wait(
    curl: &Path,
    url: &str,
    files: (&Path, &Path),
    poll_every: Duration,
    progress: &mut Progress<'_>,
) -> Result<(), String> {
    let mut delay = poll_every;
    loop {
        let state = poll(curl, url, files.0, files.1).inspect_err(|_| progress.end())?;
        progress.show(line(&state));
        if state == State::Done {
            progress.end();
            return Ok(());
        }
        pause(delay);
        delay = next(delay, poll_every);
    }
}

/// Waits `delay` between two polls.
#[expect(
    clippy::disallowed_methods,
    reason = "the CLI is synchronous and has no async runtime: a poll of the server's queue waits on the thread between two curl calls"
)]
fn pause(delay: Duration) {
    thread::sleep(delay);
}

/// The wait after `delay`: half of `every` more, at most two and a half
/// times `every`.
fn next(delay: Duration, every: Duration) -> Duration {
    (delay + every / 2).min(every * 5 / 2)
}

/// The script of the watcher: it ignores Ctrl-C, waits until its stdin
/// closes (`sloptopop` finished or was stopped) and deletes the job.
const CANCEL: &str = r#"trap '' INT
while read -r _; do :; done
exec "$1" --silent --proto =https,http --user-agent "$4" --request DELETE --output "$3" "$2""#;

/// The watcher that deletes a job on the server once `sloptopop` is done
/// with it or stopped.
pub(crate) struct Cancel(Option<Child>);

impl Cancel {
    /// Starts the watcher for the job at `url`, deleting it with `curl`
    /// (the answer goes to `output`). In its own process group, so a
    /// Ctrl-C at the terminal does not reach it. Without `/bin/sh` there
    /// is none: the job then expires on the server.
    pub(crate) fn start(curl: &Path, url: &str, output: &Path) -> Self {
        Self(
            Command::new("/bin/sh")
                .arg("-c")
                .arg(CANCEL)
                .arg("sloptopop-cancel")
                .arg(curl)
                .arg(url)
                .arg(output)
                .arg(USER_AGENT)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0)
                .spawn()
                .ok(),
        )
    }

    /// Deletes the job now and waits until the server answered.
    pub(crate) fn now(self) {
        let _deleted = self.0.map(|mut child| {
            drop(child.stdin.take());
            child.wait()
        });
    }
}

#[cfg(test)]
mod tests;
