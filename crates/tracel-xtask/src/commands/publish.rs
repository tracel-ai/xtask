use anyhow::{Context as _, anyhow};
use regex::Regex;
use std::{env, fs, path::Path, process::Command, str};

use crate::{
    endgroup, group,
    prelude::{Context, Environment},
    utils::{cargo::parse_cargo_search_output, process::run_process},
};

// Crates.io API token
const CRATES_IO_API_TOKEN: &str = "CRATES_IO_API_TOKEN";

#[tracel_xtask_macros::declare_command_args(None, None)]
pub struct PublishCmdArgs {
    /// The name of the crate to publish on crates.io
    name: String,
    /// When set, only perform a dry-run and does not publish the crate
    #[arg(long)]
    dry_run_only: bool,
    /// Optional path to the Cargo.toml to validate the version against the tag
    #[arg(long)]
    cargo_toml: Option<std::path::PathBuf>,
    /// Enable validation by comparing Git tag version with Cargo.toml version
    #[arg(short = 'V', long)]
    validate_tag_version: bool,
    /// Optional git tag to validate
    #[arg(long)]
    tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionValidation {
    pub cargo_toml_version: String,
    pub git_tag_version: String,
    pub matches: bool,
}

pub fn handle_command(
    args: PublishCmdArgs,
    _env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    let crate_name = args.name;
    // version validation against git tag if asked for
    if args.validate_tag_version {
        ensure_version_matches(args.cargo_toml.as_deref(), &crate_name, args.tag.as_deref())?;
    }
    // publish
    group!("Publishing crate '{}'...", &crate_name);
    let local_version = local_version(&crate_name)?;
    info!("Local version: {local_version}");
    match remote_version(&crate_name)? {
        Some(remote_version) => {
            info!("Found remote version: {remote_version}");
            if local_version == remote_version {
                info!("Remote version is up to date, skipping publishing!");
                endgroup!();
                return Ok(());
            }
        }
        None => info!("This is the first version to be published on crates.io!"),
    }
    publish(crate_name, args.dry_run_only)?;
    endgroup!();

    Ok(())
}

//  Version Validation =======================================================

pub fn validate_version(
    cargo_toml: Option<&Path>,
    crate_name: &str,
    tag: Option<&str>,
) -> anyhow::Result<VersionValidation> {
    let git_tag_version = resolve_git_tag_version(tag)?;
    let cargo_toml_version = match cargo_toml {
        Some(path) => version_from_cargo_toml(path)?,
        None => local_version(crate_name)?,
    };

    Ok(VersionValidation {
        matches: git_tag_version == cargo_toml_version,
        cargo_toml_version,
        git_tag_version,
    })
}

/// Return an error if the version in Cargo.toml is not the same of the git tag
pub fn ensure_version_matches(
    cargo_toml: Option<&Path>,
    crate_name: &str,
    tag: Option<&str>,
) -> anyhow::Result<VersionValidation> {
    group!("Validating Git tag vs Cargo.toml version...");
    let vv = validate_version(cargo_toml, crate_name, tag)?;
    info!("Git tag version: {}", vv.git_tag_version);
    info!("Cargo.toml version: {}", vv.cargo_toml_version);

    if !vv.matches {
        endgroup!();
        return Err(anyhow!(
            "Git tag version ({}) does not match Cargo.toml version ({}).",
            vv.git_tag_version,
            vv.cargo_toml_version
        ));
    }

    info!("Versions match!");
    endgroup!();
    Ok(vv)
}

fn resolve_git_tag_version(opt_tag: Option<&str>) -> anyhow::Result<String> {
    let raw = if let Some(t) = opt_tag {
        t.to_string()
    } else if let Ok(t) = env::var("INPUT_TAG") {
        t
    } else if let Ok(t) = env::var("REF_NAME") {
        t
    } else {
        return Err(anyhow!(
            "No Git tag provided. Pass --tag or set INPUT_TAG/REF_NAME."
        ));
    };
    Ok(strip_leading_v(&raw))
}

/// Turn "v1.2.3" or "V1.2.3-alpha.1" into "1.2.3" or "1.2.3-alpha.1"
fn strip_leading_v(s: &str) -> String {
    s.strip_prefix('v')
        .or_else(|| s.strip_prefix('V'))
        .unwrap_or(s)
        .to_string()
}

/// Read version from the passed Cargo.toml file
fn version_from_cargo_toml(path: &Path) -> anyhow::Result<String> {
    if !path.exists() {
        return Err(anyhow!("Cargo.toml should exist at {}", path.display()));
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Reading Cargo.toml should succeed at {}", path.display()))?;

    // Disallow '+' build metadata. Allow only optional pre-release after '-'.
    let re = Regex::new(r#"(?m)^\s*version\s*=\s*"(?P<v>\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)"\s*$"#)
        .expect("regex should compile");
    let caps = re.captures(&content).ok_or_else(|| {
        anyhow!(
            "Cargo.toml should contain a valid version at {}",
            path.display()
        )
    })?;
    Ok(caps
        .name("v")
        .expect("version capture should exist")
        .as_str()
        .to_string())
}

//  Publish ==================================================================

fn local_version(crate_name: &str) -> anyhow::Result<String> {
    let cargo_pkgid_output = Command::new("cargo")
        .args(["pkgid", "-p", crate_name])
        .output()
        .map_err(|e| anyhow!("Executing `cargo pkgid` should succeed: {}", e))?;
    let cargo_pkgid_str = str::from_utf8(&cargo_pkgid_output.stdout)
        .expect("cargo pkgid output should be valid UTF-8");
    let (_, local_version) = cargo_pkgid_str
        .split_once('#')
        .expect("pkgid output should contain a version after '#'");
    Ok(local_version.trim_end().to_string())
}

fn remote_version(crate_name: &str) -> anyhow::Result<Option<String>> {
    let cargo_search_output = Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .output()
        .map_err(|e| anyhow!("Executing `cargo search` should succeed: {}", e))?;
    if !cargo_search_output.stdout.is_empty() {
        let output_str = str::from_utf8(&cargo_search_output.stdout).unwrap();
        if let Some((name, version)) = parse_cargo_search_output(output_str) {
            if name == crate_name {
                return Ok(Some(version.to_string()));
            }
        }
    }
    Ok(None)
}

fn publish(crate_name: String, dry_run_only: bool) -> anyhow::Result<()> {
    run_process(
        "cargo",
        &["publish", "-p", &crate_name, "--dry-run"],
        None,
        None,
        &format!(
            "Publish dry run should succeed for crate '{}'.",
            &crate_name
        ),
    )?;

    if dry_run_only {
        return Ok(());
    }

    let crates_io_token = env::var(CRATES_IO_API_TOKEN).expect("CRATES_IO_API_TOKEN should be set");
    let status = Command::new("cargo")
        .env("CRATES_IO_API_TOKEN", crates_io_token.clone())
        .args(["publish", "-p", &crate_name, "--token", &crates_io_token])
        .status()
        .map_err(|e| anyhow!("Executing `cargo publish` should succeed: {}", e))?;
    if !status.success() {
        return Err(anyhow!(
            "Publish should succeed for crate '{}'.",
            &crate_name
        ));
    }
    Ok(())
}

//  Tests ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::fs::File;
    use std::io::Write as _;

    #[rstest]
    #[case("v1.2.3", "1.2.3")]
    #[case("V2.0.0-alpha.2", "2.0.0-alpha.2")]
    fn strip_leading_v_cases(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(strip_leading_v(input), expected);
    }

    #[test]
    fn version_from_cargo_toml_pre_release_only() {
        let dir = tempfile::tempdir().expect("tempdir should be created");
        let path = dir.path().join("Cargo.toml");
        let mut f = File::create(&path).expect("file should be created");
        writeln!(
            f,
            r#"[package]
name = "dummy"
version = "1.2.3-rc.1-exp.sha.5114f85"
edition = "2021"
"#
        )
        .unwrap();
        let v = version_from_cargo_toml(&path).expect("version extraction should succeed");
        assert_eq!(v, "1.2.3-rc.1-exp.sha.5114f85");
    }

    #[test]
    fn validate_version_with_cargo_toml_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cargo.toml");
        fs::write(
            &path,
            r#"[package]
name = "dummy"
version = "0.1.0-alpha.1"
"#,
        )
        .unwrap();

        let vv = validate_version(Some(&path), "dummy", Some("v0.1.0-alpha.1"))
            .expect("validation should succeed");
        assert_eq!(vv.cargo_toml_version, "0.1.0-alpha.1");
        assert_eq!(vv.git_tag_version, "0.1.0-alpha.1");
        assert!(vv.matches);
    }

    #[test]
    fn validate_version_with_cargo_toml_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Cargo.toml");
        fs::write(
            &path,
            r#"[package]
name = "dummy"
version = "1.2.3"
"#,
        )
        .unwrap();

        let vv = validate_version(Some(&path), "dummy", Some("v1.2.4"))
            .expect("validation should succeed");
        assert_eq!(vv.cargo_toml_version, "1.2.3");
        assert_eq!(vv.git_tag_version, "1.2.4");
        assert!(!vv.matches);
    }
}
