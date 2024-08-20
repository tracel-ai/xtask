use tracel_xtask::prelude::*;

#[macros::extend_command_args(BuildCmdArgs, Target, None)]
pub struct ExtendedBuildArgsCmdArgs {
    /// Print additional debug info when set
    #[arg(short, long)]
    pub debug: bool,
}

pub fn handle_command(args: ExtendedBuildArgsCmdArgs) -> anyhow::Result<()> {
    if args.debug {
        println!("Debug is enabled");
    }
    base_commands::build::handle_command(args.try_into().unwrap())
}
