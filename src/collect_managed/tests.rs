//! Unit tests for [`super`]: the pins agree with the manifests built into
//! the binary, and an installation counts only with the pinned versions.

use serde_json::Value;

use super::*;
use crate::tests::support::TempDir;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn file(managed: Managed, name: &str) -> &'static str {
    managed
        .files()
        .iter()
        .find(|file| file.name == name)
        .unwrap()
        .text
}

fn npm_pins() -> impl Iterator<Item = &'static Pin> {
    Managed::EslintSonarjs.pins().iter()
}

/// Installs `version` of the npm `package` in the eslint-sonarjs
/// directory of `tools`.
fn install_npm(tools: &Path, package: &str, version: &str) {
    let dir = tools.join("eslint-sonarjs/node_modules").join(package);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("package.json"),
        format!(r#"{{"name":"{package}","version":"{version}"}}"#),
    )
    .unwrap();
}

/// Writes `hash` as the recorded `SHA-512` of the pinned `NuGet` package.
fn install_nuget(tools: &Path, hash: &str) {
    let dir = tools.join("sonaranalyzer-csharp/packages/sonaranalyzer.csharp/9.32.0.97167");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("sonaranalyzer.csharp.9.32.0.97167.nupkg.sha512"),
        hash,
    )
    .unwrap();
}

#[test]
fn the_package_json_depends_on_exactly_the_npm_pins() {
    let manifest = json(file(Managed::EslintSonarjs, "package.json"));
    let dependencies = manifest["dependencies"].as_object().unwrap();
    assert_eq!(dependencies.len(), npm_pins().count(), "{dependencies:?}");
    for pin in npm_pins() {
        assert_eq!(pin.registry, Registry::Npm, "{}", pin.package);
        assert_eq!(
            dependencies[pin.package], pin.version,
            "{}: an exact version",
            pin.package
        );
    }
    assert_eq!(manifest["private"], true, "never published");
}

#[test]
fn the_lockfile_pins_every_package_by_hash_from_the_registry() {
    let lock = json(file(Managed::EslintSonarjs, "package-lock.json"));
    assert_eq!(lock["lockfileVersion"], 3, "lockfile version");
    let packages = lock["packages"].as_object().unwrap();
    for pin in npm_pins() {
        let entry = &packages[&format!("node_modules/{}", pin.package)];
        assert_eq!(entry["version"], pin.version, "{}", pin.package);
        assert_eq!(entry["integrity"], pin.integrity, "{}", pin.package);
    }
    for (path, entry) in packages.iter().filter(|(path, _)| !path.is_empty()) {
        assert!(
            entry["integrity"]
                .as_str()
                .is_some_and(|hash| hash.starts_with("sha512-")),
            "{path}: SHA-512 integrity"
        );
        assert!(
            entry["resolved"]
                .as_str()
                .is_some_and(|url| url.starts_with("https://registry.npmjs.org/")),
            "{path}: from the npm registry"
        );
        assert!(
            entry.get("hasInstallScript").is_none(),
            "{path}: no install script"
        );
    }
    let eslints: Vec<_> = packages
        .keys()
        .filter(|path| path.ends_with("node_modules/eslint"))
        .collect();
    assert_eq!(eslints, ["node_modules/eslint"], "one ESLint");
}

#[test]
fn the_nuget_files_name_the_pinned_package() {
    let [pin] = Managed::SonarCsharp.pins() else {
        panic!("one NuGet pin");
    };
    assert_eq!(pin.registry, Registry::Nuget, "registry");
    let fetch = file(Managed::SonarCsharp, "fetch.proj");
    assert!(
        fetch.contains(&format!(
            r#"<PackageDownload Include="{}" Version="[{}]" />"#,
            pin.package, pin.version
        )),
        "fetch.proj downloads the exact version"
    );
    let props = file(Managed::SonarCsharp, "Directory.Build.props");
    assert!(
        props.contains(&format!(
            "packages/{}/{}/analyzers/*.dll",
            pin.package.to_ascii_lowercase(),
            pin.version
        )),
        "the props use the downloaded analyzers"
    );
    assert!(
        props.contains("GetPathOfFileAbove('Directory.Build.props'")
            && props.contains(",version=2.1</ErrorLog>"),
        "the repository's own props are kept; SARIF v2.1"
    );
}

#[test]
fn every_pin_records_its_licence_files() {
    for pin in npm_pins().chain(Managed::SonarCsharp.pins()) {
        assert!(
            !pin.licence_files.is_empty()
                && pin
                    .licence_files
                    .iter()
                    .all(
                        |file| file.starts_with("https://") && file.contains(pin.version)
                            || file.contains("/blob/")
                    ),
            "{}: {:?}",
            pin.package,
            pin.licence_files
        );
        assert!(
            pin.integrity.starts_with("sha512-") && pin.integrity.ends_with("=="),
            "{}",
            pin.package
        );
    }
}

#[test]
fn directories_and_install_commands() {
    assert_eq!(Managed::EslintSonarjs.dir(), "eslint-sonarjs", "npm");
    assert_eq!(Managed::SonarCsharp.dir(), "sonaranalyzer-csharp", "NuGet");
    assert_eq!(
        Managed::EslintSonarjs.install(),
        "npm ci --ignore-scripts --prefix .sloptopop/tools/eslint-sonarjs",
        "npm"
    );
    assert_eq!(
        Managed::SonarCsharp.install(),
        "dotnet restore .sloptopop/tools/sonaranalyzer-csharp/fetch.proj",
        "NuGet"
    );
}

#[test]
fn npm_tools_count_as_installed_only_at_the_pinned_versions() {
    let root = TempDir::new();
    let tools = root.0.join(TOOLS_DIR);
    assert!(
        !Managed::EslintSonarjs.installed(&tools),
        "nothing installed"
    );
    for pin in npm_pins() {
        install_npm(&tools, pin.package, pin.version);
    }
    assert!(Managed::EslintSonarjs.installed(&tools), "all pinned");
    install_npm(&tools, "eslint-plugin-sonarjs", "3.0.0");
    assert!(!Managed::EslintSonarjs.installed(&tools), "another version");
    fs::write(
        tools.join("eslint-sonarjs/node_modules/eslint-plugin-sonarjs/package.json"),
        r#"{"version":2}"#,
    )
    .unwrap();
    assert!(!Managed::EslintSonarjs.installed(&tools), "not a string");
    fs::write(
        tools.join("eslint-sonarjs/node_modules/eslint-plugin-sonarjs/package.json"),
        r#"{"name":"eslint-plugin-sonarjs"}"#,
    )
    .unwrap();
    assert!(!Managed::EslintSonarjs.installed(&tools), "no version");
    fs::write(
        tools.join("eslint-sonarjs/node_modules/eslint-plugin-sonarjs/package.json"),
        "not json",
    )
    .unwrap();
    assert!(!Managed::EslintSonarjs.installed(&tools), "not JSON");
}

#[test]
fn nuget_tools_count_as_installed_only_with_the_pinned_hash() {
    let root = TempDir::new();
    let tools = root.0.join(TOOLS_DIR);
    assert!(!Managed::SonarCsharp.installed(&tools), "nothing installed");
    let pinned = Managed::SonarCsharp.pins()[0]
        .integrity
        .strip_prefix("sha512-")
        .unwrap();
    install_nuget(&tools, &format!("{pinned}\r\n"));
    assert!(Managed::SonarCsharp.installed(&tools), "pinned hash");
    install_nuget(&tools, &pinned.replace('N', "M"));
    assert!(!Managed::SonarCsharp.installed(&tools), "another archive");
    install_nuget(&tools, Managed::SonarCsharp.pins()[0].integrity);
    assert!(
        !Managed::SonarCsharp.installed(&tools),
        "the prefix is not part of the hash"
    );
}

#[test]
fn writing_replaces_the_files_and_keeps_the_installation() {
    let root = TempDir::new();
    let tools = root.0.join(TOOLS_DIR);
    install_npm(&tools, "eslint", "9.14.0");
    let dir = tools.join("eslint-sonarjs");
    fs::write(dir.join("package.json"), "old").unwrap();
    Managed::EslintSonarjs.write(&tools).unwrap();
    for file in Managed::EslintSonarjs.files() {
        assert_eq!(
            fs::read_to_string(dir.join(file.name)).unwrap(),
            file.text,
            "{}",
            file.name
        );
    }
    assert!(
        dir.join("node_modules/eslint/package.json").is_file(),
        "installation kept"
    );
    Managed::SonarCsharp.write(&tools).unwrap();
    assert!(
        tools.join("sonaranalyzer-csharp/fetch.proj").is_file(),
        "NuGet files"
    );
    let blocked = TempDir::new();
    fs::write(blocked.0.join("tools"), "").unwrap();
    assert!(
        Managed::SonarCsharp
            .write(&blocked.0.join("tools"))
            .is_err(),
        "a file in the way"
    );
    fs::create_dir_all(blocked.0.join("t/sonaranalyzer-csharp/fetch.proj")).unwrap();
    assert!(
        Managed::SonarCsharp.write(&blocked.0.join("t")).is_err(),
        "a directory in the way"
    );
}

#[test]
fn only_eslint_9_0_to_9_14_runs_the_plugin() {
    for version in ["9.0.0", "9.9.1", "9.14.0", "9.14.9-rc.1"] {
        assert!(runs_the_plugin(version), "{version}");
    }
    for version in [
        "9.15.0", "9.39.5", "8.57.0", "10.0.0", "19.1.0", "9.x", "9", "",
    ] {
        assert!(!runs_the_plugin(version), "{version}");
    }
}

#[test]
fn the_projects_own_eslint_is_used_when_it_runs_the_plugin() {
    let root = TempDir::new();
    let managed = root
        .0
        .join(".sloptopop/tools/eslint-sonarjs/node_modules/eslint/bin/eslint.js");
    assert_eq!(eslint_script(&root.0), managed, "no ESLint of its own");
    let own = root.0.join("node_modules/eslint");
    fs::create_dir_all(&own).unwrap();
    fs::write(own.join("package.json"), r#"{"version":"9.10.0"}"#).unwrap();
    assert_eq!(
        eslint_script(&root.0),
        own.join("bin/eslint.js"),
        "the project's ESLint 9.10"
    );
    fs::write(own.join("package.json"), r#"{"version":"9.39.5"}"#).unwrap();
    assert_eq!(eslint_script(&root.0), managed, "ESLint 9.39 cannot");
}
