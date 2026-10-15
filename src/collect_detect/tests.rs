//! Unit tests for [`super`].

use std::fs;

use super::*;
use crate::tests::support::TempDir;

#[test]
fn every_manifest_maps_to_its_ecosystem() {
    for (file, ecosystem) in [
        ("Cargo.toml", Ecosystem::Rust),
        ("package.json", Ecosystem::Node),
        ("pyproject.toml", Ecosystem::Python),
        ("requirements.txt", Ecosystem::Python),
        ("requirements-dev.txt", Ecosystem::Python),
        ("go.mod", Ecosystem::Go),
        ("App.csproj", Ecosystem::Dotnet),
        ("pom.xml", Ecosystem::Jvm),
        ("build.gradle", Ecosystem::Jvm),
        ("build.gradle.kts", Ecosystem::Jvm),
        ("composer.json", Ecosystem::Php),
        ("Package.swift", Ecosystem::Swift),
        ("CMakeLists.txt", Ecosystem::Cpp),
        ("Module.psd1", Ecosystem::Powershell),
        ("install.sh", Ecosystem::Shell),
        ("BUILD.SH", Ecosystem::Shell),
    ] {
        assert_eq!(Ecosystem::of_file(file), Some(ecosystem), "{file}");
    }
    for file in [
        "README.md",
        "requirements.in",
        "my-requirements.txt",
        "Cargo.lock",
        "csproj",
        "sh",
    ] {
        assert_eq!(Ecosystem::of_file(file), None, "{file}");
    }
}

#[test]
fn names_are_lowercase_and_follow_the_order() {
    let all = [
        (Ecosystem::Cpp, "cpp"),
        (Ecosystem::Dotnet, "dotnet"),
        (Ecosystem::Go, "go"),
        (Ecosystem::Jvm, "jvm"),
        (Ecosystem::Node, "node"),
        (Ecosystem::Php, "php"),
        (Ecosystem::Powershell, "powershell"),
        (Ecosystem::Python, "python"),
        (Ecosystem::Rust, "rust"),
        (Ecosystem::Shell, "shell"),
        (Ecosystem::Swift, "swift"),
    ];
    for (ecosystem, name) in all {
        assert_eq!(ecosystem.name(), name, "{ecosystem:?}");
    }
    assert!(
        all.windows(2)
            .all(|pair| pair[0].1 < pair[1].1 && pair[0].0 < pair[1].0),
        "declared in name order"
    );
}

#[test]
fn detection_reads_only_root_files() {
    let root = TempDir::new();
    fs::write(root.0.join("Cargo.toml"), "").unwrap();
    fs::write(root.0.join("requirements.txt"), "").unwrap();
    fs::write(root.0.join("notes.md"), "").unwrap();
    fs::create_dir(root.0.join("go.mod")).unwrap();
    fs::create_dir_all(root.0.join("web")).unwrap();
    fs::write(root.0.join("web/package.json"), "").unwrap();
    assert_eq!(
        detect(&root.0).unwrap(),
        BTreeSet::from([Ecosystem::Python, Ecosystem::Rust]),
        "a directory named like a manifest and nested manifests are ignored"
    );
    assert!(detect(&root.0.join("missing")).is_err(), "missing root");
}
