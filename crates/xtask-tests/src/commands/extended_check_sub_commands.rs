// Check base command already has subcommands. This command extends the Check command and
// extends its subcommands.
use strum::IntoEnumIterator;
use tracel_xtask::prelude::*;

#[macros::extend_command_args(CheckCmdArgs, Target, ExtendedCheckSubcommand)]
pub struct ExtendedCheckArgsCmdArgs {}

#[macros::extend_subcommands(CheckSubCommand)]
pub enum ExtendedCheckSubcommand {
    /// An additional subcommand for our extended Fix command.
    MySubCommand,
}

pub fn handle_command(args: ExtendedCheckArgsCmdArgs) -> anyhow::Result<()> {
    match args.get_command() {
        ExtendedCheckSubcommand::MySubCommand => run_my_subcommand(args.clone()),
        ExtendedCheckSubcommand::All => {
            eprintln!("Executing all");
            ExtendedCheckSubcommand::iter()
                .filter(|c| *c != ExtendedCheckSubcommand::All)
                .try_for_each(|c| {
                    handle_command(ExtendedCheckArgsCmdArgs {
                        command: Some(c),
                        target: args.target.clone(),
                        exclude: args.exclude.clone(),
                        only: args.only.clone(),
                        ignore_audit: args.ignore_audit,
                        ignore_typos: args.ignore_typos,
                    })
                })
        }
        command => {
            eprintln!("Executing {command}");
            // this should be uncommented but we skip the actual execution here because we use
            // this command in the integration test as well.
            // base_commands::check::handle_command(args.try_into().unwrap())
            Ok(())
        }
    }
}

fn run_my_subcommand(_args: ExtendedCheckArgsCmdArgs) -> Result<(), anyhow::Error> {
    eprintln!("Executing new subcommand");
    Ok(())
}
