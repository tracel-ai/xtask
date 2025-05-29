use anyhow::Ok;

use crate::{
    endgroup, group,
    prelude::{Context, Environment},
    utils::{cargo::ensure_cargo_crate_is_installed, process::run_process},
};

#[tracel_xtask_macros::declare_command_args(None, BumpSubCommand)]
pub struct BumpCmdArgs {}

pub fn handle_command(
    args: BumpCmdArgs,
    _env: Environment,
    _context: Context,
) -> anyhow::Result<()> {
    bump(&args.get_command())
}

fn bump(command: &BumpSubCommand) -> anyhow::Result<()> {
    group!("Bump version: {command}");
    ensure_cargo_crate_is_installed("cargo-edit", None, None, false)?;
    run_process(
        "cargo",
        &["set-version", "--bump", &command.to_string()],
        None,
        None,
        &format!("Error trying to bump {command} version"),
    )?;
    endgroup!();
    Ok(())
}
