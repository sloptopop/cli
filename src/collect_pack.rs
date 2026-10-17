//! sloptopop's own Opengrep and Semgrep rule pack, built into the binary:
//! the `rules/` directory of this repository (MIT OR Apache-2.0, like the
//! rest of it; see `rules/README.md`). `collect` writes it to [`PACK_DIR`]
//! and passes that directory to Opengrep and Semgrep when no `--rules`
//! directory is given, so neither tool ever falls back to registry rules.
//!
//! Standards: SCF TDA-17.1 (Static Code Analysis), CFG-13 (Open Source
//! Software: sloptopop's own rules, never registry rules).

use std::fs;
use std::io::{self, ErrorKind};
use std::path::Path;

/// Where `collect` writes the pack, relative to the root: a hidden
/// directory inside the reports directory, so it is ignored by git with
/// the reports and never read by the engine as a report.
pub(crate) const PACK_DIR: &str = ".sloptopop/reports/.rules";

/// One rule file of the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    /// Its directory in `rules/` (the rule id's area).
    pub(crate) area: &'static str,
    /// Its file name.
    pub(crate) file: &'static str,
    /// Its YAML.
    pub(crate) text: &'static str,
}

/// Every rule file in `rules/`, sorted by path.
pub(crate) const PACK: &[Rule] = &[
    Rule {
        area: "config",
        file: "go-local-url.yaml",
        text: include_str!("../rules/config/go-local-url.yaml"),
    },
    Rule {
        area: "config",
        file: "js-local-url.yaml",
        text: include_str!("../rules/config/js-local-url.yaml"),
    },
    Rule {
        area: "config",
        file: "python-local-url.yaml",
        text: include_str!("../rules/config/python-local-url.yaml"),
    },
    Rule {
        area: "errors",
        file: "empty-catch.yaml",
        text: include_str!("../rules/errors/empty-catch.yaml"),
    },
    Rule {
        area: "errors",
        file: "empty-promise-catch.yaml",
        text: include_str!("../rules/errors/empty-promise-catch.yaml"),
    },
    Rule {
        area: "errors",
        file: "go-empty-err-check.yaml",
        text: include_str!("../rules/errors/go-empty-err-check.yaml"),
    },
    Rule {
        area: "errors",
        file: "python-except-pass.yaml",
        text: include_str!("../rules/errors/python-except-pass.yaml"),
    },
    Rule {
        area: "eval",
        file: "js-dynamic-code.yaml",
        text: include_str!("../rules/eval/js-dynamic-code.yaml"),
    },
    Rule {
        area: "sql",
        file: "go-built-query.yaml",
        text: include_str!("../rules/sql/go-built-query.yaml"),
    },
    Rule {
        area: "sql",
        file: "js-built-query.yaml",
        text: include_str!("../rules/sql/js-built-query.yaml"),
    },
    Rule {
        area: "sql",
        file: "python-built-query.yaml",
        text: include_str!("../rules/sql/python-built-query.yaml"),
    },
    Rule {
        area: "supabase",
        file: "unchecked-write.yaml",
        text: include_str!("../rules/supabase/unchecked-write.yaml"),
    },
    Rule {
        area: "tls",
        file: "go-insecure-skip-verify.yaml",
        text: include_str!("../rules/tls/go-insecure-skip-verify.yaml"),
    },
    Rule {
        area: "tls",
        file: "js-reject-unauthorized.yaml",
        text: include_str!("../rules/tls/js-reject-unauthorized.yaml"),
    },
    Rule {
        area: "tls",
        file: "python-verify-false.yaml",
        text: include_str!("../rules/tls/python-verify-false.yaml"),
    },
];

/// Writes the pack to `dir`, replacing whatever was there, so a rule
/// dropped from the pack never lingers from an older run.
pub(crate) fn write(dir: &Path) -> io::Result<()> {
    if let Err(e) = fs::remove_dir_all(dir)
        && e.kind() != ErrorKind::NotFound
    {
        return Err(e);
    }
    PACK.iter().try_for_each(|rule| {
        let area = dir.join(rule.area);
        fs::create_dir_all(&area).and_then(|()| fs::write(area.join(rule.file), rule.text))
    })
}

#[cfg(test)]
mod tests;
