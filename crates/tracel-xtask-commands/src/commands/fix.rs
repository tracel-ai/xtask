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

#[tracel_xtask_macros::declare_command_args(Target)]
pub struct FixCmdArgs {
    #[command(subcommand)]
    pub command: FixCommand,
}

#[tracel_xtask_macros::declare_subcommand(Fix)]
pub enum FixCommand {}

pub fn handle_command(args: FixCmdArgs, answer: Option<bool>) -> anyhow::Result<()> {
    if answer.is_none()
        && args.target == Target::Workspace
        && (!args.exclude.is_empty() || !args.only.is_empty())
    {
        warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
    }
    match args.command {
        FixCommand::Audit => run_audit(answer),
        FixCommand::Format => run_format(&args.target, &args.exclude, &args.only, answer),
        FixCommand::Lint => run_lint(&args.target, &args.exclude, &args.only, answer),
        FixCommand::Typos => run_typos(answer),
        FixCommand::All => {
            let answer = ask_once("This will run all the checks with autofix mode enabled.");
            FixCommand::iter()
                .filter(|c| *c != FixCommand::All)
                .try_for_each(|c| {
                    handle_command(
                        FixCmdArgs {
                            command: c,
                            target: args.target.clone(),
                            exclude: args.exclude.clone(),
                            only: args.only.clone(),
                        },
                        Some(answer),
                    )
                })
        }
    }
}

pub(crate) fn run_audit(mut answer: Option<bool>) -> anyhow::Result<()> {
    if answer.is_none() {
        answer = Some(ask_once(
            "This will run the audit check with autofix mode enabled.",
        ));
    };
    if answer.unwrap() {
        ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
        group!("Audit Rust Dependencies");
        run_process(
            "cargo",
            &vec!["audit", "-q", "--color", "always", "fix"],
            "Audit check execution failed",
        )?;
        endgroup!();
    }
    Ok(())
}

fn run_format(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
    mut answer: Option<bool>,
) -> Result<()> {
    match target {
        Target::Workspace => {
            if answer.is_none() {
                answer = Some(ask_once(
                    "This will run format with auto-fix on the workspace.",
                ));
            }
            if answer.unwrap() {
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
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            if answer.is_none() {
                answer = Some(ask_once(&format!(
                    "This will run format with auto-fix on all {} of the workspace.",
                    if *target == Target::Crates {
                        "crates"
                    } else {
                        "examples"
                    }
                )));
            }

            if answer.unwrap() {
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
        }
        Target::AllPackages => {
            if answer.is_none() {
                answer = Some(ask_once(
                    "This will run format check with auto-fix on all packages of the workspace.",
                ));
            }
            if answer.unwrap() {
                Target::iter()
                    .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                    .try_for_each(|t| run_format(&t, excluded, only, answer))?;
            }
        }
    }
    Ok(())
}

fn run_lint(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
    mut answer: Option<bool>,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            if answer.is_none() {
                answer = Some(ask_once(
                    "This will run lint with auto-fix on the workspace.",
                ));
            }
            if answer.unwrap() {
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
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            if answer.is_none() {
                answer = Some(ask_once(&format!(
                    "This will run lint with auto-fix on all {} of the workspace.",
                    if *target == Target::Crates {
                        "crates"
                    } else {
                        "examples"
                    }
                )));
            }

            if answer.unwrap() {
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
        }
        Target::AllPackages => {
            if answer.is_none() {
                answer = Some(ask_once(
                    "This will run lint check with auto-fix on all packages of the workspace.",
                ));
            }
            if answer.unwrap() {
                Target::iter()
                    .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                    .try_for_each(|t| run_lint(&t, excluded, only, answer))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn run_typos(mut answer: Option<bool>) -> anyhow::Result<()> {
    if answer.is_none() {
        answer = Some(ask_once(
            "This will look for typos in the source code check and auto-fix them.",
        ));
    };
    if answer.unwrap() {
        ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
        group!("Typos");
        run_process(
            "typos",
            &vec!["--write-changes", "--color", "always"],
            "Some typos have been found and cannot be fixed.",
        )?;
        endgroup!();
    }
    Ok(())
}
