//! The upload manifest as text, shown before anything is sent (decision
//! D-0174): where it goes, how many files and bytes, which analyzer
//! reports go with them, what is left out and why, how many secret values
//! are masked.
//!
//! Standards: SCF PRI-02 (notice before collection), PRI-05.

use std::fmt::Write as _;

use serde_json::Value;

use crate::json_fields::{list, number, text};

/// The manifest `manifest` for an upload to `url`, as lines of text.
pub(crate) fn manifest(manifest: &Value, url: &str) -> String {
    let mut out = String::new();
    let field = |value: &'_ Value, key: &str| value.get(key).cloned().unwrap_or(Value::Null);
    let context = if field(manifest, "context").as_bool() == Some(true) {
        " and .sloptopop/context.json"
    } else {
        ""
    };
    let mode = text(manifest, "mode");
    let who = if mode.is_empty() {
        "sloptopop".to_owned()
    } else {
        format!("sloptopop {mode}")
    };
    let reports = &field(manifest, "reports");
    let _head = writeln!(
        out,
        "{who} will upload to {url}:\n  {} files, {} bytes of text (request: {} bytes), {} analyzer report(s){context}",
        number(manifest, "files"),
        number(manifest, "bytes"),
        number(manifest, "requestBytes"),
        list(reports, "sent").len(),
    );
    for left in list(reports, "left") {
        let _left = writeln!(
            out,
            "  analyzer report not uploaded: {} ({})",
            text(left, "name"),
            text(left, "reason")
        );
    }
    let masked = &field(manifest, "masked");
    let files: Vec<&str> = list(masked, "files")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let within = if files.is_empty() {
        String::new()
    } else {
        format!(" (in {})", files.join(", "))
    };
    let _masked = writeln!(
        out,
        "  {} secret value(s) masked before upload{within}",
        number(masked, "values")
    );
    let excluded = &field(manifest, "excluded");
    let reasons: Vec<String> = field(excluded, "byReason")
        .as_object()
        .into_iter()
        .flatten()
        .map(|(reason, n)| format!("{n} {reason}"))
        .collect();
    let why = if reasons.is_empty() {
        String::new()
    } else {
        format!(": {}", reasons.join("; "))
    };
    let _excluded = writeln!(
        out,
        "  {} file(s) not uploaded{why}",
        number(excluded, "count")
    );
    for item in list(excluded, "paths") {
        let _path = writeln!(out, "    {} ({})", text(item, "path"), text(item, "reason"));
    }
    out
}

#[cfg(test)]
mod tests;
