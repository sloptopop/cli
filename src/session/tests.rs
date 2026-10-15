//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use super::*;
use crate::tests::support::initialize;

fn handle(session: &mut Session, message: &Value) -> Step {
    session.handle(message.to_string().as_bytes())
}

fn error_code_of(step: &Step) -> Option<i64> {
    step.reply.as_ref()?.get("error")?.get("code")?.as_i64()
}

#[test]
fn full_lifecycle() {
    let mut s = Session::new();
    let init = handle(&mut s, &initialize(1));
    assert_eq!(
        init.reply,
        Some(success(
            Some(&Id::Number(1)),
            &InitializeResult::current().to_json()
        )),
        "initialize result"
    );
    let shutdown = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": "s", "method": "shutdown" }),
    );
    assert_eq!(
        shutdown.reply,
        Some(json!({ "jsonrpc": "2.0", "id": "s", "result": null })),
        "shutdown result"
    );
    let exit = handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "exit" }));
    assert_eq!(
        exit,
        Step {
            reply: None,
            exit_code: Some(0)
        },
        "clean exit"
    );
}

#[test]
fn exit_without_shutdown_is_unclean() {
    let mut s = Session::new();
    let exit = handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "exit" }));
    assert_eq!(exit.exit_code, Some(1), "exit code 1");
}

#[test]
fn requests_before_initialize_are_refused() {
    let mut s = Session::new();
    let step = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "shutdown" }),
    );
    assert_eq!(
        error_code_of(&step),
        Some(error_code::SERVER_NOT_INITIALIZED),
        "not initialized"
    );
}

#[test]
fn requests_after_shutdown_are_refused() {
    let mut s = Session::new();
    handle(&mut s, &initialize(1));
    handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }),
    );
    let step = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 3, "method": "shutdown" }),
    );
    assert_eq!(
        error_code_of(&step),
        Some(error_code::INVALID_REQUEST),
        "shut down"
    );
}

#[test]
fn initialize_twice_is_refused() {
    let mut s = Session::new();
    handle(&mut s, &initialize(1));
    let step = handle(&mut s, &initialize(2));
    assert_eq!(
        error_code_of(&step),
        Some(error_code::INVALID_REQUEST),
        "second initialize"
    );
}

#[test]
fn unknown_method_is_not_found() {
    let mut s = Session::new();
    handle(&mut s, &initialize(1));
    let step = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 2, "method": "nope" }),
    );
    assert_eq!(
        error_code_of(&step),
        Some(error_code::METHOD_NOT_FOUND),
        "unknown method"
    );
}

#[test]
fn bad_initialize_params_are_rejected() {
    let mut s = Session::new();
    let missing = handle(
        &mut s,
        &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
    );
    assert_eq!(
        error_code_of(&missing),
        Some(error_code::INVALID_PARAMS),
        "missing params"
    );
    let mut wrong = initialize(2);
    if let Some(params) = wrong.get_mut("params") {
        params["protocolVersion"] = json!(PROTOCOL_VERSION + 1);
    }
    let step = handle(&mut s, &wrong);
    assert_eq!(
        error_code_of(&step),
        Some(error_code::UNSUPPORTED_PROTOCOL_VERSION),
        "wrong version"
    );
    assert_eq!(s.state, State::Uninitialized, "still uninitialized");
}

#[test]
fn malformed_messages_get_errors() {
    let mut s = Session::new();
    let parse = s.handle(b"{not json");
    assert_eq!(
        error_code_of(&parse),
        Some(error_code::PARSE_ERROR),
        "not JSON"
    );
    let version = handle(&mut s, &json!({ "jsonrpc": "1.0", "id": 4, "method": "x" }));
    assert_eq!(
        error_code_of(&version),
        Some(error_code::INVALID_REQUEST),
        "wrong jsonrpc"
    );
    assert_eq!(
        version.reply.as_ref().and_then(|r| r.get("id")),
        Some(&json!(4)),
        "id echoed when readable"
    );
    let shape = handle(&mut s, &json!([1, 2]));
    assert_eq!(
        error_code_of(&shape),
        Some(error_code::INVALID_REQUEST),
        "batch"
    );
}

#[test]
fn other_notifications_are_ignored() {
    let mut s = Session::new();
    let step = handle(&mut s, &json!({ "jsonrpc": "2.0", "method": "$/anything" }));
    assert_eq!(step, Step::silent(), "no reply, no exit");
}
