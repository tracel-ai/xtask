use tracel_xtask::prelude::*;

#[macros::extend_command_args(TestCmdArgs, Target, ProjectTestSubCommand)]
pub struct ProjectTestCmdArgs {
    /// Print additional debug info when set.
    #[arg(short, long)]
    pub debug: bool,
}

#[macros::extend_subcommands(TestSubCommand)]
pub enum ProjectTestSubCommand {
    /// Run the project's custom test workflow.
    Project,
}

pub fn handle_command(
    args: ProjectTestCmdArgs,
    env: Environment,
    ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        ProjectTestSubCommand::Project => {
            let mut base_compatible_args = args.clone();
            base_compatible_args.command = Some(ProjectTestSubCommand::All);
            if !base_commands::test::check_environment(&base_compatible_args.try_into()?, &env) {
                std::process::exit(1);
            }

            if args.debug {
                eprintln!("project tests with debug enabled");
            } else {
                eprintln!("project tests with debug disabled");
            }
            Ok(())
        }
        _ => base_commands::test::handle_command(args.try_into()?, env, ctx),
    }
}
