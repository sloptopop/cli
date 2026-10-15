//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.

use super::*;

/// The committed schema is what clients generate code from, so it must
/// match the Rust types. Regenerate with:
/// `cargo run -q -- schema > schema/protocol.json`
#[test]
fn committed_schema_is_up_to_date() {
    let committed: Value =
        serde_json::from_str(include_str!("../../schema/protocol.json")).unwrap_or_default();
    assert_eq!(committed, document(), "schema/protocol.json is stale");
}

#[test]
fn methods_reference_their_types() {
    let doc = document();
    assert_eq!(
        doc["methods"]["initialize"]["params"]["$ref"],
        json!("#/$defs/InitializeParams"),
        "initialize params"
    );
    assert!(
        doc["$defs"]["InitializeResult"].is_object(),
        "result defined"
    );
}
