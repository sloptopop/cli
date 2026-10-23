//! Which ecosystems a repository uses, from the manifests in its root
//! directory (the same signals the engine uses).
//!
//! Standards: SCF VPM-12.2 (Breadth / Depth of Coverage For
//! Vulnerability Scanning: the ecosystems decide which analyzers apply).

use std::collections::BTreeSet;
use std::fs;
use std::io;
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
    /// Shell scripts (`*.sh`).
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

/// The ecosystems signalled by the files directly in `root`; entries that
/// cannot be read are ignored.
pub(crate) fn detect(root: &Path) -> io::Result<BTreeSet<Ecosystem>> {
    Ok(fs::read_dir(root)?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| !kind.is_dir()))
        .filter_map(|entry| Ecosystem::of_file(&entry.file_name().to_string_lossy()))
        .collect())
}

#[cfg(test)]
mod tests;
