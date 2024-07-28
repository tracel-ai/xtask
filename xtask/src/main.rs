mod commands;

#[macro_use]
extern crate log;

use std::time::Instant;
use xtask_common::{anyhow, clap, commands::*, init_xtask, utils::time::format_duration};

#[xtask_macros::commands(
    Bump,
    Check,
    CI,
    Coverage,
    Doc,
    Dependencies,
    Publish,
    PullRequestChecks,
    Test,
    Vulnerabilities
)]
pub enum Command {
    /// Print a message
    Foo,
}

fn main() -> anyhow::Result<()> {
    let start = Instant::now();

    let args = init_xtask::<Command>()?;
    match args.command {
        // From common_xtask
        // You can easily insert specific pre-processing for each command if required by your repository
        Command::Bump(args) => bump::handle_command(args),
        Command::Check(args) => check::handle_command(args, None),
        Command::CI(args) => ci::handle_command(args),
        Command::Coverage(args) => coverage::handle_command(args),
        Command::Dependencies(args) => dependencies::handle_command(args),
        Command::Doc(args) => doc::handle_command(args),
        Command::Publish(args) => publish::handle_command(args),
        Command::PullRequestChecks => pull_request_checks::handle_command(),
        Command::Test(args) => test::handle_command(args),
        Command::Vulnerabilities(args) => vulnerabilities::handle_command(args),

        // Implementation of new commands for your repository
        Command::Foo => {
            println!("Custom command foo");
            Ok(())
        }
    }?;

    let duration = start.elapsed();
    info!(
        "\x1B[32;1mTime elapsed for the current execution: {}\x1B[0m",
        format_duration(&duration)
    );
    Ok(())
}
