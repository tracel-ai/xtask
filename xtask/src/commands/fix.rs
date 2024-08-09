// This a comprehensive example on how to extend an existing base command provided by tracel_xtask_commands
// This example extends the targets and the sub-commands with custom args.
use strum::IntoEnumIterator;
use tracel_xtask_commands::prelude::*;

// Extends the available target for the fix command by adding a 'ci' target
#[macros::extend_targets]
pub enum FixTarget {
    /// Target the CI.
    CI,
}

// Extends the fix command arguments by defining our own subcommands
#[macros::extend_command_args(FixCmdArgs, FixTarget, ExtendedFixSubCommand)]
pub struct ExtendedFixCmdArgs {}

// Extends the subcommands of fix command by adding a 'new-subcommand' subcommand
#[macros::extend_subcommands(FixSubCommand)]
pub enum ExtendedFixSubCommand {
    /// An additional subcommand for our extended Fix command.
    NewSubcommand(NewSubcommandArgs),
}

// We can add custom arguments for our 'new-subcommand' subcommand as well
#[derive(clap::Args, Clone, PartialEq, Default)]
pub struct NewSubcommandArgs {
    /// Print additional info when set
    #[arg(short, long)]
    pub debug: bool,
}

// Handle function processing the extended command arguments struct with extended subcommands
pub fn handle_command(args: ExtendedFixCmdArgs, answer: Option<bool>) -> anyhow::Result<()> {
    // we need to handle both the new subcommand 'new-subcommand' and the 'all' subcommand
    match args.command {
        ExtendedFixSubCommand::NewSubcommand(ref subcmd_args) => {
            run_new_subcommand_fix(args.clone(), subcmd_args, answer)
        }
        ExtendedFixSubCommand::All => {
            let answer = ask_once("This will run all the checks with autofix mode enabled.");
            ExtendedFixSubCommand::iter()
                .filter(|c| *c != ExtendedFixSubCommand::All)
                .try_for_each(|c| {
                    handle_command(
                        ExtendedFixCmdArgs {
                            command: c,
                            target: args.target.clone(),
                            exclude: args.exclude.clone(),
                            only: args.only.clone(),
                        },
                        Some(answer),
                    )
                })
        }
        _ => base_commands::fix::handle_command(args.try_into().unwrap(), answer),
    }
}

fn run_new_subcommand_fix(
    args: ExtendedFixCmdArgs,
    subcmd_args: &NewSubcommandArgs,
    mut answer: Option<bool>,
) -> Result<(), anyhow::Error> {
    if answer.is_none() {
        answer = Some(ask_once(
            "This will run the new-subcommand check with autofix mode enabled.",
        ));
    };
    if answer.unwrap() {
        group!("Subcommand");
        if subcmd_args.debug {
            println!("Debug mode enabled.")
        }
        match args.target {
            FixTarget::AllPackages => println!("Executing new subcommand on all packages."),
            FixTarget::Crates => println!("Executing new subcommand on all crates."),
            FixTarget::Examples => println!("Executing new subcommand on all examples."),
            FixTarget::Workspace => println!("Executing new subcommand on workspace."),
            FixTarget::CI => println!("Executing new subcommand on CI."),
        }
        endgroup!();
    }
    Ok(())
}
