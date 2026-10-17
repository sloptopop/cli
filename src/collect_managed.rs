//! Analyzers `collect` runs from a directory of their own in
//! [`TOOLS_DIR`], never from the project's dependencies: `ESLint` with
//! eslint-plugin-sonarjs (npm) and `SonarAnalyzer.CSharp` (`NuGet`), each
//! pinned to its last release under an open-source licence.
//!
//! `collect` writes each tool's manifest files (built into the binary from
//! `tools/`) to its directory, but installs nothing: the user runs the
//! install command once, and the package manager checks every download
//! against the pinned hashes. Until the pinned versions are installed, the
//! entries are reported as missing with that command. The directory is
//! ignored by git like the reports.
//!
//! Standards: SCF CFG-13 (Open Source Software: pinned to the last
//! open-source release), TDA-06.5 (Software Release Integrity
//! Verification: the package manager checks the pinned hashes), CFG-03.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Where the managed tools live, relative to the repository root.
pub(crate) const TOOLS_DIR: &str = ".sloptopop/tools";

/// A tool installed in its own directory of [`TOOLS_DIR`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Managed {
    /// `ESLint` with eslint-plugin-sonarjs and the SARIF formatter.
    EslintSonarjs,
    /// `SonarAnalyzer.CSharp` for `dotnet build`.
    SonarCsharp,
}

/// The registry a pinned package comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Registry {
    /// registry.npmjs.org, through `npm ci` and the lockfile.
    Npm,
    /// nuget.org, through `dotnet restore` of `fetch.proj`.
    Nuget,
}

/// A pinned package: its exact version, the hash of its archive and its
/// licence as read from a licence file (never only registry metadata).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pin {
    /// Where it comes from.
    pub(crate) registry: Registry,
    /// The package name or id.
    pub(crate) package: &'static str,
    /// The exact version.
    pub(crate) version: &'static str,
    /// `sha512-<base64>` of the package archive.
    pub(crate) integrity: &'static str,
    /// The licence (SPDX expression) the licence files state.
    pub(crate) licence: &'static str,
    /// The licence files read: inside the archive (`<archive>#<path>`) or,
    /// when the archive has none, in the repository at the released commit.
    pub(crate) licence_files: &'static [&'static str],
}

/// One file `collect` writes to a tool's directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct File {
    /// Its name.
    pub(crate) name: &'static str,
    /// Its text.
    pub(crate) text: &'static str,
}

const ESLINT_SONARJS: &[Pin] = &[
    Pin {
        registry: Registry::Npm,
        package: "eslint-plugin-sonarjs",
        version: "2.0.4",
        integrity: "sha512-XVVAB/t0WSgHitHNajIcIDmviCO8kB9VSsrjy+4WUEVM3eieY9SDHEtCDaOMTjj6XMtcAr8BFDXCFaP005s+tg==",
        licence: "LGPL-3.0-only",
        licence_files: &[
            "https://registry.npmjs.org/eslint-plugin-sonarjs/-/eslint-plugin-sonarjs-2.0.4.tgz#package/LICENSE",
        ],
    },
    Pin {
        registry: Registry::Npm,
        package: "eslint",
        version: "9.14.0",
        integrity: "sha512-c2FHsVBr87lnUtjP4Yhvk4yEhKrQavGafRA/Se1ouse8PfbfC/Qh9Mxa00yWsZRlqeUB9raXip0aiiUZkgnr9g==",
        licence: "MIT",
        licence_files: &["https://registry.npmjs.org/eslint/-/eslint-9.14.0.tgz#package/LICENSE"],
    },
    Pin {
        registry: Registry::Npm,
        package: "@microsoft/eslint-formatter-sarif",
        version: "3.1.0",
        integrity: "sha512-/mn4UXziHzGXnKCg+r8HGgPy+w4RzpgdoqFuqaKOqUVBT5x2CygGefIrO4SusaY7t0C4gyIWMNu6YQT6Jw64Cw==",
        licence: "MIT",
        // The archive holds no licence file; this commit's package.json is
        // 3.1.0 and its sarif.js is the archive's, byte for byte.
        licence_files: &[
            "https://github.com/microsoft/sarif-js-sdk/blob/0ff663e507e989ae751e2e9d94a3360291e3e4e5/LICENSE",
        ],
    },
    Pin {
        registry: Registry::Npm,
        package: "@typescript-eslint/parser",
        version: "7.16.1",
        integrity: "sha512-u+1Qx86jfGQ5i4JjK33/FnawZRpsLxRnKzGE6EABZ40KxVT/vWsiZFEBBHjFOljmmV3MBYOHEKi0Jm9hbAOClA==",
        licence: "BSD-2-Clause",
        licence_files: &[
            "https://registry.npmjs.org/@typescript-eslint/parser/-/parser-7.16.1.tgz#package/LICENSE",
        ],
    },
    Pin {
        registry: Registry::Npm,
        package: "typescript",
        version: "5.5.4",
        integrity: "sha512-Mtq29sKDAEYP7aljRgtPOpTvOfbwRWlS6dPRzwjdE+C0R4brX/GUyhHSecbHMFLNBLcJIPt9nl9yG5TZ1weH+Q==",
        licence: "Apache-2.0",
        licence_files: &[
            "https://registry.npmjs.org/typescript/-/typescript-5.5.4.tgz#package/LICENSE.txt",
        ],
    },
];

const SONAR_CSHARP: &[Pin] = &[Pin {
    registry: Registry::Nuget,
    package: "SonarAnalyzer.CSharp",
    version: "9.32.0.97167",
    integrity: "sha512-NK5VSMCd8IXdKPSZruJY+Lm7aICuIitnAe5Wrmmgc+7x8ue78ee3pdjYib39t/gFBGTy8A5RiocJiNhEatUvWg==",
    licence: "LGPL-3.0-only",
    // The package holds no licence file, only its nuspec's licence
    // expression; the repository's file at the release tag is the text.
    licence_files: &[
        "https://github.com/SonarSource/sonar-dotnet/blob/9.32.0.97167/LICENSE.txt",
        "https://api.nuget.org/v3-flatcontainer/sonaranalyzer.csharp/9.32.0.97167/sonaranalyzer.csharp.9.32.0.97167.nupkg#SonarAnalyzer.CSharp.nuspec",
    ],
}];

const ESLINT_SONARJS_FILES: &[File] = &[
    File {
        name: "package.json",
        text: include_str!("../tools/eslint-sonarjs/package.json"),
    },
    File {
        name: "package-lock.json",
        text: include_str!("../tools/eslint-sonarjs/package-lock.json"),
    },
    File {
        name: "eslint.config.mjs",
        text: include_str!("../tools/eslint-sonarjs/eslint.config.mjs"),
    },
];

const SONAR_CSHARP_FILES: &[File] = &[
    File {
        name: "Directory.Build.props",
        text: include_str!("../tools/sonaranalyzer-csharp/Directory.Build.props"),
    },
    File {
        name: "fetch.proj",
        text: include_str!("../tools/sonaranalyzer-csharp/fetch.proj"),
    },
];

impl Managed {
    /// The tool's directory name in [`TOOLS_DIR`].
    pub(crate) fn dir(self) -> &'static str {
        match self {
            Self::EslintSonarjs => "eslint-sonarjs",
            Self::SonarCsharp => "sonaranalyzer-csharp",
        }
    }

    /// The files `collect` writes to the tool's directory.
    pub(crate) fn files(self) -> &'static [File] {
        match self {
            Self::EslintSonarjs => ESLINT_SONARJS_FILES,
            Self::SonarCsharp => SONAR_CSHARP_FILES,
        }
    }

    /// The pinned packages; all of them must be installed.
    pub(crate) fn pins(self) -> &'static [Pin] {
        match self {
            Self::EslintSonarjs => ESLINT_SONARJS,
            Self::SonarCsharp => SONAR_CSHARP,
        }
    }

    /// The command, run in the repository root, that installs the pinned
    /// packages into the tool's directory.
    pub(crate) fn install(self) -> &'static str {
        match self {
            Self::EslintSonarjs => {
                "npm ci --ignore-scripts --prefix .sloptopop/tools/eslint-sonarjs"
            }
            Self::SonarCsharp => "dotnet restore .sloptopop/tools/sonaranalyzer-csharp/fetch.proj",
        }
    }

    /// Whether every pinned package is installed in `tools`, the
    /// [`TOOLS_DIR`] of the root.
    pub(crate) fn installed(self, tools: &Path) -> bool {
        let dir = tools.join(self.dir());
        self.pins().iter().all(|pin| pin.installed(&dir))
    }

    /// Writes the tool's files to its directory in `tools`, replacing
    /// older copies and leaving everything else (the installed packages)
    /// in place.
    pub(crate) fn write(self, tools: &Path) -> io::Result<()> {
        let dir = tools.join(self.dir());
        fs::create_dir_all(&dir)?;
        self.files()
            .iter()
            .try_for_each(|file| fs::write(dir.join(file.name), file.text))
    }
}

impl Pin {
    /// Whether this exact package is installed in the tool directory
    /// `dir`. npm checked the archive against the lockfile's hash when it
    /// installed it; `NuGet` records the archive's `SHA-512`, which must be
    /// the pinned one.
    fn installed(&self, dir: &Path) -> bool {
        match self.registry {
            Registry::Npm => version_of(
                &dir.join("node_modules")
                    .join(self.package)
                    .join("package.json"),
            )
            .is_some_and(|version| version == self.version),
            Registry::Nuget => {
                let id = self.package.to_ascii_lowercase();
                let file = dir
                    .join("packages")
                    .join(&id)
                    .join(self.version)
                    .join(format!("{id}.{}.nupkg.sha512", self.version));
                fs::read_to_string(file)
                    .is_ok_and(|hash| Some(hash.trim()) == self.integrity.strip_prefix("sha512-"))
            }
        }
    }
}

/// The `version` field of the npm manifest `file`, if it has one.
fn version_of(file: &Path) -> Option<String> {
    let manifest: Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
    manifest.get("version")?.as_str().map(str::to_owned)
}

/// Whether `ESLint` `version` can run eslint-plugin-sonarjs 2.0.4: 9.0 to
/// 9.14. From 9.15 on, core rules take their defaults from
/// `meta.defaultOptions`, which the plugin's typescript-eslint 7 wrappers
/// do not pass on, and linting stops with a crash.
fn runs_the_plugin(version: &str) -> bool {
    version
        .strip_prefix("9.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|minor| minor.parse::<u32>().ok())
        .is_some_and(|minor| minor <= 14)
}

/// The `ESLint` command-line script the eslint-sonarjs entries run: the
/// project's own `ESLint` if its version can run the plugin, else the pinned
/// one in the tool directory. `root` is absolute, and so is the result.
pub(crate) fn eslint_script(root: &Path) -> PathBuf {
    let own = root.join("node_modules").join("eslint");
    let base = if version_of(&own.join("package.json")).is_some_and(|v| runs_the_plugin(&v)) {
        own
    } else {
        root.join(TOOLS_DIR)
            .join(Managed::EslintSonarjs.dir())
            .join("node_modules")
            .join("eslint")
    };
    base.join("bin").join("eslint.js")
}

#[cfg(test)]
mod tests;
