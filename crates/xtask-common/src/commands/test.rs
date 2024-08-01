use anyhow::{Ok, Result};
use clap::Subcommand;
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType},
    },
};

use super::Target;

#[xtask_macros::arguments(target, exclude, only)]
pub struct TestCmdArgs {
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

pub(crate) fn run_unit(target: &Target, excluded: &[String], only: &[String]) -> Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Unit Tests");
            run_process_for_workspace(
                "cargo",
                vec![
                    "test",
                    "--workspace",
                    "--lib",
                    "--bins",
                    "--examples",
                    "--color",
                    "always",
                ],
                excluded,
                "Workspace Unit Tests failed",
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
                Some("Unit Tests"),
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

fn run_unit_test(
    member: &WorkspaceMember,
    excluded: &[String],
    only: &[String],
) -> Result<(), anyhow::Error> {
    group!("Unit Tests: {}", member.name);
    run_process_for_package(
        "cargo",
        &member.name,
        &vec![
            "test",
            "--lib",
            "--bins",
            "--examples",
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
        Some(&format!(
            "No library found to test for in the crate '{}'",
            &member.name
        )),
    )?;
    endgroup!();
    Ok(())
}

pub(crate) fn run_integration(
    target: &Target,
    excluded: &[String],
    only: &[String],
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
                Some("Integration Tests"),
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

fn run_integration_test(
    member: &WorkspaceMember,
    excluded: &[String],
    only: &[String],
) -> Result<()> {
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
        Some(&format!(
            "No tests found matching the pattern `test_*` for '{}'",
            &member.name
        )),
    )?;
    endgroup!();
    Ok(())
}
