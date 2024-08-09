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

#[tracel_xtask_macros::declare_command_args(Target, None)]
pub struct BuildCmdArgs {}

pub fn handle_command(args: BuildCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    run_build(&args.target, &args.exclude, &args.only)
}

pub(crate) fn run_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Build Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["build", "--workspace", "--color", "always"],
                excluded,
                "Workspace build failed",
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
                group!("Build: {}", member.name);
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["build", "-p", &member.name, "--color", "always"],
                    excluded,
                    only,
                    &format!("Build command failed for {}", &member.name),
                    None,
                    None,
                )?;
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
