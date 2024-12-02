use tracel_xtask::prelude::*;

// TestCmdArgs has a subcommand so we need to pass it as third argument
#[macros::extend_command_args(TestCmdArgs, Target, TestSubCommand)]
pub struct ExtendedTestArgsCmdArgs {
    /// Print additional debug info when set
    #[arg(short, long)]
    pub debug: bool,
}

pub fn handle_command(args: ExtendedTestArgsCmdArgs) -> anyhow::Result<()> {
    if args.debug {
        println!("debug enabled");
    } else {
        println!("debug disabled");
    }
    // We don't run the actual tests as it creates an infinite loop while executing the integration tests.
    // base_commands::test::handle_command(args.try_into().unwrap())
    Ok(())
}
