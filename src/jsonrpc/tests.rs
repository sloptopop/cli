//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use super::*;

#[test]
fn ids_serialize_as_their_json_type() {
    assert_eq!(Id::Number(7).to_json(), json!(7), "numeric id");
    assert_eq!(Id::String("a".into()).to_json(), json!("a"), "string id");
}

#[test]
fn error_data_is_included_only_when_present() {
    let plain = ErrorObject::new(error_code::PARSE_ERROR, "bad");
    assert_eq!(
        plain.to_json(),
        json!({ "code": -32700, "message": "bad" }),
        "no data key without data"
    );
    let detailed = ErrorObject {
        data: Some(json!({ "x": 1 })),
        ..plain
    };
    assert_eq!(
        detailed.to_json(),
        json!({ "code": -32700, "message": "bad", "data": { "x": 1 } }),
        "data key with data"
    );
}

#[test]
fn responses_carry_jsonrpc_and_id() {
    let id = Id::Number(1);
    assert_eq!(
        success(Some(&id), &json!(null)),
        json!({ "jsonrpc": "2.0", "id": 1, "result": null }),
        "success"
    );
    assert_eq!(
        failure(None, &ErrorObject::new(error_code::PARSE_ERROR, "bad")),
        json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": "bad" } }),
        "failure with unknown id"
    );
}
