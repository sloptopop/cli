//! Readable text for the Report that `sloptopop-engine check` prints.
//!
//! Only the public Report format is read; unknown fields are ignored and
//! missing ones render as empty, so newer engines stay readable.
//!
//! Standards: SCF CPL-30 (Control Assurance Automation evidence
//! integrity: findings and controls shown as the engine reported them).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::json_fields::{list, text};

/// Renders a Report: a verdict line, findings grouped by severity
/// (errors, then warnings, then anything else), then the controls.
#[must_use]
pub fn report(report: &Value) -> String {
    let findings = list(report, "findings");
    let ecosystems: Vec<&str> = list(report, "ecosystems")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut out = vec![format!(
        "sloptopop check: {} ({} finding{}; ecosystems: {})\n",
        if report["passed"] == true {
            "passed"
        } else {
            "FAILED"
        },
        findings.len(),
        if findings.len() == 1 { "" } else { "s" },
        if ecosystems.is_empty() {
            "none".to_owned()
        } else {
            ecosystems.join(", ")
        },
    )];
    let mut groups: BTreeMap<(u8, &str), Vec<&Value>> = BTreeMap::new();
    for finding in findings {
        let severity = text(finding, "severity");
        let rank = match severity {
            "error" => 0,
            "warning" => 1,
            _ => 2,
        };
        groups.entry((rank, severity)).or_default().push(finding);
    }
    for ((_, severity), group) in groups {
        let heading = match severity {
            "error" => "Errors",
            "warning" => "Warnings",
            other => other,
        };
        out.push(format!("\n{heading} ({})\n", group.len()));
        out.extend(group.into_iter().map(finding_lines));
    }
    let controls = list(report, "controls");
    if !controls.is_empty() {
        out.push("\nControls\n".to_owned());
        out.extend(controls.iter().map(|control| {
            format!(
                "  {:<7} {:<11} {}\n",
                text(control, "id"),
                text(control, "status"),
                text(control, "title")
            )
        }));
    }
    out.concat()
}

fn finding_lines(finding: &Value) -> String {
    let line = finding["line"]
        .as_u64()
        .map_or_else(String::new, |line| format!(":{line}"));
    let control = finding["control"]
        .as_str()
        .map_or_else(String::new, |control| format!("  [{control}]"));
    let mut out = vec![format!(
        "  {}{line}  {}{control}\n    {}\n",
        text(finding, "path"),
        text(finding, "rule"),
        text(finding, "message")
    )];
    if finding["fix"].is_object() {
        out.push(format!(
            "    fix: `sloptopop fix --apply` creates {}\n",
            text(&finding["fix"], "path")
        ));
    }
    let remediation = text(finding, "remediation");
    if !remediation.is_empty() {
        out.push(format!("    how: {remediation}\n"));
    }
    out.concat()
}

#[cfg(test)]
mod tests;
