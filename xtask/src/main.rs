mod commands;

use tracel_xtask_commands::prelude::*;

#[macros::commands(
    Bump,
    Check,
    Compile,
    Coverage,
    Doc,
    Dependencies,
    Fix,
    Publish,
    Test,
    Vulnerabilities
)]
pub enum Command {
    /// Example of an additional command
    Foo(commands::foo::FooCmdArgs),
    /// Extended Build command
    Build(commands::build::ExtendedBuildCmdArgs),
}

fn main() -> anyhow::Result<()> {
    let args = init_xtask::<Command>()?;
    match args.command {
        // From common_xtask
        // You can easily insert specific pre-processing for each command if required by your repository
        Command::Build(args) => commands::build::handle_command(args),
        Command::Bump(args) => base_commands::bump::handle_command(args),
        Command::Check(args) => base_commands::check::handle_command(args),
        Command::Compile(args) => base_commands::compile::handle_command(args),
        Command::Coverage(args) => base_commands::coverage::handle_command(args),
        Command::Dependencies(args) => base_commands::dependencies::handle_command(args),
        Command::Doc(args) => base_commands::doc::handle_command(args),
        Command::Fix(args) => base_commands::fix::handle_command(args, None),
        Command::Publish(args) => base_commands::publish::handle_command(args),
        Command::Test(args) => base_commands::test::handle_command(args),
        Command::Vulnerabilities(args) => base_commands::vulnerabilities::handle_command(args),

        // Implementation of a new command that is not part of xtask-common
        Command::Foo(args) => commands::foo::handle_commands(args),
    }?;
    Ok(())
}
