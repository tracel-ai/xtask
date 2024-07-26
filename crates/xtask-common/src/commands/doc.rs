use std::process::Command;

use anyhow::{anyhow, Ok};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    endgroup, group,
    utils::workspace::{get_workspace_members, WorkspaceMemberType},
};

use super::Target;

#[derive(Args, Clone)]
pub struct DocCmdArgs {
    /// Target to check for.
    #[arg(short, long, value_enum, default_value_t = Target::All)]
    target: Target,
    /// Comma-separated list of excluded crates.
    #[arg(
        short = 'x',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub exclude: Vec<String>,
    /// Comma-separated list of crates to include exclusively.
    #[arg(
        short = 'n',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub only: Vec<String>,
    #[command(subcommand)]
    pub command: DocCommand,
}

#[derive(Default, EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum DocCommand {
    #[default]
    /// Build documentation.
    Build,
}

pub fn handle_command(args: DocCmdArgs) -> anyhow::Result<()> {
    match args.command {
        DocCommand::Build => run_documentation_build(&args.target, &args.exclude, &args.only),
    }
}

fn run_documentation_build(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    match target {
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                group!("Doc Build: {}", member.name);
                if excluded.contains(&member.name)
                    || (!only.is_empty() && !only.contains(&member.name))
                {
                    info!("Skip '{}' because it has been excluded!", &member.name);
                    continue;
                }
                info!("Command line: cargo doc -p {} --color=always", &member.name);
                let status = Command::new("cargo")
                    .args(["doc", "-p", &member.name, "--color=always"])
                    .status()
                    .map_err(|e| anyhow!("Failed to execute cargo fmt: {}", e))?;
                if !status.success() {
                    return Err(anyhow!(
                        "Format check execution failed for {}",
                        &member.name
                    ));
                }
                endgroup!();
            }
        }
        Target::All => {
            Target::iter()
                .filter(|t| *t != Target::All)
                .try_for_each(|t| run_documentation_build(&t, excluded, only))?;
        }
    }
    Ok(())
}
