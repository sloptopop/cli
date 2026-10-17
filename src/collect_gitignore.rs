//! Keeping collected reports out of version control: they can hold secret
//! snippets (even redacted, their locations) and machine paths. The managed
//! tools directory, which holds installed packages, is kept out the same
//! way.
//!
//! Standards: SCF CFG-17.3 (Software Development Repository: reports
//! that can hold secrets never enter version control); CWE-538.

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

/// Whether a `.gitignore` text ignores the `.sloptopop` directory (or its
/// subdirectory `sub`) as a whole. Later lines win, so a negation such as
/// `!.sloptopop/` undoes an earlier match.
pub(crate) fn ignores_sloptopop(gitignore: &str, sub: &str) -> bool {
    gitignore.lines().fold(false, |ignored, line| {
        let line = line.trim();
        let (negated, pattern) = match line.strip_prefix('!') {
            Some(pattern) => (true, pattern),
            None => (false, line),
        };
        if covers(pattern, sub) {
            !negated
        } else {
            ignored
        }
    })
}

/// Whether one pattern matches `.sloptopop` or `.sloptopop/<sub>`.
fn covers(pattern: &str, sub: &str) -> bool {
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    let pattern = ["/**", "/*", "/"]
        .iter()
        .find_map(|suffix| pattern.strip_suffix(suffix))
        .unwrap_or(pattern);
    pattern == ".sloptopop" || pattern.strip_prefix(".sloptopop/") == Some(sub)
}

/// Makes sure git ignores `.sloptopop/<sub>` in `root`, an existing
/// directory: unless the `.gitignore` in `root` already ignores it or
/// `.sloptopop`, the directory's own `.gitignore` must hold a `*` line (it
/// is added, keeping any other lines).
pub(crate) fn ensure_ignored(root: &Path, sub: &str) -> io::Result<()> {
    let root_rules = fs::read_to_string(root.join(".gitignore")).unwrap_or_default();
    if ignores_sloptopop(&root_rules, sub) {
        return Ok(());
    }
    let path = root.join(".sloptopop").join(sub).join(".gitignore");
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
