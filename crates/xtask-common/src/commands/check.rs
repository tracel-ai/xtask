use anyhow::Ok;
use clap::Subcommand;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS,
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
};

use super::Target;

#[xtask_macros::arguments(target, exclude, only)]
pub struct CheckCmdArgs {
    #[command(subcommand)]
    pub command: CheckCommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum CheckCommand {
    /// Run all the checks.
    All,
    /// Run audit command.
    Audit,
    /// Run format command.
    Format,
    /// Run lint command.
    Lint,
    /// Report typos in source code.
    Typos,
}

pub fn handle_command(args: CheckCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace
        && (!args.exclude.is_empty() || !args.only.is_empty())
    {
        warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
    }

    match args.command {
        CheckCommand::Audit => run_audit(),
        CheckCommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CheckCommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
        CheckCommand::Typos => run_typos(),
        CheckCommand::All => CheckCommand::iter()
            .filter(|c| *c != CheckCommand::All)
            .try_for_each(|c| {
                handle_command(CheckCmdArgs {
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
    run_process(
        "cargo",
        &vec!["audit", "-q", "--color", "always"],
        "Audit check execution failed",
        true,
    )?;
    endgroup!();
    Ok(())
}

fn run_format(target: &Target, excluded: &[String], only: &[String]) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Format Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["fmt", "--check"],
                &[],
                "Workspace format failed",
                None,
                None,
            )?;
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
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["fmt", "--check", "-p", &member.name],
                    excluded,
                    only,
                    &format!("Format check execution failed for {}", &member.name),
                    None,
                    None,
                )?;
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

fn run_lint(target: &Target, excluded: &[String], only: &[String]) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");
            run_process_for_workspace(
                "cargo",
                vec![
                    "clippy",
                    "--no-deps",
                    "--color=always",
                    "--",
                    "--deny",
                    "warnings",
                ],
                &[],
                "Workspace lint failed",
                None,
                None,
            )?;
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
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec![
                        "clippy",
                        "--no-deps",
                        "--color=always",
                        "-p",
                        &member.name,
                        "--",
                        "--deny",
                        "warnings",
                    ],
                    excluded,
                    only,
                    &format!("Lint fix execution failed for {}", &member.name),
                    None,
                    None,
                )?;
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
    run_process(
        "typos",
        &vec!["--diff", "--color", "always"],
        "Typos check execution failed",
        true,
    )?;
    endgroup!();
    Ok(())
}
