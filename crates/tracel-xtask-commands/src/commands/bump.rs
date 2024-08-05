use anyhow::Ok;
use clap::Subcommand;
use strum::{Display, EnumIter, EnumString};

use crate::{
    endgroup, group,
    utils::{cargo::ensure_cargo_crate_is_installed, process::run_process},
};

#[tracel_xtask_macros::bump_command_arguments()]
pub struct BumpCmdArgs {
    #[command(subcommand)]
    pub command: BumpCommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum BumpCommand {
    /// Run unit tests.
    Major,
    /// Run integration tests.
    Minor,
    /// Run documentation tests.
    Patch,
}

pub fn handle_command(args: BumpCmdArgs) -> anyhow::Result<()> {
    bump(&args.command)
}

fn bump(command: &BumpCommand) -> anyhow::Result<()> {
    group!("Bump version: {command}");
    ensure_cargo_crate_is_installed("cargo-edit", None, None, false)?;
    run_process(
        "cargo",
        &vec!["set-version", "--bump", &command.to_string()],
        &format!("Error trying to bump {command} version"),
    )?;
    endgroup!();
    Ok(())
}
