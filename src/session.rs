//! One JSON-RPC session's state machine: turns each message body into the
//! reply (if any) and the exit code (if the session ends). No I/O.
//!
//! Lifecycle (same as LSP): `initialize` → any requests → `shutdown` →
//! `exit`. Requests before `initialize` get `SERVER_NOT_INITIALIZED`;
//! requests after `shutdown` get `INVALID_REQUEST`. The exit code is 0 if
//! `shutdown` came first, otherwise 1. Notifications other than `exit` are
//! ignored. Batches are not supported.
//!
//! Standards: SCF NET-35 (Application Programming Interface Security:
//! requests before `initialize` are refused), TDA-12.

use serde_json::{Value, json};

use crate::handshake::{InitializeParams, InitializeResult, PROTOCOL_VERSION};
use crate::jsonrpc::{ErrorObject, Id, Message, error_code, failure, success};

/// Where the session is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Uninitialized,
    Running,
    ShutDown,
}

/// What handling one message produced.
#[derive(Debug, PartialEq)]
pub(crate) struct Step {
    /// The reply to send, if any.
    pub(crate) reply: Option<Value>,
    /// The process exit code, if the session ended.
    pub(crate) exit_code: Option<u8>,
}

impl Step {
    fn reply(reply: Value) -> Self {
        Self {
            reply: Some(reply),
            exit_code: None,
        }
    }

    const fn silent() -> Self {
        Self {
            reply: None,
            exit_code: None,
        }
    }
}

/// One client session.
#[derive(Debug)]
pub(crate) struct Session {
    state: State,
}

impl Session {
    /// A session waiting for `initialize`.
    pub(crate) const fn new() -> Self {
        Self {
            state: State::Uninitialized,
        }
    }

    /// The exit code if the session ends now: 0 after `shutdown`,
    /// otherwise 1.
    pub(crate) fn exit_code(&self) -> u8 {
        u8::from(self.state != State::ShutDown)
    }

    /// Handles one message body.
    pub(crate) fn handle(&mut self, body: &[u8]) -> Step {
        let value: Value = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(e) => {
                let error = ErrorObject::new(error_code::PARSE_ERROR, format!("parse error: {e}"));
                return Step::reply(failure(None, &error));
            }
        };
        let id = value
            .get("id")
            .and_then(|id| serde_json::from_value::<Id>(id.clone()).ok());
        let message = match serde_json::from_value::<Message>(value) {
            Ok(message) if message.jsonrpc == "2.0" => message,
            Ok(_) => return invalid_request(id.as_ref(), "jsonrpc must be \"2.0\""),
            Err(e) => return invalid_request(id.as_ref(), &format!("invalid request: {e}")),
        };
        match message.id {
            Some(request_id) => {
                Step::reply(self.request(&request_id, &message.method, message.params))
            }
            None => self.notification(&message.method),
        }
    }

    fn notification(&self, method: &str) -> Step {
        if method == "exit" {
            Step {
                reply: None,
                exit_code: Some(self.exit_code()),
            }
        } else {
            Step::silent()
        }
    }

    fn request(&mut self, id: &Id, method: &str, params: Option<Value>) -> Value {
        let outcome = match (self.state, method) {
            (State::Uninitialized, "initialize") => self.initialize(params),
            (State::Uninitialized, _) => Err(ErrorObject::new(
                error_code::SERVER_NOT_INITIALIZED,
                "the first request must be initialize",
            )),
            (State::ShutDown, _) => Err(ErrorObject::new(
                error_code::INVALID_REQUEST,
                "the server is shut down; only the exit notification is allowed",
            )),
            (State::Running, "initialize") => Err(ErrorObject::new(
                error_code::INVALID_REQUEST,
                "initialize may only be sent once",
            )),
            (State::Running, "shutdown") => {
                self.state = State::ShutDown;
                Ok(Value::Null)
            }
            (State::Running, _) => Err(ErrorObject::new(
                error_code::METHOD_NOT_FOUND,
                format!("method not found: {method}"),
            )),
        };
        match outcome {
            Ok(result) => success(Some(id), &result),
            Err(error) => failure(Some(id), &error),
        }
    }

    fn initialize(&mut self, params: Option<Value>) -> Result<Value, ErrorObject> {
        let params: InitializeParams = serde_json::from_value(params.unwrap_or(Value::Null))
            .map_err(|e| {
                ErrorObject::new(error_code::INVALID_PARAMS, format!("invalid params: {e}"))
            })?;
        if params.protocol_version != PROTOCOL_VERSION {
            return Err(ErrorObject {
                code: error_code::UNSUPPORTED_PROTOCOL_VERSION,
                message: format!(
                    "unsupported protocol version {}; this server speaks {PROTOCOL_VERSION}",
                    params.protocol_version
                ),
                data: Some(json!({ "supported": [PROTOCOL_VERSION] })),
            });
        }
        self.state = State::Running;
        Ok(InitializeResult::current().to_json())
    }
}

fn invalid_request(id: Option<&Id>, message: &str) -> Step {
    Step::reply(failure(
        id,
        &ErrorObject::new(error_code::INVALID_REQUEST, message),
    ))
}

#[cfg(test)]
mod tests;
