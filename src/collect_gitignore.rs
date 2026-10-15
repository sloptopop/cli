//! Keeping collected reports out of version control: they can hold secret
//! snippets (even redacted, their locations) and machine paths.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

/// Whether a `.gitignore` text ignores the `.sloptopop` directory (or its
/// `reports` subdirectory) as a whole. Later lines win, so a negation
/// such as `!.sloptopop/` undoes an earlier match.
pub(crate) fn ignores_sloptopop(gitignore: &str) -> bool {
    gitignore.lines().fold(false, |ignored, line| {
        let line = line.trim();
        let (negated, pattern) = match line.strip_prefix('!') {
            Some(pattern) => (true, pattern),
            None => (false, line),
        };
        if covers_reports(pattern) {
            !negated
        } else {
            ignored
        }
    })
}

/// Whether one pattern matches `.sloptopop` or `.sloptopop/reports`.
fn covers_reports(pattern: &str) -> bool {
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    let pattern = ["/**", "/*", "/"]
        .iter()
        .find_map(|suffix| pattern.strip_suffix(suffix))
        .unwrap_or(pattern);
    matches!(pattern, ".sloptopop" | ".sloptopop/reports")
}

/// Makes sure git ignores the reports directory `reports`: unless the
/// `.gitignore` in `root` already ignores `.sloptopop`, the reports
/// directory's own `.gitignore` must hold a `*` line (it is added,
/// keeping any other lines).
pub(crate) fn ensure_ignored(root: &Path, reports: &Path) -> io::Result<()> {
    let root_rules = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    if ignores_sloptopop(&root_rules) {
        return Ok(());
    }
    let path = reports.join(".gitignore");
    let mut existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    if existing.lines().any(|line| line.trim() == "*") {
        return Ok(());
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str("*\n");
    fs::write(path, existing)
}

#[cfg(test)]
mod tests;
