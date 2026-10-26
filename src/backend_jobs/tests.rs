//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only. The fake curl answers by host and, for
//! polls, from a script next to its output file.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::{
    Cancel, LONGEST, Progress, RETRIES, USER_AGENT, answered, backoff, endpoint, explain,
    first_number, next, pause, submit, wait,
};
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
    assert_eq!(
        refused, "the server is busy: 200 jobs queued, try again in 30 s",
        "a plain sentence"
    );
    let limited = submit(curl(), "http://limited", &request, &answer).unwrap_err();
    assert_eq!(
        limited,
        "too many checks from this address at once: try again in 30 s, or once one of them has ended",
        "per address"
    );
    let edge = submit(curl(), "http://edge", &request, &answer).unwrap_err();
    assert_eq!(
        edge, "too many requests from this address: try again in 12 s",
        "the edge's rate limit"
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

/// What a wait left: its result, stderr, the response, the pauses it
/// made and its directory.
struct Waited {
    result: Result<(), String>,
    stderr: String,
    response: Option<String>,
    pauses: Vec<Duration>,
    dir: TempDir,
}

/// Waits for the job at `url`, polled every `every`, with the fake curl
/// answering `script` (none: done at once); the pauses are recorded, not
/// slept.
fn waited_every(url: &str, script: Option<&str>, terminal: bool, every: Duration) -> Waited {
    let dir = TempDir::new();
    if let Some(script) = script {
        fs::write(dir.0.join("fake-curl.script"), script).unwrap();
    }
    let answer = dir.0.join("job.json");
    let response = dir.0.join("response.json");
    let mut stderr = Vec::new();
    let mut pauses = Vec::new();
    let result = wait(
        curl(),
        url,
        (&answer, &response),
        every,
        &mut Progress::new(&mut stderr, terminal),
        &mut |delay| pauses.push(delay),
    );
    Waited {
        result,
        stderr: String::from_utf8(stderr).unwrap(),
        response: fs::read_to_string(&response).ok(),
        pauses,
        dir,
    }
}

/// As [`waited_every`], polled without a pause; the result, stderr, the
/// response and the dir.
fn waited(
    url: &str,
    script: Option<&str>,
    terminal: bool,
) -> (Result<(), String>, String, Option<String>, TempDir) {
    let done = waited_every(url, script, terminal, Duration::ZERO);
    (done.result, done.stderr, done.response, done.dir)
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
        &mut pause,
    );
    assert!(
        result.unwrap_err().starts_with("cannot write "),
        "unwritable"
    );
}

#[test]
fn polls_wait_their_backing_off_delay() {
    let done = waited_every(
        "http://ok/v1/jobs/j1",
        Some(QUEUE),
        false,
        Duration::from_secs(2),
    );
    assert_eq!(done.result, Ok(()), "done");
    let secs: Vec<u64> = done.pauses.iter().map(Duration::as_secs).collect();
    assert_eq!(
        secs,
        [2, 3, 4, 5, 5, 5],
        "one pause after each poll but the last"
    );
}

#[test]
fn a_failed_poll_is_tried_again_and_the_wait_goes_on() {
    let script = "x\n~503 - <html>bad gateway</html>\n{\"state\":\"queued\",\"position\":1,\"running\":1}\n~429 7\n~502 300\nx\nx\nx\n{\"state\":\"done\",\"response\":{}}\n";
    let done = waited_every(
        "http://ok/v1/jobs/j1",
        Some(script),
        false,
        Duration::from_secs(2),
    );
    assert_eq!(done.result, Ok(()), "{}", done.stderr);
    let secs: Vec<u64> = done.pauses.iter().map(Duration::as_secs).collect();
    assert_eq!(
        secs,
        [2, 4, 2, 7, 20, 8, 16, 20],
        "doubling, Retry-After when longer, at most 20 s; the count starts again after a good poll"
    );
    assert_eq!(
        done.stderr,
        "waiting: no answer from the server, try 1 of 5 in 2 s\nwaiting: no answer from the server, try 2 of 5 in 4 s\nwaiting: the server is busy, you are next\nwaiting: no answer from the server, try 1 of 5 in 7 s\nwaiting: no answer from the server, try 2 of 5 in 20 s\nwaiting: no answer from the server, try 3 of 5 in 8 s\nwaiting: no answer from the server, try 4 of 5 in 16 s\nwaiting: no answer from the server, try 5 of 5 in 20 s\nrunning…\n",
        "each try is shown"
    );
    let log = fs::read_to_string(done.dir.0.join("fake-curl.log")).unwrap();
    assert_eq!(log.lines().count(), 9, "{log}");
}

#[test]
fn polls_that_keep_failing_give_up_after_five_retries() {
    for (script, why) in [
        ("x\n", "curl: (7) Failed to connect"),
        (
            "~429 3\n",
            "(too many requests from this address: try again in 3 s",
        ),
        (
            "~500 - {\"error\":{\"code\":\"internal\",\"message\":\"m\"}}\n",
            "curl: (22) The requested URL returned error: 500 ({\"error\":{\"code\":\"internal\",\"message\":\"m\"}})",
        ),
    ] {
        let done = waited_every("http://ok/v1/jobs/j1", Some(script), true, Duration::ZERO);
        let gave_up = done.result.unwrap_err();
        assert!(
            gave_up.starts_with("gave up waiting for the job after 5 retries (")
                && gave_up.ends_with(&format!("{why}); run the command again")),
            "{gave_up}"
        );
        assert_eq!(done.pauses.len(), 5, "{script}");
        assert!(
            done.stderr
                .contains("\r\x1b[Kwaiting: no answer from the server, try 5 of 5 in ")
                && done.stderr.ends_with(" s\n"),
            "the line is ended: {:?}",
            done.stderr
        );
        let log = fs::read_to_string(done.dir.0.join("fake-curl.log")).unwrap();
        assert_eq!(
            log.lines().count(),
            6,
            "the first poll and five retries: {log}"
        );
    }
    let (gone, shown, _, _dir) = waited("http://ok/v1/jobs/j1", Some("~410 30 {}\n"), false);
    assert!(
        gone.unwrap_err().ends_with("error: 410 ({})"),
        "a 4xx is not tried again"
    );
    assert_eq!(shown, "", "no retry shown");
}

#[test]
fn the_retry_wait_doubles_honours_retry_after_and_is_capped() {
    let every = Duration::from_secs(2);
    let secs: Vec<u64> = (1..=RETRIES)
        .map(|failures| backoff(failures, every, None).as_secs())
        .collect();
    assert_eq!(secs, [2, 4, 8, 16, 20], "doubling up to the cap");
    assert_eq!(backoff(1, every, Some(7)), Duration::from_secs(7), "longer");
    assert_eq!(
        backoff(3, every, Some(7)),
        Duration::from_secs(8),
        "shorter"
    );
    assert_eq!(backoff(1, every, Some(300)), LONGEST, "capped");
    assert_eq!(backoff(40, every, None), LONGEST, "no overflow");
    assert_eq!(
        backoff(1, Duration::ZERO, Some(3)),
        Duration::from_secs(3),
        "from the server alone"
    );
    assert!(
        LONGEST * RETRIES < Duration::from_secs(120),
        "inside the server's expiry of an unpolled job"
    );
}

#[test]
fn the_header_dump_gives_the_last_status_and_retry_after() {
    assert_eq!(answered(""), (None, None), "no answer");
    assert_eq!(
        answered(
            "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 503 Service Unavailable\r\nretry-after: 30\r\nContent-Type: x\r\n\r\n"
        ),
        (Some(503), Some(30)),
        "after the interim answer, any case"
    );
    assert_eq!(
        answered("HTTP/2 429\r\nRetry-After: Wed, 21 Oct 2026 07:28:00 GMT\r\n"),
        (Some(429), None),
        "a date is not seconds"
    );
    assert_eq!(answered("HTTP/1.1 abc\r\n"), (None, None), "no code");
}

#[test]
fn refusals_are_told_in_a_sentence() {
    let full =
        r#"{"error":{"code":"queue-full","message":"the queue is full (50 jobs); retry later"}}"#;
    assert_eq!(
        explain("u", Some(503), Some(30), full, "c"),
        "the server is busy: 50 jobs queued, try again in 30 s",
        "full"
    );
    let unnumbered = r#"{"error":{"code":"queue-full","message":"full"}}"#;
    assert_eq!(
        explain("u", Some(503), None, unnumbered, "c"),
        "the server is busy: its queue is full, try again later",
        "no number, no Retry-After"
    );
    let busy = r#"{"error":{"code":"backstop.unavailable","message":"m"}}"#;
    assert_eq!(
        explain("u", Some(503), Some(30), busy, "c"),
        format!("the backend at u did not answer: c ({busy})"),
        "another 503"
    );
    let per_ip = r#"{"error":{"code":"per-ip","message":"m"}}"#;
    assert_eq!(
        explain("u", Some(429), None, per_ip, "c"),
        "too many checks from this address at once: try again later, or once one of them has ended",
        "per address"
    );
    let other = r#"{"error":{"code":"rate","message":"m"}}"#;
    assert_eq!(
        explain("u", Some(429), None, other, "c"),
        format!("the backend at u did not answer: c ({other})"),
        "another 429 code"
    );
    assert_eq!(
        explain("u", Some(429), None, " \n", "c"),
        "too many requests from this address: try again later",
        "the edge"
    );
    assert_eq!(
        explain("u", Some(429), None, "<html>slow down</html>", "c"),
        "the backend at u did not answer: c (<html>slow down</html>)",
        "a body that is not the server's"
    );
    assert_eq!(
        explain("u", Some(422), None, per_ip, "c"),
        format!("the backend at u did not answer: c ({per_ip})"),
        "another status"
    );
    assert_eq!(
        explain("u", None, None, "", "c"),
        "the backend at u did not answer: c",
        "no answer"
    );
    assert_eq!(first_number("at most 12 of 30"), Some(12), "the first");
    assert_eq!(first_number("none"), None, "none");
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
