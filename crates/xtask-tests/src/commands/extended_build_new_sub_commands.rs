// Build base command has not subcommands. This command extends the Build command to add
// new subcommands.
use tracel_xtask::prelude::*;

#[macros::extend_command_args(BuildCmdArgs, Target, BuildSubcommand)]
pub struct ExtendedBuildSubCommandsCmdArgs {}

#[derive(
    strum::EnumString, strum::EnumIter, strum::Display, Clone, PartialEq, clap::Subcommand, Default,
)]
#[strum(serialize_all = "lowercase")]
pub enum BuildSubcommand {
    #[default]
    Command1,
    Command2,
}

pub fn handle_command(
    args: ExtendedBuildSubCommandsCmdArgs,
    env: Environment,
    ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        BuildSubcommand::Command1 => eprintln!("Executing build sub command 1"),
        BuildSubcommand::Command2 => eprintln!("Executing build sub command 2"),
    }
    base_commands::build::handle_command(args.try_into().unwrap(), env.clone(), ctx.clone())
}
