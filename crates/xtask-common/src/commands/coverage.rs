use std::process::Command;

use anyhow::{anyhow, Ok};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString};

use crate::{
    endgroup, group,
    utils::{cargo::ensure_cargo_crate_is_installed, rustup::rustup_add_component},
};

use super::Profile;

#[derive(Args, Clone)]
pub struct CoverageCmdArgs {
    #[command(subcommand)]
    pub command: CoverageCommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum CoverageCommand {
    /// Install grcov and its dependencies.
    Install,
    /// Generate lcov.info file.
    Generate(GenerateCmdArgs),
}

#[derive(Args, Default, Clone, PartialEq)]
pub struct GenerateCmdArgs {
    /// Build profile to use.
    #[arg(short, long, value_enum)]
    profile: Profile,
    /// Comma-separated list of excluded crates.
    #[arg(
        short = 'i',
        long,
        value_name = "PATH,PATH,...",
        value_delimiter = ',',
        required = false
    )]
    pub ignore: Vec<String>,
}

pub fn handle_command(args: CoverageCmdArgs) -> anyhow::Result<()> {
    match args.command {
        CoverageCommand::Install => install_grcov(),
        CoverageCommand::Generate(gen_args) => run_grcov(&gen_args),
    }
}

fn install_grcov() -> anyhow::Result<()> {
    rustup_add_component("llvm-tools-preview")?;
    ensure_cargo_crate_is_installed("grcov", None, Some("0.8.19"), false)?;
    Ok(())
}

fn run_grcov(generate_args: &GenerateCmdArgs) -> anyhow::Result<()> {
    group!("Grcov");
    let binary_path = format!("./target/{}/", generate_args.profile);
    #[rustfmt::skip]
    let mut args = vec![
        ".",
        "--binary-path", &binary_path,
        "-s", ".",
        "-t", "lcov",
        "-o", "lcov.info",
        "--branch",
        "--ignore-not-existing",
    ];
    generate_args.ignore.iter().for_each(|i| args.extend(vec!["--ignore", i]));
    let status = Command::new("grcov")
        .args(args)
        .status()
        .map_err(|e| anyhow!("Failed to execute grcov: {}", e))?;
    if !status.success() {
        return Err(anyhow!("Error executing grcov"));
    }
    endgroup!();
    Ok(())
}
