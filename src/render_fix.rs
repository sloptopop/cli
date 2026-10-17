//! Readable text for the result that `sloptopop-engine fix` prints.
//!
//! Unknown fields are ignored and missing ones render as empty, so newer
//! engines stay readable.
//!
//! Standards: SCF CHG-07 (Test, Validate & Document Changes: every
//! planned or created file is listed).

use serde_json::Value;

use crate::json_fields::{list, text};

/// Renders the result of `sloptopop-engine fix`.
#[must_use]
pub fn fix(result: &Value) -> String {
    let applied = result["applied"] == true;
    let changes = list(result, "changes");
    let mut out = vec![if applied {
        "sloptopop fix: applied\n".to_owned()
    } else {
        "sloptopop fix: dry run, nothing written\n".to_owned()
    }];
    if changes.is_empty() {
        out.push("  nothing to fix\n".to_owned());
    }
    out.extend(changes.iter().map(|change| {
        format!(
            "  {:<8} {}  (template {})\n",
            text(change, "status"),
            text(change, "path"),
            text(change, "template")
        )
    }));
    if !applied && changes.iter().any(|c| c["status"] == "planned") {
        out.push("Run `sloptopop fix --apply` to create the planned files.\n".to_owned());
    }
    out.concat()
}

#[cfg(test)]
mod tests;
