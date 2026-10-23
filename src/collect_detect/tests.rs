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
        ("App.sln", Ecosystem::Dotnet),
        ("App.slnx", Ecosystem::Dotnet),
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
        "sln",
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

#[test]
fn shell_scripts_are_found_anywhere_below_the_root() {
    let tree = |files: &[(&str, &str)]| {
        let root = TempDir::new();
        for (path, text) in files {
            let path = root.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        detect(&root.0).unwrap().contains(&Ecosystem::Shell)
    };
    for (files, shell) in [
        (&[("a/b/run.sh", "")][..], true),
        (&[("a/b/run.bash", "")][..], true),
        (
            &[("a/install", "#!/usr/bin/env -S bash -e\necho\n")][..],
            true,
        ),
        (
            &[
                ("a/x.py", "#!/usr/bin/env python3\n"),
                ("b/c/notes", "#!/bin/sh"),
            ][..],
            true,
        ),
        // The wrapper's skipped directories, other files and an empty one.
        (&[("node_modules/x/run.sh", "")][..], false),
        (&[(".git/hooks/pre-commit", "#!/bin/sh\n")][..], false),
        (&[(".sloptopop/reports/a.sh", "")][..], false),
        (
            &[
                ("a/run.SH", ""),
                ("a/notes", "echo #!/bin/sh\n"),
                ("a/empty", ""),
            ][..],
            false,
        ),
    ] {
        assert_eq!(tree(files), shell, "{files:?}");
    }
    // A long first line is cut at 256 bytes, and only the first line counts.
    let long = format!("#!/bin/sh {}\n", "x".repeat(300));
    assert!(tree(&[("a/long", &long)]));
    assert!(!tree(&[("a/second", "\n#!/bin/sh\n")]));
}

#[cfg(unix)]
#[test]
fn links_are_not_followed_to_shell_scripts() {
    let root = TempDir::new();
    let elsewhere = TempDir::new();
    fs::write(elsewhere.0.join("run.sh"), "").unwrap();
    // (A root file named *.sh counts by its name, as every manifest does.)
    fs::create_dir(root.0.join("a")).unwrap();
    std::os::unix::fs::symlink(&elsewhere.0, root.0.join("a/dir")).unwrap();
    std::os::unix::fs::symlink(elsewhere.0.join("run.sh"), root.0.join("a/run.sh")).unwrap();
    assert!(!detect(&root.0).unwrap().contains(&Ecosystem::Shell));
}

#[test]
fn shebangs_of_bourne_shells_by_path_or_through_env() {
    for (line, shell) in [
        ("#!/bin/sh", true),
        ("#! /bin/bash -eu", true),
        ("#!/usr/bin/dash", true),
        ("#!/bin/ksh\t-x", true),
        ("#!/bin/ash", true),
        ("#!/bin/busybox sh", true),
        ("#!/usr/bin/env bash", true),
        ("#!/usr/bin/env  -S  sh -e", true),
        ("#!/usr/bin/env busybox sh", true),
        ("#!/bin/busybox ash", false),
        ("#!/bin/busybox", false),
        ("#!/usr/bin/env busybox ash", false),
        ("#!/usr/bin/env perl sh", false),
        ("#!/usr/bin/env -Sbash", false),
        ("#!/usr/bin/env", false),
        ("#!/usr/bin/env -S", false),
        ("#!/bin/shell", false),
        ("#!/bin/zsh", false),
        ("#!sh", false),
        ("#!", false),
        ("# /bin/sh", false),
        ("#!/bin/bash\r", false),
    ] {
        assert_eq!(is_shell_shebang(line.as_bytes()), shell, "{line:?}");
    }
}
