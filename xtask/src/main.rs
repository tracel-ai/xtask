mod commands;

#[macro_use]
extern crate log;

use std::time::Instant;
use xtask_common::{anyhow, clap, commands::*, init_xtask, utils::time::format_duration};

#[derive(clap::Subcommand)]
pub enum Command {
    // From common_xtask
    // For now they need to be manually declared in each repository using xtask-common
    // You only need to declare the command that you are effectively using
    // TODO see if a derive macro could generate this code
    /// Bump the version of all crates to be published
    Bump(bump::BumpCmdArgs),
    /// Runs checks and fix issues (used for development purposes)
    Check(check::CheckCmdArgs),
    /// Runs checks for Continuous Integration
    CI(ci::CICmdArgs),
    /// Install and run coverage tools
    Coverage(coverage::CoverageCmdArgs),
    /// Build documentation
    Doc(doc::DocCmdArgs),
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

    // Example of how to add new commands specific to your repository
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
