use std::process::{Command, Stdio};

use anyhow::{anyhow, Ok, Result};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType},
};

use super::Target;

#[derive(Args, Clone)]
pub struct TestCmdArgs {
    /// Target to test for.
    #[arg(short, long, value_enum, default_value_t = Target::Workspace)]
    target: Target,
    /// Comma-separated list of excluded crates.
    #[arg(
        short = 'x',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub exclude: Vec<String>,
    /// Comma-separated list of crates to include exclusively.
    #[arg(
        short = 'n',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub only: Vec<String>,
    #[command(subcommand)]
    pub command: TestCommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum TestCommand {
    /// Run unit tests.
    Unit,
    /// Run integration tests.
    Integration,
    /// Run documentation tests.
    Documentation,
    /// Run all the checks.
    All,
}

pub fn handle_command(args: TestCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    match args.command {
        TestCommand::Unit => run_unit(&args.target, &args.exclude, &args.only),
        TestCommand::Integration => run_integration(&args.target, &args.exclude, &args.only),
        TestCommand::Documentation => run_documentation(&args.target, &args.exclude, &args.only),
        TestCommand::All => TestCommand::iter()
            .filter(|c| *c != TestCommand::All)
            .try_for_each(|c| {
                handle_command(TestCmdArgs {
                    command: c,
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                })
            }),
    }
}

pub(crate) fn run_unit(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> Result<()> {
    match target {
        Target::Workspace => {
            let mut args = vec!["test", "--workspace", "--color", "always"];
            let excluded_crates = excluded.join(",");
            if !excluded.is_empty() {
                args.extend(["--exclude", &excluded_crates]);
            }
            group!("Workspace Unit Tests");
            info!("Command line: cargo {}", args.join(" "));
            let status = Command::new("cargo")
                .args(args)
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo test: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace unit test failed"));
            }
            endgroup!();
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                run_unit_test(&member)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_unit(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_unit_test(member: &WorkspaceMember) -> Result<(), anyhow::Error> {
    group!("Unit Tests: {}", member.name);
    info!("Command line: cargo test --lib --bins -p {}", &member.name);
    let error_output = Command::new("cargo")
        .args([
            "test",
            "--lib",
            "--bins",
            "-p",
            &member.name,
            "--color=always",
            "--",
            "--color=always",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| anyhow!("Failed to execute unit test: {}", e))?;

    let stderr = String::from_utf8_lossy(&error_output.stderr);
    if !error_output.status.success() {
        if stderr.contains("no library targets found") {
            warn!(
                "No library found to test for in the crate '{}'",
                &member.name
            );
            endgroup!();
            return Ok(());
        }
        return Err(anyhow!(
            "Failed to execute unit test for {}: {}",
            &member.name,
            stderr
        ));
    }
    endgroup!();
    Ok(())
}

pub(crate) fn run_documentation(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> Result<()> {
    match target {
        Target::Workspace => {
            let mut args = vec!["test", "--workspace", "--doc", "--color", "always"];
            let excluded_crates = excluded.join(",");
            if !excluded.is_empty() {
                args.extend(["--exclude", &excluded_crates]);
            }
            group!("Workspace Documentation Tests");
            info!("Command line: cargo {}", args.join(" "));
            let status = Command::new("cargo")
                .args(args)
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo test: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace documentation test failed"));
            }
            endgroup!();
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                run_doc_test(&member)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_documentation(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_doc_test(member: &WorkspaceMember) -> Result<(), anyhow::Error> {
    group!("Doc Tests: {}", member.name);
    info!("Command line: cargo test --doc -p {}", &member.name);
    let error_output = Command::new("cargo")
        .args(["test", "--doc", "-p", &member.name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| anyhow!("Failed to execute documentation test: {}", e))?;

    let stderr = String::from_utf8_lossy(&error_output.stderr);
    if !error_output.status.success() {
        if stderr.contains("no library targets found") {
            warn!(
                "No library found to test documentation for in the crate '{}'",
                &member.name
            );
            endgroup!();
            return Ok(());
        }
        return Err(anyhow!(
            "Failed to execute documentation test for {}: {}",
            &member.name,
            stderr
        ));
    }
    endgroup!();
    Ok(())
}

pub(crate) fn run_integration(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            let mut args = vec!["test", "--test", "test_*", "--color", "always"];
            let excluded_crates = excluded.join(",");
            if !excluded.is_empty() {
                args.extend(["--exclude", &excluded_crates]);
            }
            group!("Workspace Integration Tests");
            info!("Command line: cargo {}", args.join(" "));
            let status = Command::new("cargo")
                .args(args)
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo test: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace integration test failed"));
            }
            endgroup!();
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                run_integration_test(&member)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_integration(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_integration_test(member: &WorkspaceMember) -> Result<()> {
    group!("Integration Tests: {}", &member.name);
    info!(
        "Command line: cargo test --test \"test_*\" -p {} --color=always",
        &member.name
    );
    let output = Command::new("cargo")
        .args([
            "test",
            "--test",
            "test_*",
            "-p",
            &member.name,
            "--color",
            "always",
        ])
        .output()
        .map_err(|e| anyhow!("Failed to execute integration test: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("no test target matches pattern") {
            warn!(
                "No tests found matching the pattern `test_*` for {}",
                &member.name
            );
            endgroup!();
            return Ok(());
        }
        return Err(anyhow!(
            "Failed to execute integration test for {}: {}",
            &member.name,
            stderr
        ));
    }
    endgroup!();
    Ok(())
}
