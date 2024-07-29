use std::process::Command;

use anyhow::{anyhow, Ok};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::{WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS, WARN_IGNORED_ONLY_ARGS},
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
    versions::TYPOS_VERSION,
};

use super::{
    check::run_compile,
    test::{run_documentation, run_integration, run_unit},
    Target,
};

#[derive(Args, Clone)]
pub struct CICmdArgs {
    /// Target to check for.
    #[arg(short, long, value_enum, default_value_t = Target::Workspace)]
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
    /// Compile the targets (does not write actual binaries).
    Compile,
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
        CICommand::Build |
        CICommand::AllTests |
        CICommand::DocTests |
        CICommand::IntegrationTests |
        CICommand::UnitTests => if args.target == Target::Workspace && !args.only.is_empty() {
            warn!("{}", WARN_IGNORED_ONLY_ARGS);
        }
        _ => if args.target == Target::Workspace && (!args.exclude.is_empty() || !args.only.is_empty()) {
            warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
        }
    }

    match args.command {
        CICommand::Audit => run_audit(),
        CICommand::Build => run_build(&args.target, &args.exclude, &args.only),
        CICommand::Compile => run_compile(&args.target, &args.exclude, &args.only, None),
        CICommand::DocTests => run_documentation(&args.target, &args.exclude, &args.only),
        CICommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CICommand::IntegrationTests => run_integration(&args.target, &args.exclude, &args.only),
        CICommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
        CICommand::Typos => run_typos(),
        CICommand::UnitTests => run_unit(&args.target, &args.exclude, &args.only),
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

fn run_audit() -> anyhow::Result<()> {
    group!("Audit Rust Dependencies");
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
    Ok(())
}

fn run_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> std::prelude::v1::Result<(), anyhow::Error> {
    match target {
        Target::Workspace => {
            let mut args = vec!["build", "--workspace"];
            excluded.iter().for_each(|ex| args.extend(["--exclude", ex]));
            group!("Build Workspace");
            info!("Command line: cargo {}", args.join(" "));
            let status = Command::new("cargo")
                .args(args)
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo build: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace build failed"));
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
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_build(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_format(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Format Workspace");
            info!("Command line: cargo fmt --check -- --color=always");
            let status = Command::new("cargo")
                .args(["fmt", "--check", "--", "--color=always"])
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo fmt: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace format failed"));
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
                group!("Format: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!(
                    "Command line: cargo fmt --check -p {} -- --color=always",
                    &member.name
                );
                let status = Command::new("cargo")
                    .args(["fmt", "--check", "-p", &member.name, "--", "--color=always"])
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
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_format(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_lint(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");
            info!("Command line: cargo clippy --no-deps --color=always -- --deny warnings");
            let status = Command::new("cargo")
                .args([
                    "clippy",
                    "--no-deps",
                    "--color=always",
                    "--",
                    "--deny",
                    "warnings",
                ])
                .status()
                .map_err(|e| anyhow!("Failed to execute cargo fmt: {}", e))?;
            if !status.success() {
                return Err(anyhow!("Workspace lint failed"));
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
                group!("Lint: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!(
                    "Command line: cargo clippy --no-deps --color=always -p {} -- --deny warnings",
                    &member.name
                );
                let status = Command::new("cargo")
                    .args([
                        "clippy",
                        "--no-deps",
                        "--color=always",
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
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_lint(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_typos() -> anyhow::Result<()> {
    group!("Typos");
    info!("Command line: typos --diff --color always");
    let status = Command::new("typos")
        .args(["--diff", "--color", "always"])
        .status()
        .map_err(|e| anyhow!("Failed to execute typos: {}", e))?;
    if !status.success() {
        return Err(anyhow!("Typos check execution failed"));
    }
    endgroup!();
    Ok(())
}

fn run_all_tests(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    run_unit(target, excluded, only)?;
    run_integration(target, excluded, only)?;
    run_documentation(target, excluded, only)?;
    Ok(())
}
