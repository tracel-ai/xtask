use anyhow::{Ok, Result};
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::{process::{run_process_for_package, run_process_for_workspace}, workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType}},
};

use super::Target;

#[derive(Args, Clone)]
pub struct TestCmdArgs {
    /// Target to test for.
    #[arg(short, long, value_enum, default_value_t = Target::Workspace)]
    target: Target,
    /// Comma-separated list of excluded crates.
    #[arg(
        short = 'x',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub exclude: Vec<String>,
    /// Comma-separated list of crates to include exclusively.
    #[arg(
        short = 'n',
        long,
        value_name = "CRATE,CRATE,...",
        value_delimiter = ',',
        required = false
    )]
    pub only: Vec<String>,
    #[command(subcommand)]
    pub command: TestCommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum TestCommand {
    /// Run unit tests.
    Unit,
    /// Run integration tests.
    Integration,
    /// Run documentation tests.
    Documentation,
    /// Run all the checks.
    All,
}

pub fn handle_command(args: TestCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    match args.command {
        TestCommand::Unit => run_unit(&args.target, &args.exclude, &args.only),
        TestCommand::Integration => run_integration(&args.target, &args.exclude, &args.only),
        TestCommand::Documentation => run_documentation(&args.target, &args.exclude, &args.only),
        TestCommand::All => TestCommand::iter()
            .filter(|c| *c != TestCommand::All)
            .try_for_each(|c| {
                handle_command(TestCmdArgs {
                    command: c,
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                })
            }),
    }
}

pub(crate) fn run_unit(target: &Target, excluded: &Vec<String>, only: &Vec<String>) -> Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Unit Tests");
            run_process_for_workspace(
                "cargo",
                vec!["test", "--workspace", "--tests", "--color", "always"],
                excluded,
                "Workspace Unit Tests failed",
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
            )?;
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_unit_test(&member, excluded, only)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_unit(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_unit_test(member: &WorkspaceMember, excluded: &Vec<String>, only: &Vec<String>) -> Result<(), anyhow::Error> {
    group!("Unit Tests: {}", member.name);
    run_process_for_package(
        "cargo",
        &member.name,
        &vec![
            "test",
            "--lib",
            "--bins",
            "-p",
            &member.name,
            "--color=always",
            "--",
            "--color=always",
        ],
        excluded,
        only,
        &format!("Failed to execute unit test for '{}'", &member.name),
        Some("no library targets found"),
        Some(&format!("No library found to test for in the crate '{}'", &member.name)),
    )?;
    endgroup!();
    Ok(())
}

pub(crate) fn run_documentation(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> Result<()> {
    match target {
        Target::Workspace => {
            group!("Workspace Documentation Tests");
            run_process_for_workspace(
                "cargo",
                vec!["test", "--workspace", "--doc", "--color", "always"],
                excluded,
                "Workspace documentation test failed",
                Some(r".*Doc-tests\s([^-\s]+)$"),
            )?;
            endgroup!();
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_doc_test(&member, excluded, only)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_documentation(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_doc_test(member: &WorkspaceMember, excluded: &Vec<String>, only: &Vec<String>) -> Result<(), anyhow::Error> {
    group!("Doc Tests: {}", member.name);
    run_process_for_package(
        "cargo",
        &member.name,
        &vec!["test", "--doc", "-p", &member.name],
        excluded,
        only,
        &format!("Failed to execute documentation test for '{}'", &member.name),
        Some("no library targets found"),
        Some(&format!("No library found to test documentation for in the crate '{}'", &member.name)),
    )?;
    endgroup!();
    Ok(())
}

pub(crate) fn run_integration(
    target: &Target,
    excluded: &Vec<String>,
    only: &Vec<String>,
) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Integration Tests");
            run_process_for_workspace(
                "cargo",
                vec![
                    "test",
                    "--workspace",
                    "--test",
                    "test_*",
                    "--color",
                    "always",
                ],
                excluded,
                "Workspace Integration Tests failed",
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
            )?;
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_integration_test(&member, excluded, only)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_integration(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_integration_test(member: &WorkspaceMember, excluded: &Vec<String>, only: &Vec<String>) -> Result<()> {
    group!("Integration Tests: {}", &member.name);
    run_process_for_package(
        "cargo",
        &member.name,
        &vec![
            "test",
            "--test",
            "test_*",
            "-p",
            &member.name,
            "--color",
            "always",
        ],
        excluded,
        only,
        &format!("Failed to execute integration test for '{}'", &member.name),
        Some("no test target matches pattern"),
        Some(&format!("No tests found matching the pattern `test_*` for '{}'", &member.name)),
    )?;
    endgroup!();
    Ok(())
}
