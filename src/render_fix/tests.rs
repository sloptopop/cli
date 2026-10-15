//! Unit tests for [`super`]; kept in a `tests.rs` file so coverage
//! measures product code only.
use serde_json::json;

use super::*;

#[test]
fn fix_lists_the_changes() {
    let dry = json!({"applied": false, "changes": [
        {"path": ".gitignore", "template": "gitignore", "status": "exists"},
        {"path": "rust-toolchain.toml", "template": "rust-toolchain", "status": "planned"}
    ]});
    assert_eq!(
        fix(&dry),
        "\
sloptopop fix: dry run, nothing written
  exists   .gitignore  (template gitignore)
  planned  rust-toolchain.toml  (template rust-toolchain)
Run `sloptopop fix --apply` to create the planned files.
",
        "dry run"
    );
    let applied = json!({"applied": true, "changes": [
        {"path": "rust-toolchain.toml", "template": "rust-toolchain", "status": "created"}
    ]});
    assert_eq!(
        fix(&applied),
        "sloptopop fix: applied\n  created  rust-toolchain.toml  (template rust-toolchain)\n",
        "applied"
    );
    let nothing =
        json!({"applied": false, "changes": [{"path": "x", "template": "t", "status": "exists"}]});
    assert!(
        !fix(&nothing).contains("Run `sloptopop"),
        "no hint without planned files"
    );
    assert_eq!(
        fix(&json!({"applied": false, "changes": []})),
        "sloptopop fix: dry run, nothing written\n  nothing to fix\n",
        "empty"
    );
}
