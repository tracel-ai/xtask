use std::process::Command;

use anyhow::Ok;
use regex::Regex;

use crate::{endgroup, group, utils::process::run_process};

/// Ensure that a cargo crate is installed
pub fn ensure_cargo_crate_is_installed(
    crate_name: &str,
    features: Option<&str>,
    version: Option<&str>,
    locked: bool,
) -> anyhow::Result<()> {
    if !is_cargo_crate_installed(crate_name) {
        group!("Cargo: install crate '{}'", crate_name);
        let mut args = vec!["install", crate_name];
        if locked {
            args.push("--locked");
        }
        if let Some(features) = features {
            if !features.is_empty() {
                args.extend(vec!["features", features]);
            }
        }
        if let Some(version) = version {
            args.extend(vec!["--version", version]);
        }
        run_process(
            "cargo",
            &args,
            None,
            None,
            &format!("crate '{}' should be installed", crate_name),
        )?;
        endgroup!();
    }
    Ok(())
}

/// Returns true if the passed cargo crate is installed locally
pub fn is_cargo_crate_installed(crate_name: &str) -> bool {
    let output = Command::new("cargo")
        .arg("install")
        .arg("--list")
        .output()
        .expect("Should get the list of installed cargo commands");
    let output_str = String::from_utf8_lossy(&output.stdout);
    output_str.lines().any(|line| line.contains(crate_name))
}

pub fn parse_cargo_search_output(output: &str) -> Option<(&str, &str)> {
    let re = Regex::new(r#"(?P<name>[a-zA-Z0-9_-]+)\s*=\s*"(?P<version>\d+\.\d+\.\d+)""#)
        .expect("should compile regex");
    if let Some(captures) = re.captures(output) {
        if let (Some(name), Some(version)) = (captures.name("name"), captures.name("version")) {
            return Some((name.as_str(), version.as_str()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::valid_input("tracel-xtask-macros = \"1.0.1\"", Some(("tracel-xtask-macros", "1.0.1")))]
    #[case::missing_version("tracel-xtask-macros =", None)]
    #[case::invalid_format("tracel-xtask-macros: \"1.0.1\"", None)]
    #[case::extra_whitespace("   tracel-xtask-macros    =    \"1.0.1\"  ", Some(("tracel-xtask-macros", "1.0.1")))]
    #[case::no_quotes("tracel-xtask-macros = 1.0.1", None)]
    #[case::wrong_version_format("tracel-xtask-macros = \"1.0\"", None)]
    fn test_parse_cargo_search_output(#[case] input: &str, #[case] expected: Option<(&str, &str)>) {
        let result = parse_cargo_search_output(input);
        assert_eq!(result, expected);
    }
}
