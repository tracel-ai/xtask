mod commands;

use tracel_xtask_commands::{anyhow, clap, commands::*, init_xtask};

#[tracel_xtask_macros::commands(
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
        Command::Bump(args) => bump::handle_command(args),
        Command::Check(args) => check::handle_command(args),
        Command::Compile(args) => compile::handle_command(args),
        Command::Coverage(args) => coverage::handle_command(args),
        Command::Dependencies(args) => dependencies::handle_command(args),
        Command::Doc(args) => doc::handle_command(args),
        Command::Fix(args) => fix::handle_command(args, None),
        Command::Publish(args) => publish::handle_command(args),
        Command::Test(args) => test::handle_command(args),
        Command::Vulnerabilities(args) => vulnerabilities::handle_command(args),

        // Implementation of a new command that is not part of xtask-common
        Command::Foo(args) => commands::foo::handle_commands(args),
    }?;
    Ok(())
}
