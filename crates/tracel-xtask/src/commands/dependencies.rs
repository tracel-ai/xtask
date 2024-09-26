use anyhow::Ok;
use strum::IntoEnumIterator;

use crate::{
    endgroup, group,
    utils::{cargo::ensure_cargo_crate_is_installed, process::run_process},
};

#[tracel_xtask_macros::declare_command_args(None, DependenciesSubCommand)]
pub struct DependenciesCmdArgs {}

pub fn handle_command(args: DependenciesCmdArgs) -> anyhow::Result<()> {
    match args.get_command() {
        DependenciesSubCommand::Deny => run_cargo_deny(),
        DependenciesSubCommand::Unused => run_cargo_machete(),
        DependenciesSubCommand::All => DependenciesSubCommand::iter()
            .filter(|c| *c != DependenciesSubCommand::All)
            .try_for_each(|c| handle_command(DependenciesCmdArgs { command: Some(c) })),
    }
}

/// Run cargo-deny
fn run_cargo_deny() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("cargo-deny", None, None, false)?;
    // Run cargo deny
    group!("Cargo: run deny checks");
    run_process(
        "cargo",
        &["deny", "check"],
        None,
        None,
        "Some dependencies don't meet the requirements!",
    )?;
    endgroup!();
    Ok(())
}

/// Run cargo-machete
fn run_cargo_machete() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("cargo-machete", None, None, false)?;
    // Run cargo machete
    group!("Cargo: run unused dependencies checks");
    run_process(
        "cargo",
        &["machete"],
        None,
        None,
        "Unused dependencies found!",
    )?;
    endgroup!();

    Ok(())
}
