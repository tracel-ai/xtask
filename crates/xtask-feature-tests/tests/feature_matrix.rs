use std::{fs, path::Path, process::Command};

const COMMAND_FEATURES: &[&str] = &[
    "aws-container",
    "aws-secrets",
    "build",
    "bump",
    "check",
    "clean",
    "compile",
    "coverage",
    "dependencies",
    "doc",
    "docker-compose",
    "fix",
    "gcp-container",
    "gcp-secrets",
    "host",
    "image",
    "infra",
    "publish",
    "test",
    "validate",
    "vulnerabilities",
];

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("fixture should live two directories below the workspace root")
}

#[test]
fn custom_only_consumer_compiles_without_base_features() {
    let status = Command::new("cargo")
        .current_dir(workspace_root())
        .args([
            "check",
            "--quiet",
            "--package",
            "xtask-feature-tests",
            "--no-default-features",
        ])
        .status()
        .expect("cargo check should start");
    assert!(status.success(), "custom-only consumer should compile");
}

#[test]
fn every_command_feature_compiles_independently() {
    for feature in COMMAND_FEATURES {
        let status = Command::new("cargo")
            .current_dir(workspace_root())
            .args([
                "check",
                "--quiet",
                "--package",
                "xtask-feature-tests",
                "--no-default-features",
                "--features",
                feature,
            ])
            .status()
            .expect("cargo check should start");
        assert!(status.success(), "feature `{feature}` should compile alone");
    }
}

fn help_for(feature: &str) -> String {
    let output = Command::new("cargo")
        .current_dir(workspace_root())
        .args([
            "run",
            "--quiet",
            "--package",
            "xtask-feature-tests",
            "--no-default-features",
            "--features",
            feature,
            "--",
            "--help",
        ])
        .output()
        .expect("fixture should run");
    assert!(output.status.success());
    String::from_utf8(output.stdout).expect("help output should be UTF-8")
}

fn dependency_names(feature: Option<&str>) -> Vec<String> {
    let mut command = Command::new("cargo");
    command.current_dir(workspace_root()).args([
        "tree",
        "--package",
        "xtask-feature-tests",
        "--no-default-features",
        "--edges",
        "normal",
        "--prefix",
        "none",
    ]);
    if let Some(feature) = feature {
        command.args(["--features", feature]);
    }
    let output = command.output().expect("cargo tree should start");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("dependency tree should be UTF-8")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

fn assert_dependencies_absent(feature: Option<&str>, dependencies: &[&str]) {
    let names = dependency_names(feature);
    for dependency in dependencies {
        assert!(
            !names.iter().any(|name| name == dependency),
            "feature {feature:?} should not compile `{dependency}`"
        );
    }
}

fn assert_fixture_fails(name: &str, source: &str, expected_error: &str) {
    let fixture = workspace_root()
        .join("target/feature-test-ui")
        .join(format!("{name}-{}", std::process::id()));
    fs::create_dir_all(fixture.join("src")).expect("UI fixture directory should be created");
    let dependency_path = workspace_root()
        .join("crates/tracel-xtask")
        .display()
        .to_string()
        .replace('\\', "\\\\");
    fs::write(
        fixture.join("Cargo.toml"),
        format!(
            r#"[workspace]

[package]
name = "{name}"
version = "0.0.0"
edition = "2024"

[dependencies]
tracel-xtask = {{ path = "{dependency_path}", default-features = false }}
"#,
        ),
    )
    .expect("UI fixture manifest should be written");
    fs::write(fixture.join("src/main.rs"), source).expect("UI fixture source should be written");

    let output = Command::new("cargo")
        .args([
            "check",
            "--offline",
            "--manifest-path",
            fixture.join("Cargo.toml").to_str().expect("UTF-8 path"),
        ])
        .output()
        .expect("UI fixture cargo check should start");
    assert!(!output.status.success(), "UI fixture should not compile");
    let stderr = String::from_utf8(output.stderr).expect("compiler output should be UTF-8");
    assert!(
        stderr.contains(expected_error),
        "compiler output should contain `{expected_error}`:\n{stderr}"
    );
}

#[test]
fn help_contains_only_selected_base_commands() {
    let build = help_for("build");
    assert!(build.contains("\n  build"));
    assert!(!build.contains("\n  test"));

    let validate = help_for("validate");
    assert!(validate.contains("\n  validate"));
    assert!(!validate.contains("\n  check"));
    assert!(!validate.contains("\n  test"));

    let all = help_for("all");
    for command in COMMAND_FEATURES {
        assert!(
            all.contains(&format!("\n  {command}")),
            "`all` should expose `{command}`"
        );
    }
}

#[test]
fn representative_features_keep_unrelated_dependencies_out() {
    assert_dependencies_absent(
        None,
        &[
            "inquire",
            "owo-colors",
            "rand",
            "serde",
            "serde_json",
            "time",
            "ureq",
            "zip",
        ],
    );
    assert_dependencies_absent(
        Some("build"),
        &["inquire", "owo-colors", "serde", "time", "ureq", "zip"],
    );
    assert_dependencies_absent(
        Some("aws-secrets"),
        &["inquire", "owo-colors", "serde", "time", "ureq", "zip"],
    );
    for feature in ["host", "gcp-container"] {
        assert_dependencies_absent(
            Some(feature),
            &["inquire", "owo-colors", "time", "ureq", "zip"],
        );
    }

    let infra = dependency_names(Some("infra"));
    for dependency in ["home", "toml", "ureq", "zip"] {
        assert!(
            infra.iter().any(|name| name == dependency),
            "infra should compile `{dependency}`"
        );
    }

    let all = dependency_names(Some("all"));
    for dependency in ["inquire", "owo-colors", "ureq", "zip"] {
        assert!(
            all.iter().any(|name| name == dependency),
            "all commands should compile `{dependency}`"
        );
    }
}

#[test]
fn invalid_base_command_declarations_have_actionable_errors() {
    assert_fixture_fails(
        "old-macro-arguments",
        r#"use tracel_xtask::prelude::macros;

#[macros::base_commands(Build)]
enum Command {}

fn main() {}
"#,
        "no longer accepts command arguments in v5",
    );
    assert_fixture_fails(
        "empty-command-enum",
        r#"use tracel_xtask::prelude::macros;

#[macros::base_commands]
enum Command {}

fn main() {}
"#,
        "cannot generate an empty command enum",
    );
}
