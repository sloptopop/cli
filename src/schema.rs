//! The protocol description that language clients are generated and
//! tested from: methods, error codes, and JSON Schemas (draft 2020-12) of
//! every params/result type, all derived from the Rust types of the
//! `protocol` module.
//!
//! Standards: SCF TDA-08 (Technical Documentation Artifacts: the protocol
//! description clients are generated from), NET-35.

use schemars::JsonSchema;
use schemars::generate::{SchemaGenerator, SchemaSettings};
use serde_json::{Value, json};

use crate::handshake::{InitializeParams, InitializeResult, PROTOCOL_VERSION};
use crate::jsonrpc::{ErrorObject, Id, error_code};

fn reference<T: JsonSchema>(generator: &mut SchemaGenerator) -> Value {
    generator.subschema_for::<T>().to_value()
}

/// Builds the protocol description document.
#[must_use]
pub fn document() -> Value {
    let mut generator = SchemaSettings::draft2020_12().into_generator();
    let id = reference::<Id>(&mut generator);
    let error = reference::<ErrorObject>(&mut generator);
    let initialize_params = reference::<InitializeParams>(&mut generator);
    let initialize_result = reference::<InitializeResult>(&mut generator);
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "sloptopop protocol",
        "description": "JSON-RPC 2.0 over stdin/stdout of `sloptopop serve --stdio`, framed as `Content-Length: N\\r\\n\\r\\n<N bytes of UTF-8 JSON>`.",
        "protocolVersion": PROTOCOL_VERSION,
        "id": id,
        "error": error,
        "errorCodes": {
            "PARSE_ERROR": error_code::PARSE_ERROR,
            "INVALID_REQUEST": error_code::INVALID_REQUEST,
            "METHOD_NOT_FOUND": error_code::METHOD_NOT_FOUND,
            "INVALID_PARAMS": error_code::INVALID_PARAMS,
            "UNSUPPORTED_PROTOCOL_VERSION": error_code::UNSUPPORTED_PROTOCOL_VERSION,
            "SERVER_NOT_INITIALIZED": error_code::SERVER_NOT_INITIALIZED,
        },
        "methods": {
            "initialize": {
                "kind": "request",
                "description": "First request of every session; negotiates the protocol version.",
                "params": initialize_params,
                "result": initialize_result,
            },
            "shutdown": {
                "kind": "request",
                "description": "Asks the server to stop; only `exit` may follow.",
                "params": { "type": "null" },
                "result": { "type": "null" },
            },
            "exit": {
                "kind": "notification",
                "description": "Ends the process: exit code 0 after `shutdown`, otherwise 1.",
            },
        },
        "$defs": generator.take_definitions(true),
    })
}

#[cfg(test)]
mod tests;
