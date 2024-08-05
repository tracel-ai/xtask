use anyhow::Ok;
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::CARGO_NIGHTLY_MSG,
    endgroup, group,
    utils::{
        cargo::{ensure_cargo_crate_is_installed, is_current_toolchain_nightly},
        process::run_process,
    },
};

#[derive(Args, Clone)]
pub struct DependenciesCmdArgs {
    #[command(subcommand)]
    pub command: DependencyCommand,
}

#[derive(EnumString, Default, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum DependencyCommand {
    /// Run all dependency checks.
    #[default]
    All,
    /// Run cargo-deny Lint dependency graph to ensure all dependencies meet requirements `<https://crates.io/crates/cargo-deny>`
    Deny,
    /// Run cargo-udeps to find unused dependencies `<https://crates.io/crates/cargo-udeps>`
    Unused,
}

pub fn handle_command(args: DependenciesCmdArgs) -> anyhow::Result<()> {
    match args.command {
        DependencyCommand::Deny => run_cargo_deny(),
        DependencyCommand::Unused => run_cargo_udeps(),
        DependencyCommand::All => DependencyCommand::iter()
            .filter(|c| *c != DependencyCommand::All)
            .try_for_each(|c| handle_command(DependenciesCmdArgs { command: c })),
    }
}

/// Run cargo-deny
fn run_cargo_deny() -> anyhow::Result<()> {
    ensure_cargo_crate_is_installed("cargo-deny", None, None, false)?;
    // Run cargo deny
    group!("Cargo: run deny checks");
    run_process(
        "cargo",
        &vec!["deny", "check"],
        "Some dependencies don't meet the requirements!",
    )?;
    endgroup!();
    Ok(())
}

/// Run cargo-udeps
fn run_cargo_udeps() -> anyhow::Result<()> {
    if is_current_toolchain_nightly() {
        ensure_cargo_crate_is_installed("cargo-udeps", None, None, false)?;
        // Run cargo udeps
        group!("Cargo: run unused dependencies checks");
        run_process("cargo", &vec!["udeps"], "Unused dependencies found!")?;
        endgroup!();
    } else {
        error!("{}", CARGO_NIGHTLY_MSG);
    }
    Ok(())
}
