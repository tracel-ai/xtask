use anyhow::{Ok, Result};
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS,
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        prompt::ask_once,
        workspace::{WorkspaceMemberType, get_workspace_members},
    },
    versions::TYPOS_VERSION,
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, FixSubCommand)]
pub struct FixCmdArgs {}

pub fn handle_command(
    args: FixCmdArgs,
    _env: Environment,
    _ctx: Context,
    mut answer: Option<bool>,
) -> anyhow::Result<()> {
    answer = if args.yes {
        Some(true)
    } else {
        warning_prompt(answer, &args)
    };
    if answer.unwrap() {
        match args.get_command() {
            FixSubCommand::Audit => run_audit(),
            FixSubCommand::Format => run_format(&args.target, &args.exclude, &args.only),
            FixSubCommand::Lint => run_lint(
                &args.target,
                &args.exclude,
                &args.only,
                &args.features,
                args.no_default_features,
            ),
            FixSubCommand::Typos => run_typos(),
            FixSubCommand::All => FixSubCommand::iter()
                .filter(|c| *c != FixSubCommand::All)
                .try_for_each(|c| {
                    handle_command(
                        FixCmdArgs {
                            command: Some(c),
                            target: args.target.clone(),
                            exclude: args.exclude.clone(),
                            only: args.only.clone(),
                            features: args.features.clone(),
                            no_default_features: args.no_default_features,
                            yes: args.yes,
                        },
                        _env.clone(),
                        _ctx.clone(),
                        answer,
                    )
                }),
        }
    } else {
        Ok(())
    }
}

pub fn warning_prompt(answer: Option<bool>, args: &FixCmdArgs) -> Option<bool> {
    if answer.is_none() {
        if args.target == Target::Workspace && (!args.exclude.is_empty() || !args.only.is_empty()) {
            warn!("{WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS}");
        }
        return Some(ask_once(
            "This will run the check with autofix mode enabled.",
        ));
    };
    answer
}

pub(crate) fn run_audit() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
    group!("Audit Rust Dependencies");
    run_process(
        "cargo",
        &["audit", "-q", "--color", "always", "fix"],
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
                &["fmt"],
                &[],
                None,
                None,
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
                    &["fmt", "-p", &member.name],
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

fn run_lint(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
    features: &[String],
    no_default_features: bool,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");

            let mut cmd_args = vec![
                "clippy",
                "--no-deps",
                "--fix",
                "--allow-dirty",
                "--allow-staged",
                "--allow-no-vcs",
                "--color=always",
            ];

            if no_default_features {
                cmd_args.push("--no-default-features");
            }

            let features_str = features.join(",");
            if !features.is_empty() {
                cmd_args.push("--features");
                cmd_args.push(&features_str);
            }

            cmd_args.extend(&["--", "--deny", "warnings"]);

            run_process_for_workspace(
                "cargo",
                &cmd_args,
                &[],
                None,
                None,
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

                let mut cmd_args = vec![
                    "clippy",
                    "--no-deps",
                    "--fix",
                    "--allow-dirty",
                    "--allow-staged",
                    "--color=always",
                    "-p",
                    &member.name,
                ];

                if no_default_features {
                    cmd_args.push("--no-default-features");
                }

                let features_str = features.join(",");
                if !features.is_empty() {
                    cmd_args.push("--features");
                    cmd_args.push(&features_str);
                }

                cmd_args.extend(&["--", "--deny", "warnings"]);

                run_process_for_package(
                    "cargo",
                    &member.name,
                    &cmd_args,
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
                .try_for_each(|t| run_lint(&t, excluded, only, features, no_default_features))?;
        }
    }
    Ok(())
}

pub(crate) fn run_typos() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
    group!("Typos");
    run_process(
        "typos",
        &["--write-changes", "--color", "always"],
        None,
        None,
        "Some typos have been found and cannot be fixed.",
    )?;
    endgroup!();
    Ok(())
}
