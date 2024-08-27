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
    match args.get_command() {
        TestSubCommand::Unit => run_unit(&args.target, &args),
        TestSubCommand::Integration => run_integration(&args.target, &args),
        TestSubCommand::All => TestSubCommand::iter()
            .filter(|c| *c != TestSubCommand::All)
            .try_for_each(|c| {
                handle_command(TestCmdArgs {
                    command: Some(c),
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                    threads: args.threads,
                    jobs: args.jobs,
                })
            }),
    }
}

fn push_optional_args(cmd_args: &mut Vec<String>, args: &TestCmdArgs) {
    // cargo options
    if let Some(jobs) = &args.jobs {
        cmd_args.extend(vec!["--jobs".to_string(), jobs.to_string()]);
    };
    // test harness options
    cmd_args.extend(vec!["--".to_string(), "--color=always".to_string()]);
    if let Some(threads) = &args.threads {
        cmd_args.extend(vec!["--test-threads".to_string(), threads.to_string()]);
    };
}

pub fn run_unit(target: &Target, args: &TestCmdArgs) -> Result<()> {
    match target {
        Target::Workspace => {
            info!("Workspace Unit Tests");
            let mut cmd_args = vec![
                "test",
                "--workspace",
                "--lib",
                "--bins",
                "--examples",
                "--color",
                "always",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
            push_optional_args(&mut cmd_args, args);
            run_process_for_workspace(
                "cargo",
                cmd_args.iter().map(String::as_str).collect(),
                &args.exclude,
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
                Some("Unit Tests"),
                "Workspace Unit Tests failed",
                Some("no library targets found"),
                Some("No library found to test for in workspace."),
            )?;
        }
        Target::Crates | Target::Examples => {
            let members = match target {
                Target::Crates => get_workspace_members(WorkspaceMemberType::Crate),
                Target::Examples => get_workspace_members(WorkspaceMemberType::Example),
                _ => unreachable!(),
            };

            for member in members {
                run_unit_test(&member, args)?;
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_unit(&t, args))?;
        }
    }
    anyhow::Ok(())
}

fn run_unit_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<(), anyhow::Error> {
    group!("Unit Tests: {}", member.name);
    let mut cmd_args = vec![
        "test",
        "--lib",
        "--bins",
        "--examples",
        "-p",
        &member.name,
        "--color=always",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
    push_optional_args(&mut cmd_args, args);
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect(),
        &args.exclude,
        &args.only,
        &format!("Failed to execute unit test for '{}'", &member.name),
        Some("no library targets found"),
        Some(&format!(
            "No library found to test for in the crate '{}'.",
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
            let mut cmd_args = vec![
                "test",
                "--workspace",
                "--test",
                "*",
                "--color",
                "always",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
            push_optional_args(&mut cmd_args, args);
            run_process_for_workspace(
                "cargo",
                cmd_args.iter().map(String::as_str).collect(),
                &args.exclude,
                Some(r".*target/[^/]+/deps/([^-\s]+)"),
                Some("Integration Tests"),
                "Workspace Integration Tests failed",
                Some("no test target matches pattern"),
                Some("No tests found matching the pattern `test_*` in workspace."),
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
                .try_for_each(|t| run_integration(&t, args))?;
        }
    }
    anyhow::Ok(())
}

fn run_integration_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<()> {
    group!("Integration Tests: {}", &member.name);
    let mut cmd_args = vec![
        "test",
        "--test",
        "*",
        "-p",
        &member.name,
        "--color",
        "always",
    ]
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
    push_optional_args(&mut cmd_args, args);
    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect(),
        &args.exclude,
        &args.only,
        &format!("Failed to execute integration test for '{}'", &member.name),
        Some("no test target matches pattern"),
        Some(&format!(
            "No tests found matching the pattern `test_*` for '{}'.",
            &member.name
        )),
    )?;
    endgroup!();
    anyhow::Ok(())
}
