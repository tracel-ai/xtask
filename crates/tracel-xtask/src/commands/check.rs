use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS,
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
    versions::TYPOS_VERSION,
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, CheckSubCommand)]
pub struct CheckCmdArgs {}

pub fn handle_command(
    args: CheckCmdArgs,
    _env: Environment,
    _context: Context,
) -> anyhow::Result<()> {
    if args.target == Target::Workspace && (!args.exclude.is_empty() || !args.only.is_empty()) {
        warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
    }

    match args.get_command() {
        CheckSubCommand::Audit if args.ignore_audit => {
            if run_audit().is_err() {
                warn!("Ignoring audit error because of '--ignore-audit' flag.");
            }
            Ok(())
        }
        CheckSubCommand::Audit => run_audit(),
        CheckSubCommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CheckSubCommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
        CheckSubCommand::Typos => run_typos(),
        CheckSubCommand::All => CheckSubCommand::iter()
            .filter(|c| *c != CheckSubCommand::All)
            .try_for_each(|c| {
                handle_command(
                    CheckCmdArgs {
                        command: Some(c),
                        target: args.target.clone(),
                        exclude: args.exclude.clone(),
                        only: args.only.clone(),
                        ignore_audit: args.ignore_audit,
                    },
                    _env.clone(),
                    _context.clone(),
                )
            }),
    }
}

fn run_audit() -> anyhow::Result<()> {
    group!("Audit Rust Dependencies");
    ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
    run_process(
        "cargo",
        &["audit", "-q", "--color", "always"],
        None,
        None,
        "Audit check execution failed",
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
                &["fmt", "--check"],
                &[],
                None,
                None,
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
                    &["fmt", "--check", "-p", &member.name],
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
                &[
                    "clippy",
                    "--no-deps",
                    "--color=always",
                    "--",
                    "--deny",
                    "warnings",
                ],
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
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &[
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
    if std::env::var("CI").is_err() {
        ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
    }
    group!("Typos");
    run_process(
        "typos",
        &["--diff", "--color", "always"],
        None,
        None,
        "Typos check execution failed",
    )?;
    endgroup!();
    Ok(())
}
