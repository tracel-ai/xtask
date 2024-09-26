use std::{env, process::Command, str};

use anyhow::{anyhow, Ok};

use crate::{
    endgroup, group,
    utils::{cargo::parse_cargo_search_output, process::run_process},
};

// Crates.io API token
const CRATES_IO_API_TOKEN: &str = "CRATES_IO_API_TOKEN";

#[tracel_xtask_macros::declare_command_args(None, None)]
pub struct PublishCmdArgs {
    /// The name of the crate to publish on crates.io
    name: String,
}

pub fn handle_command(args: PublishCmdArgs) -> anyhow::Result<()> {
    let crate_name = args.name;

    group!("Publishing crate '{}'...", &crate_name);
    // Retrieve local version for crate
    let local_version = local_version(&crate_name)?;
    info!("Local version: {local_version}");
    // Retrieve remote version for crate if it exists
    match remote_version(&crate_name)? {
        Some(remote_version) => {
            info!("Found remote version: {remote_version}");
            // Early return if we don't need to publish the crate
            if local_version == remote_version {
                info!("Remote version is up to date, skipping publishing!");
                return Ok(());
            }
        }
        None => info!("This is the first version to be published on crates.io!"),
    }
    // Publish the crate
    publish(crate_name)?;
    endgroup!();

    Ok(())
}

// Obtain local crate version
fn local_version(crate_name: &str) -> anyhow::Result<String> {
    // Obtain local crate version contained in cargo pkgid data
    let cargo_pkgid_output = Command::new("cargo")
        .args(["pkgid", "-p", crate_name])
        .output()
        .map_err(|e| anyhow!("Failed to execute cargo pkgid: {}", e))?;
    // Convert cargo pkgid output into a str
    let cargo_pkgid_str = str::from_utf8(&cargo_pkgid_output.stdout)
        .expect("Failed to convert pkgid output into a str");
    // Extract only the local crate version from str
    let (_, local_version) = cargo_pkgid_str
        .split_once('#')
        .expect("Failed to get local crate version");
    Ok(local_version.trim_end().to_string())
}

// Obtain the crate version from crates.io
fn remote_version(crate_name: &str) -> anyhow::Result<Option<String>> {
    // Obtain remote crate version contained in cargo search data
    let cargo_search_output = Command::new("cargo")
        .args(["search", crate_name, "--limit", "1"])
        .output()
        .map_err(|e| anyhow!("Failed to execute cargo search: {}", e))?;
    // Cargo search returns an empty string in case of a crate not present on crates.io
    if !cargo_search_output.stdout.is_empty() {
        let output_str = str::from_utf8(&cargo_search_output.stdout).unwrap();
        // Convert cargo search output into a str
        // as cargo search does not support exact match only we need to make sure that the
        // result returned by cargo search is indeed the crate that we are looking for and not
        // a crate whose name contains the name of the crate we are looking for.
        if let Some((name, version)) = parse_cargo_search_output(output_str) {
            if name == crate_name {
                return Ok(Some(version.to_string()));
            }
        }
    }
    Ok(None)
}

fn publish(crate_name: String) -> anyhow::Result<()> {
    // Perform dry-run to ensure everything is good for publishing
    run_process(
        "cargo",
        &["publish", "-p", &crate_name, "--dry-run"],
        None,
        None,
        &format!("Publish dry run failed for crate '{}'.", &crate_name),
    )?;

    let crates_io_token =
        env::var(CRATES_IO_API_TOKEN).expect("Failed to retrieve the crates.io API token");
    // Actually publish the crate
    let status = Command::new("cargo")
        .env("CRATES_IO_API_TOKEN", crates_io_token.clone())
        .args(["publish", "-p", &crate_name, "--token", &crates_io_token])
        .status()
        .map_err(|e| anyhow!("Failed to execute cargo publish: {}", e))?;
    if !status.success() {
        return Err(anyhow!("Publish failed for crate '{}'.", &crate_name));
    }
    Ok(())
}
