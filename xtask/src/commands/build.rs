use tracel_xtask::prelude::*;

// Define a new target enum with an additional Frontend target
#[macros::extend_targets]
pub enum BuildTarget {
    /// Target the frontend.
    Frontend,
}

// Define new command arguments struct with an additional --debug argument
#[macros::extend_command_args(BuildCmdArgs, BuildTarget, None)]
pub struct ExtendedBuildCmdArgs {
    /// Print additional info when set
    #[arg(short, long)]
    pub debug: bool,
}

// Handle function processing the new command arguments struct
// For all other base targets we call the base command implementation
pub fn handle_command(args: ExtendedBuildCmdArgs) -> anyhow::Result<()> {
    match args.target {
        BuildTarget::Frontend => handle_frontend_target(args),
        _ => base_commands::build::handle_command(args.try_into().unwrap()),
    }
}

fn handle_frontend_target(args: ExtendedBuildCmdArgs) -> Result<(), anyhow::Error> {
    println!("Handling of extended target 'frontend'");
    if args.debug {
        println!("This is a debug log.")
    }
    Ok(())
}
