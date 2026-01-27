use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{WorkspaceMemberType, get_workspace_members},
    },
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, None)]
pub struct CleanCmdArgs {}

pub fn handle_command(args: CleanCmdArgs, _env: Environment, _ctx: Context) -> anyhow::Result<()> {
    run_clean(&args.target, &args)
}

pub(crate) fn run_clean(target: &Target, args: &CleanCmdArgs) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Clean Workspace");
            let cmd_args = vec!["clean", "--color", "always"];
            run_process_for_workspace(
                "cargo",
                &cmd_args,
                &args.exclude,
                None,
                None,
                "Workspace clean failed",
                None,
                None,
            )?;
            endgroup!();
        }
        Target::Crates | Target::Examples => {
            let members = match args.target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                let cmd_args = vec!["clean", "-p", &member.name, "--color", "always"];
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &cmd_args,
                    &args.exclude,
                    &args.only,
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
                .try_for_each(|t| run_clean(&t, args))?;
        }
    }
    Ok(())
}
