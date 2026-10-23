//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only. The fake curl answers by host and, for
//! polls, from a script next to its output file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{Cancel, Progress, USER_AGENT, endpoint, next, pause, submit, wait};
use crate::tests::support::TempDir;

const CURL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fake-curl/curl");

fn curl() -> &'static Path {
    Path::new(CURL)
}

#[test]
fn the_endpoint_is_under_the_base_url() {
    assert_eq!(
        endpoint("https://b.example/"),
        "https://b.example/v1/jobs",
        "slash"
    );
    assert_eq!(
        endpoint("http://192.0.2.10:8080"),
        "http://192.0.2.10:8080/v1/jobs",
        "none"
    );
    assert_eq!(
        USER_AGENT,
        concat!("sloptopop/", env!("CARGO_PKG_VERSION")),
        "generic"
    );
}

#[test]
fn a_submit_answers_the_status_url_or_says_why_not() {
    let dir = TempDir::new();
    let request = dir.0.join("request.json");
    let answer = dir.0.join("job.json");
    fs::write(&request, "{}").unwrap();
    assert_eq!(
        submit(curl(), "http://ok/", &request, &answer).unwrap(),
        "http://ok/v1/jobs/j1",
        "the status URL under the base"
    );
    assert_eq!(
        fs::read_to_string(dir.0.join("request.json.posted")).unwrap(),
        "{}",
        "posted"
    );
    assert_eq!(
        fs::read_to_string(dir.0.join("fake-curl.log")).unwrap(),
        "POST http://ok/v1/jobs\n",
        "to the jobs route"
    );
    let refused = submit(curl(), "http://refuses", &request, &answer).unwrap_err();
    assert!(
        refused.contains("(22)") && refused.contains("the queue is full"),
        "{refused}"
    );
    fs::remove_file(&answer).unwrap();
    let down = submit(curl(), "http://down", &request, &answer).unwrap_err();
    assert!(down.ends_with("curl: (7) Failed to connect"), "{down}");
    let odd = submit(curl(), "http://odd", &request, &answer).unwrap_err();
    assert!(
        odd.starts_with("the backend at http://odd/v1/jobs answered no job:"),
        "{odd}"
    );
    let missing = submit(
        Path::new("/nonexistent/curl"),
        "http://ok",
        &request,
        &answer,
    )
    .unwrap_err();
    assert!(
        missing.starts_with("cannot run /nonexistent/curl"),
        "{missing}"
    );
}

/// Waits for the job at `url` with the fake curl answering `script`
/// (none: done at once); the result, stderr, the response and the dir.
fn waited(
    url: &str,
    script: Option<&str>,
    terminal: bool,
) -> (Result<(), String>, String, Option<String>, TempDir) {
    let dir = TempDir::new();
    if let Some(script) = script {
        fs::write(dir.0.join("fake-curl.script"), script).unwrap();
    }
    let answer = dir.0.join("job.json");
    let response = dir.0.join("response.json");
    let mut stderr = Vec::new();
    let result = wait(
        curl(),
        url,
        (&answer, &response),
        Duration::ZERO,
        &mut Progress::new(&mut stderr, terminal),
    );
    let written = fs::read_to_string(&response).ok();
    (result, String::from_utf8(stderr).unwrap(), written, dir)
}

const QUEUE: &str = r#"{"state":"queued","position":3,"running":1,"freeSlots":0}
{"state":"queued","position":3,"running":1,"freeSlots":0}
{"state":"queued","position":2,"running":1,"freeSlots":0}
{"state":"queued","position":1,"running":1,"freeSlots":0}
{"state":"running","elapsedSeconds":0}
{"state":"running","elapsedSeconds":2}
{"state":"done","response":{ "x" : [1, 2] }}
"#;

#[test]
fn a_queued_job_shows_its_place_until_it_is_done() {
    let (result, stderr, response, dir) = waited("http://ok/v1/jobs/j1", Some(QUEUE), false);
    assert_eq!(result, Ok(()), "done");
    assert_eq!(
        stderr,
        "waiting: 3 projects ahead of you\nwaiting: 2 projects ahead of you\nwaiting: the server is busy, you are next\nrunning…\n",
        "a new line only when it changes"
    );
    assert_eq!(
        response.as_deref(),
        Some(r#"{ "x" : [1, 2] }"#),
        "the response as the server sent it"
    );
    let log = fs::read_to_string(dir.0.join("fake-curl.log")).unwrap();
    assert_eq!(log.lines().count(), 7, "{log}");
    assert!(
        log.lines().all(|line| line == "GET http://ok/v1/jobs/j1"),
        "{log}"
    );
}

#[test]
fn on_a_terminal_the_line_is_rewritten_in_place() {
    let (result, stderr, _, _dir) = waited("http://ok/v1/jobs/j1", Some(QUEUE), true);
    assert_eq!(result, Ok(()), "done");
    assert_eq!(
        stderr,
        "\r\x1b[Kwaiting: 3 projects ahead of you\r\x1b[Kwaiting: 2 projects ahead of you\r\x1b[Kwaiting: the server is busy, you are next\r\x1b[Krunning…\n",
        "one line"
    );
    let (done, shown, response, _at_once) = waited("http://ok/v1/jobs/j1", None, true);
    assert_eq!(done, Ok(()), "done at once");
    assert_eq!(shown, "\r\x1b[Krunning…\n", "only the end");
    assert_eq!(response.as_deref(), Some(r#"{"answered":true}"#), "written");
}

#[test]
fn one_queued_project_ahead_is_singular() {
    let script = "{\"state\":\"queued\",\"position\":1,\"running\":0}\n{\"state\":\"done\",\"response\":{}}\n";
    let (result, stderr, _, _dir) = waited("http://ok/v1/jobs/j1", Some(script), false);
    assert_eq!(result, Ok(()), "done");
    assert_eq!(
        stderr, "waiting: 1 project ahead of you\nrunning…\n",
        "{stderr}"
    );
}

#[test]
fn a_failed_or_lost_job_says_why() {
    let failed = "{\"state\":\"running\"}\n{\"state\":\"failed\",\"error\":{\"code\":\"timeout\",\"message\":\"the transform took over 3600 seconds\"}}\n";
    let (result, stderr, response, _dir) = waited("http://ok/v1/jobs/j1", Some(failed), true);
    assert_eq!(
        result,
        Err(
            "the server refused the request: timeout: the transform took over 3600 seconds"
                .to_owned()
        ),
        "the typed error"
    );
    assert_eq!(stderr, "\r\x1b[Krunning…\n", "the line is ended");
    assert_eq!(response, None, "no response");
    for (script, wanted) in [
        (
            "!{\"error\":{\"code\":\"not-found\",\"message\":\"no such job\"}}\n",
            "curl: (22) The requested URL returned error: 404 ({\"error\":{\"code\":\"not-found\",\"message\":\"no such job\"}})",
        ),
        ("x\n", "curl: (7) Failed to connect"),
        (
            "{\"state\":\"lost\"}\n",
            "answered an unknown job state `lost`",
        ),
        (
            "{\"state\":\"done\"}\n",
            "answered an unknown job state `done`",
        ),
        (
            "{\"state\":\"failed\"}\n",
            "answered an unknown job state `failed`",
        ),
        ("[]\n", "answered no job state:"),
    ] {
        let (lost, shown, _, _at) = waited("http://ok/v1/jobs/j1", Some(script), true);
        let why = lost.unwrap_err();
        assert!(why.contains(wanted), "{why}");
        assert_eq!(shown, "", "nothing was shown: {script}");
    }
    let (odd, _, _, _odd_dir) = waited("http://odd/v1/jobs/j1", None, false);
    assert!(odd.unwrap_err().contains("answered no job state"), "{{}}");
}

#[test]
fn a_response_that_cannot_be_written_is_an_error() {
    let dir = TempDir::new();
    let mut stderr = Vec::new();
    let result = wait(
        curl(),
        "http://ok/v1/jobs/j1",
        (
            &dir.0.join("job.json"),
            &dir.0.join("missing/response.json"),
        ),
        Duration::ZERO,
        &mut Progress::new(&mut stderr, false),
    );
    assert!(
        result.unwrap_err().starts_with("cannot write "),
        "unwritable"
    );
}

#[test]
fn polls_back_off_from_every_to_two_and_a_half_times_it() {
    let every = Duration::from_secs(2);
    let mut delay = every;
    let mut seen = vec![delay];
    for _ in 0..4 {
        delay = next(delay, every);
        seen.push(delay);
    }
    let secs: Vec<u64> = seen.iter().map(Duration::as_secs).collect();
    assert_eq!(secs, [2, 3, 4, 5, 5], "2 s backing off to 5 s");
    assert_eq!(
        next(Duration::ZERO, Duration::ZERO),
        Duration::ZERO,
        "tests"
    );
}

#[test]
fn a_pause_waits_its_delay() {
    let start = Instant::now();
    pause(Duration::from_millis(40));
    assert!(start.elapsed() >= Duration::from_millis(40), "waited");
}

#[test]
fn the_watcher_deletes_the_job() {
    let dir = TempDir::new();
    let output: PathBuf = dir.0.join("job.json");
    Cancel::start(curl(), "http://ok/v1/jobs/j1", &output).now();
    assert_eq!(
        fs::read_to_string(dir.0.join("fake-curl.log")).unwrap(),
        "DELETE http://ok/v1/jobs/j1\n",
        "deleted once sloptopop is done"
    );
}
