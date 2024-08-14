use std::process::{Command, Stdio};

use crate::{endgroup, group, utils::process::run_process};

/// Add a Rust target
pub fn rustup_add_target(target: &str) -> anyhow::Result<()> {
    group!("Rustup: add target {}", target);
    run_process(
        "rustup",
        &vec!["target", "add", target],
        None,
        None,
        &format!("Failed to add target {target}"),
    )?;
    endgroup!();
    Ok(())
}

/// Add a Rust component
pub fn rustup_add_component(component: &str) -> anyhow::Result<()> {
    group!("Rustup: add component {}", component);
    run_process(
        "rustup",
        &vec!["component", "add", component],
        None,
        None,
        &format!("Failed to add component {component}"),
    )?;
    endgroup!();
    Ok(())
}

// Returns the output of the rustup command to get the installed targets
pub fn rustup_get_installed_targets() -> String {
    let output = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .stdout(Stdio::piped())
        .output()
        .expect("Rustup command should execute successfully");
    String::from_utf8(output.stdout).expect("Output should be valid UTF-8")
}

/// Returns true if the current toolchain is the nightly
pub fn is_current_toolchain_nightly() -> bool {
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
