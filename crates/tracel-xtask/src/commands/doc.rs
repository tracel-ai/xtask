use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType},
    },
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, DocSubCommand)]
pub struct DocCmdArgs {}

pub fn handle_command(args: DocCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    match args.get_command() {
        DocSubCommand::Build => run_documentation_build(&args.target, &args.exclude, &args.only),
        DocSubCommand::Tests => run_documentation(&args.target, &args.exclude, &args.only),
    }
}

fn run_documentation_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Build Workspace documentation");
            run_process_for_workspace(
                "cargo",
                vec!["doc", "--workspace", "--no-deps", "--color=always"],
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
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["doc", "-p", &member.name, "--no-deps", "--color=always"],
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
                .try_for_each(|t| run_documentation_build(&t, excluded, only))?;
        }
    }
    Ok(())
}

pub(crate) fn run_documentation(
    target: &Target,
    excluded: &[String],
    only: &[String],
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Workspace Documentation Tests");
            run_process_for_workspace(
                "cargo",
                vec!["test", "--workspace", "--doc", "--color", "always"],
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
                run_doc_test(&member, excluded, only)?;
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

fn run_doc_test(
    member: &WorkspaceMember,
    excluded: &[String],
    only: &[String],
) -> Result<(), anyhow::Error> {
    group!("Doc Tests: {}", member.name);
    run_process_for_package(
        "cargo",
        &member.name,
        &vec!["test", "--doc", "-p", &member.name],
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
