use anyhow::Ok;
use clap::Args;

use crate::{
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed, process::run_process, rustup::rustup_add_component,
    },
    versions::GRCOV_VERSION,
};

use super::Profile;

#[tracel_xtask_macros::declare_command_args]
pub struct CoverageCmdArgs {
    #[command(subcommand)]
    pub command: CoverageCommand,
}

#[tracel_xtask_macros::declare_subcommands(Coverage)]
pub enum CoverageCommand {}

#[derive(Args, Default, Clone, PartialEq)]
pub struct GenerateCmdArgs {
    /// Build profile to use.
    #[arg(short, long, value_enum, default_value_t = Profile::Debug)]
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
    if std::env::var("CI").is_err() {
        ensure_cargo_crate_is_installed("grcov", None, Some(GRCOV_VERSION), false)?;
    }
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
    generate_args
        .ignore
        .iter()
        .for_each(|i| args.extend(vec!["--ignore", i]));
    run_process("grcov", &args, "Error executing grcov")?;
    endgroup!();
    Ok(())
}
