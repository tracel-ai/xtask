use anyhow::Ok;

use crate::{
    endgroup, group,
    prelude::{Context, Environment},
    utils::{
        cargo::ensure_cargo_crate_is_installed, process::run_process, rustup::rustup_add_component,
    },
    versions::GRCOV_VERSION,
};

use super::Profile;

#[tracel_xtask_macros::declare_command_args(None, CoverageSubCommand)]
pub struct CoverageCmdArgs {}

impl Default for CoverageSubCommand {
    fn default() -> Self {
        CoverageSubCommand::Generate(GenerateSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GenerateSubCmdArgs {
    /// Build profile to use.
    #[arg(short, long, value_enum, default_value_t = Profile::default())]
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

pub fn handle_command(
    args: CoverageCmdArgs,
    _env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        CoverageSubCommand::Install => install_grcov(),
        CoverageSubCommand::Generate(gen_args) => run_grcov(&gen_args),
    }
}

fn install_grcov() -> anyhow::Result<()> {
    rustup_add_component("llvm-tools-preview")?;
    if std::env::var("CI").is_err() {
        ensure_cargo_crate_is_installed("grcov", None, Some(GRCOV_VERSION), false)?;
    }
    Ok(())
}

fn run_grcov(generate_args: &GenerateSubCmdArgs) -> anyhow::Result<()> {
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
    run_process("grcov", &args, None, None, "Error executing grcov")?;
    endgroup!();
    Ok(())
}
