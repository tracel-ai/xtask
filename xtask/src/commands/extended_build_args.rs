use tracel_xtask::prelude::*;

// BuildCmdArgs has no subcommand so we pass None as third argument
#[macros::extend_command_args(BuildCmdArgs, Target, None)]
pub struct ExtendedBuildArgsCmdArgs {
    /// Print additional debug info when set
    #[arg(short, long)]
    pub debug: bool,
}

pub fn handle_command(
    args: ExtendedBuildArgsCmdArgs,
    env: Environment,
    context: Context,
) -> anyhow::Result<()> {
    if args.debug {
        println!("debug enabled");
    } else {
        println!("debug disabled");
    }
    base_commands::build::handle_command(args.try_into().unwrap(), env.clone(), context.clone())
}
