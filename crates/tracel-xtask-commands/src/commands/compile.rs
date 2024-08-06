use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
};

use super::Target;

#[tracel_xtask_macros::command_args(Target)]
pub struct CompileCmdArgs {}

pub fn handle_command(args: CompileCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    run_compile(&args.target, &args.exclude, &args.only)
}

pub(crate) fn run_compile(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> std::prelude::v1::Result<(), anyhow::Error> {
    match target {
        Target::Workspace => {
            group!("Compile Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["check", "--workspace"],
                excluded,
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
                group!("Compile: {}", member.name);
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["check", "-p", &member.name],
                    excluded,
                    only,
                    &format!("Compilation failed for {}", &member.name),
                    None,
                    None,
                )?;
                endgroup!();
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_compile(&t, excluded, only))?;
        }
    }
    Ok(())
}
