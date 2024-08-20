use tracel_xtask::prelude::*;

#[macros::declare_command_args(Target, MySubcommand)]
struct MyCommandWithSubCommandsCmdArgs {}

#[derive(
    strum::EnumString, strum::EnumIter, strum::Display, Clone, PartialEq, clap::Subcommand, Default,
)]
#[strum(serialize_all = "lowercase")]
pub enum MySubcommand {
    #[default]
    Command1,
    Command2,
}

pub fn handle_command(args: MyCommandWithSubCommandsCmdArgs) -> anyhow::Result<()> {
    match args.get_command() {
        MySubcommand::Command1 => println!("Execute Command 1 (default)"),
        MySubcommand::Command2 => println!("Execute Command 2"),
    };
    Ok(())
}
