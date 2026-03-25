use std::process::{Command, Stdio};

use crate::{endgroup, group, utils::process::run_process};

/// Add a Rust target
pub fn rustup_add_target(target: &str) -> anyhow::Result<()> {
    group!("Rustup: add target {}", target);
    run_process(
        "rustup",
        &["target", "add", target],
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
        &["component", "add", component],
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

/// Returns true if the current toolchain is nightly.
pub fn is_current_toolchain_nightly() -> bool {
    if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
        let toolchain = toolchain.trim();
        if toolchain == "nightly" || toolchain.starts_with("nightly-") {
            return true;
        }
    }

    let output = Command::new("rustup")
        .args(["show", "active-toolchain"])
        .output()
        .expect("should get the active Rust toolchain");

    if !output.status.success() {
        return false;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let active_toolchain = output_str.trim();

    active_toolchain == "nightly" || active_toolchain.starts_with("nightly-")
}
