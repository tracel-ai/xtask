use tracel_xtask::prelude::*;

#[macros::declare_command_args(Target, MySubCommand)]
struct MyCommandWithSubCommandsCmdArgs {}

#[derive(
    strum::EnumString, strum::EnumIter, strum::Display, Clone, PartialEq, clap::Subcommand, Default,
)]
#[strum(serialize_all = "lowercase")]
pub enum MySubCommand {
    #[default]
    Command1,
    Command2,
}

pub fn handle_command(args: MyCommandWithSubCommandsCmdArgs) -> anyhow::Result<()> {
    match args.get_command() {
        MySubCommand::Command1 => eprintln!("Execute Command 1 (default)"),
        MySubCommand::Command2 => eprintln!("Execute Command 2"),
    };
    Ok(())
}
