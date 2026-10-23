//! Which ecosystems a repository uses, from the manifests in its root
//! directory (the same signals the engine uses), and, for shell, from the
//! scripts anywhere below it: a file named `*.sh` or `*.bash`, or one
//! whose first line is a shebang that runs a Bourne shell, the same files
//! the `ShellCheck` image's wrapper checks.
//!
//! Standards: SCF VPM-12.2 (Breadth / Depth of Coverage For
//! Vulnerability Scanning: the ecosystems decide which analyzers apply).

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// A language ecosystem, in the order reports list them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Ecosystem {
    /// C and C++ (`CMakeLists.txt`).
    Cpp,
    /// .NET (`*.csproj`, or a solution: `*.sln`, `*.slnx`).
    Dotnet,
    /// Go (`go.mod`).
    Go,
    /// Java and Kotlin (`pom.xml`, `build.gradle`, `build.gradle.kts`).
    Jvm,
    /// JavaScript and TypeScript (`package.json`).
    Node,
    /// PHP (`composer.json`).
    Php,
    /// PowerShell (`*.psd1`).
    Powershell,
    /// Python (`pyproject.toml`, `requirements*.txt`).
    Python,
    /// Rust (`Cargo.toml`).
    Rust,
    /// Shell scripts (`*.sh` in the root; `*.sh`, `*.bash` or a Bourne
    /// shell shebang anywhere below it).
    Shell,
    /// Swift (`Package.swift`).
    Swift,
}

impl Ecosystem {
    /// The lowercase name used in reports and on the command line.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Cpp => "cpp",
            Self::Dotnet => "dotnet",
            Self::Go => "go",
            Self::Jvm => "jvm",
            Self::Node => "node",
            Self::Php => "php",
            Self::Powershell => "powershell",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::Shell => "shell",
            Self::Swift => "swift",
        }
    }

    /// The ecosystem a root-directory file name signals, if any.
    pub(crate) fn of_file(name: &str) -> Option<Self> {
        let extension = Path::new(name)
            .extension()
            .map(|ext| ext.to_string_lossy().to_ascii_lowercase());
        let ecosystem = match (name, extension.as_deref()) {
            ("Cargo.toml", _) => Self::Rust,
            ("package.json", _) => Self::Node,
            ("pyproject.toml", _) => Self::Python,
            ("go.mod", _) => Self::Go,
            ("pom.xml" | "build.gradle" | "build.gradle.kts", _) => Self::Jvm,
            ("composer.json", _) => Self::Php,
            ("Package.swift", _) => Self::Swift,
            ("CMakeLists.txt", _) => Self::Cpp,
            (_, Some("txt")) if name.starts_with("requirements") => Self::Python,
            (_, Some("csproj" | "sln" | "slnx")) => Self::Dotnet,
            (_, Some("psd1")) => Self::Powershell,
            (_, Some("sh")) => Self::Shell,
            _ => return None,
        };
        Some(ecosystem)
    }
}

/// The ecosystems signalled by the files directly in `root`, and shell if
/// a shell script is anywhere below it; entries that cannot be read are
/// ignored.
pub(crate) fn detect(root: &Path) -> io::Result<BTreeSet<Ecosystem>> {
    let mut found: BTreeSet<Ecosystem> = fs::read_dir(root)?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .filter_map(|entry| Ecosystem::of_file(&entry.file_name().to_string_lossy()))
        .collect();
    if !found.contains(&Ecosystem::Shell) && has_script(root) {
        found.insert(Ecosystem::Shell);
    }
    Ok(found)
}

/// The directories the `ShellCheck` wrapper skips.
const SKIPPED: [&str; 3] = [".git", ".sloptopop", "node_modules"];

/// Whether a shell script is below `dir` (links are not followed).
fn has_script(dir: &Path) -> bool {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    !SKIPPED.contains(&name.as_str()) && has_script(&entry.path())
                }
                Ok(kind) if kind.is_file() => {
                    // As the wrapper's `find -name '*.sh'`: case-sensitive.
                    name.rsplit_once('.')
                        .is_some_and(|(_, extension)| matches!(extension, "sh" | "bash"))
                        || first_line(&entry.path()).is_some_and(|line| is_shell_shebang(&line))
                }
                Ok(_) | Err(_) => false,
            }
        })
}

/// The first line of the file at `path` (up to 256 bytes of it).
fn first_line(path: &Path) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(256).read_to_end(&mut head))
        .ok()
        .map(|_| {
            head.split(|&b| b == b'\n')
                .next()
                .unwrap_or_default()
                .to_vec()
        })
}

/// Whether `line` is a shebang that runs sh, bash, dash, ksh, ash or
/// busybox sh, by path or through env (`#!/usr/bin/env -S bash`): the
/// wrapper's pattern. Words are split at spaces and tabs only.
fn is_shell_shebang(line: &[u8]) -> bool {
    let Some(rest) = line.strip_prefix(b"#!") else {
        return false;
    };
    let text = String::from_utf8_lossy(rest);
    let mut words = text.split([' ', '\t']).filter(|word| !word.is_empty());
    let Some((_, program)) = words.next().and_then(|first| first.rsplit_once('/')) else {
        return false;
    };
    let shell = |name: &str| matches!(name, "sh" | "bash" | "dash" | "ksh" | "ash");
    match program {
        "env" => {
            let next = words.next().unwrap_or_default();
            let name = if next == "-S" {
                words.next().unwrap_or_default()
            } else {
                next
            };
            shell(name) || (name == "busybox" && words.next() == Some("sh"))
        }
        "busybox" => words.next() == Some("sh"),
        name => shell(name),
    }
}

#[cfg(test)]
mod tests;
