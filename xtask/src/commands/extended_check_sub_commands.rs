// Check base command already has subcommands. This command extends the Check command and
// extends its subcommands.
use strum::IntoEnumIterator;
use tracel_xtask::prelude::*;

#[macros::extend_command_args(CheckCmdArgs, Target, ExtendedCheckSubcommand)]
pub struct ExtendedCheckedArgsCmdArgs {}

#[macros::extend_subcommands(CheckSubCommand)]
pub enum ExtendedCheckSubcommand {
    /// An additional subcommand for our extended Fix command.
    MySubcommand,
}

pub fn handle_command(args: ExtendedCheckedArgsCmdArgs) -> anyhow::Result<()> {
    match args.get_command() {
        ExtendedCheckSubcommand::MySubcommand => run_my_subcommand(args.clone()),
        ExtendedCheckSubcommand::All => ExtendedCheckSubcommand::iter()
            .filter(|c| *c != ExtendedCheckSubcommand::All)
            .try_for_each(|c| {
                handle_command(ExtendedCheckedArgsCmdArgs {
                    command: Some(c),
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                })
            }),
        _ => base_commands::check::handle_command(args.try_into().unwrap()),
    }
}

fn run_my_subcommand(_args: ExtendedCheckedArgsCmdArgs) -> Result<(), anyhow::Error> {
    println!("Executing new subcommand");
    Ok(())
}
