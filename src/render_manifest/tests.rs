//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::json;

use super::manifest;

#[test]
fn the_manifest_says_what_is_sent_and_what_is_not() {
    let full = json!({
        "files": 10, "bytes": 2515, "requestBytes": 7586, "findings": 11, "context": true,
        "masked": {"values": 2, "files": ["src/pay.ts"]},
        "excluded": {"count": 2, "byReason": {"not UTF-8 text": 1, "symbolic link": 1},
                     "paths": [{"path": "logo.png", "reason": "not UTF-8 text"},
                               {"path": "link", "reason": "symbolic link"}]}
    });
    assert_eq!(
        manifest(&full, "https://backend.example/v1/transform"),
        "sloptopop transform will upload to https://backend.example/v1/transform:\n  10 files, 2515 bytes of text (request: 7586 bytes), the Report (11 findings) and .sloptopop/context.json\n  2 secret value(s) masked before upload (in src/pay.ts)\n  2 file(s) not uploaded: 1 not UTF-8 text; 1 symbolic link\n    logo.png (not UTF-8 text)\n    link (symbolic link)\n",
        "full"
    );
    assert_eq!(
        manifest(&json!({}), "http://192.0.2.10:8080/v1/transform"),
        "sloptopop transform will upload to http://192.0.2.10:8080/v1/transform:\n  0 files, 0 bytes of text (request: 0 bytes), the Report (0 findings)\n  0 secret value(s) masked before upload\n  0 file(s) not uploaded\n",
        "empty"
    );
}
