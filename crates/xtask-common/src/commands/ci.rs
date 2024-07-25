use std::process::Command;

use anyhow::{anyhow, Ok};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        workspace::{get_workspace_members, WorkspaceMemberType},
    }, versions::TYPOS_VERSION,
};

use super::{
    test::{run_documentation, run_integration, run_unit},
    Target,
};

#[derive(Args, Clone)]
pub struct CICmdArgs {
    /// Target to check for.
    #[arg(short, long, value_enum)]
    pub target: Target,
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
    pub command: CICommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum CICommand {
    /// Run all the checks.
    All,
    /// Run all tests.
    AllTests,
    /// Run audit command.
    Audit,
    /// Build the targets.
    Build,
    /// Run documentation tests.
    DocTests,
    /// Run format command.
    Format,
    /// Run integration tests.
    IntegrationTests,
    /// Run lint command.
    Lint,
    /// Report typos in source code.
    Typos,
    /// Run unit tests.
    UnitTests,
}

pub fn handle_command(args: CICmdArgs) -> anyhow::Result<()> {
    match args.command {
        CICommand::Audit => run_audit(&args.target),
        CICommand::Build => run_build(&args.target, &args.exclude, &args.only),
        CICommand::DocTests => run_doc_tests(&args.target, &args.exclude, &args.only),
        CICommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CICommand::IntegrationTests => {
            run_integration_tests(&args.target, &args.exclude, &args.only)
        }
        CICommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
        CICommand::Typos => run_typos(&args.target),
        CICommand::UnitTests => run_unit_tests(&args.target, &args.exclude, &args.only),
        CICommand::AllTests => run_all_tests(&args.target, &args.exclude, &args.only),
        CICommand::All => CICommand::iter()
            .filter(|c| *c != CICommand::All && *c != CICommand::AllTests)
            .try_for_each(|c| {
                handle_command(CICmdArgs {
                    command: c,
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                })
            }),
    }
}

fn run_audit(target: &Target) -> anyhow::Result<()> {
    match target {
        Target::Crates | Target::Examples => {
            group!("Audit: Crates and Examples");
            ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
            info!("Command line: cargo audit");
            let status = Command::new("cargo")
                .args(["audit", "-q", "--color", "always"])
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo audit: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Audit check execution failed"));
            }
            endgroup!();
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All && *t != Target::Examples)
                .try_for_each(|t| run_audit(&t))?;
        }
    }
    Ok(())
}

fn run_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> std::prelude::v1::Result<(), anyhow::Error> {
    match target {
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                group!("Build: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!("Command line: cargo build -p {}", &member.name);
                let status = Command::new("cargo")
                    .args(["build", "-p", &member.name])
                    .status()
                    .map_err(|e| anyhow!("Failed to execute cargo build: {}", e))?;
                if !status.success() {
                    return Err(anyhow!("Build failed for {}", &member.name));
                }
                endgroup!();
            }
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All)
                .try_for_each(|t| run_build(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_format(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> anyhow::Result<()> {
    match target {
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                group!("Format: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!("Command line: cargo fmt --check -p {}", &member.name);
                let status = Command::new("cargo")
                    .args(["fmt", "--check", "-p", &member.name])
                    .status()
                    .map_err(|e| anyhow!("Failed to execute cargo fmt: {}", e))?;
                if !status.success() {
                    return Err(anyhow!(
                        "Format check execution failed for {}",
                        &member.name
                    ));
                }
                endgroup!();
            }
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All)
                .try_for_each(|t| run_format(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_lint(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> anyhow::Result<()> {
    match target {
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                group!("Lint: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!(
                    "Command line: cargo clippy --no-deps -p {} -- --deny warnings",
                    &member.name
                );
                let status = Command::new("cargo")
                    .args([
                        "clippy",
                        "--no-deps",
                        "-p",
                        &member.name,
                        "--",
                        "--deny",
                        "warnings",
                    ])
                    .status()
                    .map_err(|e| anyhow!("Failed to execute cargo clippy: {}", e))?;
                if !status.success() {
                    return Err(anyhow!("Lint fix execution failed for {}", &member.name));
                }
                endgroup!();
            }
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All)
                .try_for_each(|t| run_lint(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_typos(target: &Target) -> anyhow::Result<()> {
    match target {
        Target::Crates | Target::Examples => {
            group!("Typos: Crates and Examples");
            ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
            info!("Command line: typos --diff");
            let status = Command::new("typos")
                .args(["--diff"])
                .status()
                .map_err(|e| anyhow!("Failed to execute typos: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Typos check execution failed"));
            }
            endgroup!();
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All && *t != Target::Examples)
                .try_for_each(|t| run_typos(&t))?;
        }
    }
    Ok(())
}

fn run_unit_tests(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    run_unit(target, excluded, only)
}

fn run_integration_tests(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    run_integration(target, excluded, only)
}

fn run_doc_tests(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    run_documentation(target, excluded, only)
}

fn run_all_tests(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    run_unit_tests(target, excluded, only)?;
    run_integration_tests(target, excluded, only)?;
    run_doc_tests(target, excluded, only)?;
    Ok(())
}
