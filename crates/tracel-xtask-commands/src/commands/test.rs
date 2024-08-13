use anyhow::Result;
use strum::IntoEnumIterator;

use crate::{
    commands::WARN_IGNORED_ONLY_ARGS,
    endgroup, group,
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMember, WorkspaceMemberType},
    },
};

use super::Target;

#[tracel_xtask_macros::declare_command_args(Target, TestSubCommand)]
pub struct TestCmdArgs {}

pub fn handle_command(args: TestCmdArgs) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{}", WARN_IGNORED_ONLY_ARGS);
    }
    match args.command {
        TestSubCommand::Unit => run_unit(&args.target, &args),
        TestSubCommand::Integration => run_integration(&args.target, &args),
        TestSubCommand::All => TestSubCommand::iter()
            .filter(|c| *c != TestSubCommand::All)
            .try_for_each(|c| {
                handle_command(TestCmdArgs {
                    command: c,
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                    threads: args.threads,
                })
            }),
    }
}

pub fn run_unit(target: &Target, args: &TestCmdArgs) -> Result<()> {
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
                &args.exclude,
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
                run_unit_test(&member, &args.exclude, &args.only)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_unit(&t, &args))?;
        }
    }
    anyhow::Ok(())
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
    anyhow::Ok(())
}

pub fn run_integration(target: &Target, args: &TestCmdArgs) -> anyhow::Result<()> {
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
                &args.exclude,
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
                run_integration_test(&member, args)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_integration(&t, &args))?;
        }
    }
    anyhow::Ok(())
}

fn run_integration_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<()> {
    group!("Integration Tests: {}", &member.name);
    let mut cmd_args = vec![
        "test",
        "--test",
        "test_*",
        "-p",
        &member.name,
        "--color",
        "always",
    ];
    let threads_str: String;
    if let Some(threads) = &args.threads {
        threads_str = threads.to_string();
        cmd_args.extend(vec!["--", "--test-threads", &threads_str]);
    }
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args,
        &args.exclude,
        &args.only,
        &format!("Failed to execute integration test for '{}'", &member.name),
        Some("no test target matches pattern"),
        Some(&format!(
            "No tests found matching the pattern `test_*` for '{}'",
            &member.name
        )),
    )?;
    endgroup!();
    anyhow::Ok(())
}
