//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use super::*;

#[test]
fn initialize_result_reports_this_binary() {
    assert_eq!(
        InitializeResult::current().to_json(),
        json!({
            "serverName": "sloptopop",
            "serverVersion": SERVER_VERSION,
            "protocolVersion": PROTOCOL_VERSION,
        }),
        "initialize result"
    );
}
