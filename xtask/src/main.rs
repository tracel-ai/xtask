mod commands;

use tracel_xtask_commands::prelude::*;

#[macro_use]
extern crate log;

#[macros::commands(
    Bump,
    Check,
    Compile,
    Coverage,
    Doc,
    Dependencies,
    Publish,
    Test,
    Vulnerabilities
)]
pub enum Command {
    /// Example of a new command with support of base Target
    NewCommand(commands::new_command::NewCommandCmdArgs),
    /// Example of extended Build command with an additional target called 'Frontend'
    Build(commands::build::ExtendedBuildCmdArgs),
    /// Comprehensive example of an extended Fix command with an additional target and subcommand
    Fix(commands::fix::ExtendedFixCmdArgs),
}

fn main() -> anyhow::Result<()> {
    let args = init_xtask::<Command>()?;
    match args.command {
        Command::Build(args) => commands::build::handle_command(args),
        Command::Bump(args) => base_commands::bump::handle_command(args),
        Command::Check(args) => base_commands::check::handle_command(args),
        Command::Compile(args) => base_commands::compile::handle_command(args),
        Command::Coverage(args) => base_commands::coverage::handle_command(args),
        Command::Dependencies(args) => base_commands::dependencies::handle_command(args),
        Command::Doc(args) => base_commands::doc::handle_command(args),
        Command::Fix(args) => commands::fix::handle_command(args, None),
        Command::NewCommand(args) => commands::new_command::handle_commands(args),
        Command::Publish(args) => base_commands::publish::handle_command(args),
        Command::Test(args) => base_commands::test::handle_command(args),
        Command::Vulnerabilities(args) => base_commands::vulnerabilities::handle_command(args),
    }?;
    Ok(())
}
