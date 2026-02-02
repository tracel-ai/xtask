use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS,
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        workspace::{WorkspaceMemberType, get_workspace_members},
    },
    versions::TYPOS_VERSION,
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, CheckSubCommand)]
pub struct CheckCmdArgs {}

pub fn handle_command(args: CheckCmdArgs, _env: Environment, _ctx: Context) -> anyhow::Result<()> {
    if args.target == Target::Workspace && (!args.exclude.is_empty() || !args.only.is_empty()) {
        warn!("{WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS}");
    }

    match args.get_command() {
        CheckSubCommand::Audit => {
            let res = run_audit();
            if res.is_err() && args.ignore_audit {
                warn!("Ignoring audit error because of '--ignore-audit' flag.");
                Ok(())
            } else {
                res
            }
        }
        CheckSubCommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CheckSubCommand::Lint => run_lint(
            &args.target,
            &args.exclude,
            &args.only,
            &args.features,
            args.no_default_features,
        ),
        CheckSubCommand::Typos => {
            let res = run_typos();
            if res.is_err() && args.ignore_typos {
                warn!("Ignoring typos error because of '--ignore-typos' flag.");
                Ok(())
            } else {
                res
            }
        }
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
                        ignore_typos: args.ignore_typos,
                        features: args.features.clone(),
                        no_default_features: args.no_default_features,
                    },
                    _env.clone(),
                    _ctx.clone(),
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

fn run_lint(
    target: &Target,
    excluded: &[String],
    only: &[String],
    features: &[String],
    no_default_features: bool,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");
            let mut cmd_args = vec!["clippy", "--no-deps", "--color=always"];

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
                let mut cmd_args =
                    vec!["clippy", "--no-deps", "--color=always", "-p", &member.name];

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

fn run_typos() -> anyhow::Result<()> {
    if std::env::var("CI").is_err() {
        ensure_cargo_crate_is_installed("typos-cli", None, Some(TYPOS_VERSION), false)?;
    }
    group!("Typos");
    run_process(
        "typos",
        // default without any args if better than '--diff --colors always'
        &[],
        None,
        None,
        "Typos check execution failed",
    )?;
    endgroup!();
    Ok(())
}
