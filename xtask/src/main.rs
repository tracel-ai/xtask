mod commands;

#[macro_use]
extern crate log;

use std::time::Instant;
use xtask_common::{
    anyhow,
    clap::{self, Parser},
    commands::*,
    init_xtask,
    utils::time::format_duration,
};

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct XtaskArgs {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
pub enum Command {
    // From common_xtask
    /// Bump the version of all crates to be published
    Bump(bump::BumpCmdArgs),
    /// Runs checks and fix issues (used for development purposes)
    Check(check::CheckCmdArgs),
    /// Runs checks for Continous Integration
    CI(ci::CICmdArgs),
    /// Run the specified dependencies check locally
    Dependencies(dependencies::DependenciesCmdArgs),
    /// Publish a crate to crates.io
    Publish(publish::PublishCmdArgs),
    /// Runs all tests and checks that should pass before opening a Pull Request.
    PullRequestChecks,
    /// Runs tests.
    Test(test::TestCmdArgs),
    /// Run the specified vulnerability check locally. These commands must be called with 'cargo +nightly'.
    Vulnerabilities(vulnerabilities::VulnerabilitiesCmdArgs),

    // Additional commands specific to this repository
    /// Print a message
    Foo,
}

fn main() -> anyhow::Result<()> {
    init_xtask();
    let args = XtaskArgs::parse();

    let start = Instant::now();
    match args.command {
        // From common_xtask
        Command::Bump(args) => bump::handle_command(args),
        Command::Check(args) => check::handle_command(args, None),
        Command::CI(args) => ci::handle_command(args),
        Command::Dependencies(args) => dependencies::handle_command(args),
        Command::Publish(args) => publish::handle_command(args),
        Command::PullRequestChecks => pull_request_checks::handle_command(),
        Command::Test(args) => test::handle_command(args),
        Command::Vulnerabilities(args) => vulnerabilities::handle_command(args),

        // Specific commands
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
