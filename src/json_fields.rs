//! Lenient field access on the JSON documents `sloptopop-engine` prints:
//! a missing or mistyped field reads as empty, so newer engines stay
//! readable.
//!
//! Standards: SCF TDA-12 (Input Data Validation: a missing or mistyped
//! field reads as empty, never as a crash).

use serde_json::Value;

/// The array at `key`, or an empty slice.
pub(crate) fn list<'v>(value: &'v Value, key: &str) -> &'v [Value] {
    value[key].as_array().map_or(&[], Vec::as_slice)
}

/// The string at `key`, or `""`.
pub(crate) fn text<'v>(value: &'v Value, key: &str) -> &'v str {
    value[key].as_str().unwrap_or_default()
}

/// The unsigned number at `key`, or 0.
pub(crate) fn number(value: &Value, key: &str) -> u64 {
    value[key].as_u64().unwrap_or_default()
}
