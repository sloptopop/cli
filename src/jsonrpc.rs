//! The JSON-RPC 2.0 message model: request ids, incoming messages, error
//! objects and codes, and the success/error response envelopes.
//!
//! Standards: SCF NET-35 (Application Programming Interface Security),
//! TDA-12 (malformed messages get JSON-RPC errors).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// JSON-RPC and sloptopop error codes.
pub mod error_code {
    /// The message body is not valid JSON.
    pub const PARSE_ERROR: i64 = -32700;
    /// The JSON is not a valid request object, or the request is not
    /// allowed in the current session state.
    pub const INVALID_REQUEST: i64 = -32600;
    /// The method does not exist.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// The params are missing or have the wrong shape.
    pub const INVALID_PARAMS: i64 = -32602;
    /// The client asked for a protocol version this binary does not speak.
    pub const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32001;
    /// A request other than `initialize` arrived before `initialize`.
    pub const SERVER_NOT_INITIALIZED: i64 = -32002;
}

/// A JSON-RPC request id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Id {
    /// A numeric id.
    Number(i64),
    /// A string id.
    String(String),
}

impl Id {
    /// The id as a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        match self {
            Self::Number(n) => json!(n),
            Self::String(s) => json!(s),
        }
    }
}

/// A JSON-RPC request (`id` present) or notification (`id` absent or null).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,
    /// Request id; `None` makes this a notification, which gets no reply.
    #[serde(default)]
    pub id: Option<Id>,
    /// Method name.
    pub method: String,
    /// Method parameters.
    #[serde(default)]
    pub params: Option<Value>,
}

/// A JSON-RPC error object.
#[derive(Debug, Clone, PartialEq, JsonSchema)]
pub struct ErrorObject {
    /// One of the codes in [`error_code`].
    pub code: i64,
    /// Human-readable description.
    pub message: String,
    /// Structured detail, if any.
    pub data: Option<Value>,
}

impl ErrorObject {
    /// An error without `data`.
    #[must_use]
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// The error as a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut obj = json!({ "code": self.code, "message": self.message });
        if let (Some(data), Some(map)) = (&self.data, obj.as_object_mut()) {
            map.insert("data".into(), data.clone());
        }
        obj
    }
}

/// Builds a success response. The `id` is `null` only when the request's
/// id could not be determined.
#[must_use]
pub fn success(id: Option<&Id>, result: &Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.map(Id::to_json), "result": result })
}

/// Builds an error response.
#[must_use]
pub fn failure(id: Option<&Id>, error: &ErrorObject) -> Value {
    json!({ "jsonrpc": "2.0", "id": id.map(Id::to_json), "error": error.to_json() })
}

#[cfg(test)]
mod tests;
