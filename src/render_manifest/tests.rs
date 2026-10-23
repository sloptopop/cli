//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::json;

use super::manifest;

#[test]
fn the_manifest_says_what_is_sent_and_what_is_not() {
    let full = json!({
        "mode": "transform", "files": 10, "bytes": 2515, "requestBytes": 7586, "context": true,
        "reports": {"sent": ["gitleaks.sarif", "ruff.sarif"], "left": [{"name": "bad.sarif", "reason": "a SARIF report that is not JSON"}]},
        "masked": {"values": 2, "files": ["src/pay.ts"]},
        "excluded": {"count": 2, "byReason": {"not UTF-8 text": 1, "symbolic link": 1},
                     "paths": [{"path": "logo.png", "reason": "not UTF-8 text"},
                               {"path": "link", "reason": "symbolic link"}]}
    });
    assert_eq!(
        manifest(&full, "https://backend.example/v1/transform"),
        "sloptopop transform will upload to https://backend.example/v1/transform:\n  10 files, 2515 bytes of text (request: 7586 bytes), 2 analyzer report(s) and .sloptopop/context.json\n  analyzer report not uploaded: bad.sarif (a SARIF report that is not JSON)\n  2 secret value(s) masked before upload (in src/pay.ts)\n  2 file(s) not uploaded: 1 not UTF-8 text; 1 symbolic link\n    logo.png (not UTF-8 text)\n    link (symbolic link)\n",
        "full"
    );
    assert_eq!(
        manifest(&json!({}), "http://192.0.2.10:8080/v1/transform"),
        "sloptopop will upload to http://192.0.2.10:8080/v1/transform:\n  0 files, 0 bytes of text (request: 0 bytes), 0 analyzer report(s)\n  0 secret value(s) masked before upload\n  0 file(s) not uploaded\n",
        "empty"
    );
}
