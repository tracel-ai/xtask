use anyhow::{Ok, Result};
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS,
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        prompt::ask_once,
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
    versions::TYPOS_VERSION,
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, FixSubCommand)]
pub struct FixCmdArgs {}

pub fn handle_command(args: FixCmdArgs, mut answer: Option<bool>) -> anyhow::Result<()> {
    if answer.is_none() {
        if args.target == Target::Workspace && (!args.exclude.is_empty() || !args.only.is_empty()) {
            warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
        }
        answer = Some(ask_once(
            "This will run the check with autofix mode enabled.",
        ));
    };
    if answer.unwrap() {
        match args.command {
            FixSubCommand::Audit => run_audit(),
            FixSubCommand::Format => run_format(&args.target, &args.exclude, &args.only),
            FixSubCommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
            FixSubCommand::Typos => run_typos(),
            FixSubCommand::All => FixSubCommand::iter()
                .filter(|c| *c != FixSubCommand::All)
                .try_for_each(|c| {
                    handle_command(
                        FixCmdArgs {
                            command: c,
                            target: args.target.clone(),
                            exclude: args.exclude.clone(),
                            only: args.only.clone(),
                        },
                        answer,
                    )
                }),
        }
    } else {
        Ok(())
    }
}

pub(crate) fn run_audit() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
    group!("Audit Rust Dependencies");
    run_process(
        "cargo",
        &vec!["audit", "-q", "--color", "always", "fix"],
        None,
        None,
        "Audit check execution failed",
    )?;
    endgroup!();
    Ok(())
}

fn run_format(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> Result<()> {
    match target {
        Target::Workspace => {
            group!("Format Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["fmt"],
                &[],
                "Workspace compilation failed",
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
                    &vec!["fmt", "-p", &member.name],
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

fn run_lint(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");
            run_process_for_workspace(
                "cargo",
                vec![
                    "clippy",
                    "--no-deps",
                    "--fix",
                    "--allow-dirty",
                    "--allow-staged",
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
                        "--fix",
                        "--allow-dirty",
                        "--allow-staged",
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

pub(crate) fn run_typos() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
    group!("Typos");
    run_process(
        "typos",
        &vec!["--write-changes", "--color", "always"],
        None,
        None,
        "Some typos have been found and cannot be fixed.",
    )?;
    endgroup!();
    Ok(())
}
