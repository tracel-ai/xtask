use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{WorkspaceMember, WorkspaceMemberType, get_workspace_members},
    },
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, DocSubCommand)]
pub struct DocCmdArgs {}

pub fn handle_command(args: DocCmdArgs, _env: Environment, _ctx: Context) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{WARN_IGNORED_ONLY_ARGS}");
    }
    match args.get_command() {
        DocSubCommand::Build => run_documentation_build(
            &args.target,
            &args.exclude,
            &args.only,
            &args.features,
            args.no_default_features,
        ),
        DocSubCommand::Tests => run_documentation(
            &args.target,
            &args.exclude,
            &args.only,
            &args.features,
            args.no_default_features,
        ),
    }
}

fn run_documentation_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
    features: &[String],
    no_default_features: bool,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Build Workspace documentation");
            let mut cmd_args = vec!["doc", "--workspace", "--no-deps", "--color=always"];

            if no_default_features {
                cmd_args.push("--no-default-features");
            }

            let features_str = features.join(",");
            if !features.is_empty() {
                cmd_args.push("--features");
                cmd_args.push(&features_str);
            }

            run_process_for_workspace(
                "cargo",
                &cmd_args,
                excluded,
                None,
                None,
                "Workspace documentation build failed",
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
                group!("Doc Build: {}", member.name);
                let mut cmd_args = vec!["doc", "-p", &member.name, "--no-deps", "--color=always"];

                if no_default_features {
                    cmd_args.push("--no-default-features");
                }

                let features_str = features.join(",");
                if !features.is_empty() {
                    cmd_args.push("--features");
                    cmd_args.push(&features_str);
                }

                run_process_for_package(
                    "cargo",
                    &member.name,
                    &cmd_args,
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
                .try_for_each(|t| {
                    run_documentation_build(&t, excluded, only, features, no_default_features)
                })?;
        }
    }
    Ok(())
}

pub(crate) fn run_documentation(
    target: &Target,
    excluded: &[String],
    only: &[String],
    features: &[String],
    no_default_features: bool,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Workspace Documentation Tests");
            let mut cmd_args = vec!["test", "--workspace", "--doc", "--color", "always"];

            if no_default_features {
                cmd_args.push("--no-default-features");
            }

            let features_str = features.join(",");
            if !features.is_empty() {
                cmd_args.push("--features");
                cmd_args.push(&features_str);
            }

            run_process_for_workspace(
                "cargo",
                &cmd_args,
                excluded,
                Some(r"Doc-tests (\w+)"),
                Some("Doc Tests"),
                "Workspace documentation test failed",
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
                run_doc_test(&member, excluded, only, features, no_default_features)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| {
                    run_documentation(&t, excluded, only, features, no_default_features)
                })?;
        }
    }
    Ok(())
}

fn run_doc_test(
    member: &WorkspaceMember,
    excluded: &[String],
    only: &[String],
    features: &[String],
    no_default_features: bool,
) -> Result<(), anyhow::Error> {
    group!("Doc Tests: {}", member.name);
    let mut cmd_args = vec!["test", "--doc", "-p", &member.name];

    if no_default_features {
        cmd_args.push("--no-default-features");
    }

    let features_str = features.join(",");
    if !features.is_empty() {
        cmd_args.push("--features");
        cmd_args.push(&features_str);
    }
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args,
        excluded,
        only,
        &format!(
            "Failed to execute documentation test for '{}'",
            &member.name
        ),
        Some("no library targets found"),
        Some(&format!(
            "No library found to test documentation for in the crate '{}'",
            &member.name
        )),
    )?;
    endgroup!();
    Ok(())
}
