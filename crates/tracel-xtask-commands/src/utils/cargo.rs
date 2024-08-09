use std::process::Command;

use anyhow::Ok;

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
fn is_cargo_crate_installed(crate_name: &str) -> bool {
    let output = Command::new("cargo")
        .arg("install")
        .arg("--list")
        .output()
        .expect("Should get the list of installed cargo commands");
    let output_str = String::from_utf8_lossy(&output.stdout);
    output_str.lines().any(|line| line.contains(crate_name))
}

/// Returns true if the current toolchain is the nightly
pub(crate) fn is_current_toolchain_nightly() -> bool {
    let output = Command::new("rustup")
        .arg("show")
        .output()
        .expect("Should get the list of installed Rust toolchains");
    let output_str = String::from_utf8_lossy(&output.stdout);
    for line in output_str.lines() {
        // look for the "rustc.*-nightly" line
        if line.contains("rustc") && line.contains("-nightly") {
            return true;
        }
    }
    // assume we are using a stable toolchain if we did not find the nightly compiler
    false
}
