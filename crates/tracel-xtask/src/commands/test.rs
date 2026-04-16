use std::collections::HashMap;

use anyhow::Result;
use clap::ValueEnum;
use strum::IntoEnumIterator;

use crate::{
    commands::{CARGO_NIGHTLY_MSG, WARN_IGNORED_ONLY_ARGS},
    endgroup,
    environment::EnvironmentName,
    group,
    prelude::{Context, Environment, is_current_toolchain_nightly, rustup_add_component},
    utils::{
        process::{run_process_for_package, run_process_for_workspace},
        workspace::{WorkspaceMember, WorkspaceMemberType, get_workspace_members},
    },
};

use super::Target;

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MiriMode {
    #[default]
    All,
    UbOnly,
}

#[tracel_xtask_macros::declare_command_args(Target, TestSubCommand)]
pub struct TestCmdArgs {}

pub fn handle_command(args: TestCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    if args.target == Target::Workspace && !args.only.is_empty() {
        warn!("{WARN_IGNORED_ONLY_ARGS}");
    }
    if !check_environment(&args, &env) {
        std::process::exit(1);
    }
    match args.get_command() {
        TestSubCommand::Unit => run_unit(&args.target, &args),
        TestSubCommand::Integration => run_integration(&args.target, &args),
        TestSubCommand::All => TestSubCommand::iter()
            .filter(|command| *command != TestSubCommand::All)
            .try_for_each(|command| {
                handle_command(
                    TestCmdArgs {
                        command: Some(command),
                        target: args.target.clone(),
                        exclude: args.exclude.clone(),
                        only: args.only.clone(),
                        threads: args.threads,
                        test: args.test.clone(),
                        jobs: args.jobs,
                        force: args.force,
                        features: args.features.clone(),
                        no_default_features: args.no_default_features,
                        no_capture: args.no_capture,
                        miri: args.miri,
                        release: args.release,
                    },
                    env.clone(),
                    _ctx.clone(),
                )
            }),
    }
}

/// Return true if the environment is OK.
/// Prevents from running tests in production unless the `force` flag is set.
pub fn check_environment(args: &TestCmdArgs, env: &Environment) -> bool {
    if env.name == EnvironmentName::Production {
        if args.force {
            warn!("Force running tests in production (--force argument is set)");
            true
        } else {
            info!("Abort tests to avoid running them in production!");
            false
        }
    } else {
        true
    }
}

fn push_test_command_prefix(cmd_args: &mut Vec<String>, args: &TestCmdArgs) {
    if args.miri.is_some() {
        cmd_args.push("miri".to_string());
    }
    cmd_args.push("test".to_string());
}

fn push_cargo_optional_args(cmd_args: &mut Vec<String>, args: &TestCmdArgs) {
    if let Some(jobs) = args.jobs {
        cmd_args.extend(["--jobs".to_string(), jobs.to_string()]);
    }

    if let Some(features) = &args.features {
        if !features.is_empty() {
            cmd_args.extend(["--features".to_string(), features.join(",")]);
        }
    }

    if args.release {
        cmd_args.push("--release".to_string());
    }

    if args.no_default_features {
        cmd_args.push("--no-default-features".to_string());
    }
}

fn push_test_harness_args(cmd_args: &mut Vec<String>, args: &TestCmdArgs) {
    cmd_args.extend(["--".to_string(), "--color=always".to_string()]);

    if let Some(threads) = args.threads {
        cmd_args.extend(["--test-threads".to_string(), threads.to_string()]);
    }

    if args.no_capture {
        cmd_args.push("--nocapture".to_string());
    }
}

fn build_miri_env(args: &TestCmdArgs) -> Option<HashMap<&'static str, &'static str>> {
    match args.miri {
        Some(MiriMode::UbOnly) => Some(HashMap::from([("MIRIFLAGS", "-Zmiri-ignore-leaks")])),
        _ => None,
    }
}

pub fn run_unit(target: &Target, args: &TestCmdArgs) -> Result<()> {
    if args.miri.is_some() {
        ensure_miri_ready()?;
    }

    match target {
        Target::Workspace => {
            info!("Workspace Unit Tests");

            let test = args.test.as_deref().unwrap_or("");
            let mut cmd_args = Vec::new();

            push_test_command_prefix(&mut cmd_args, args);
            cmd_args.extend([
                "--workspace".to_string(),
                "--lib".to_string(),
                "--bins".to_string(),
                "--examples".to_string(),
            ]);

            if !test.is_empty() {
                cmd_args.push(test.to_string());
            }

            push_cargo_optional_args(&mut cmd_args, args);
            push_test_harness_args(&mut cmd_args, args);

            run_process_for_workspace(
                "cargo",
                &cmd_args.iter().map(String::as_str).collect::<Vec<_>>(),
                build_miri_env(args),
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
                .filter(|target| *target != Target::AllPackages && *target != Target::Workspace)
                .try_for_each(|target| run_unit(&target, args))?;
        }
    }

    Ok(())
}

pub fn run_unit_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<()> {
    group!("Unit Tests: {}", member.name);

    let test = args.test.as_deref().unwrap_or("");
    let mut cmd_args = Vec::new();

    push_test_command_prefix(&mut cmd_args, args);

    if !test.is_empty() {
        cmd_args.push(test.to_string());
    }

    cmd_args.extend([
        "--lib".to_string(),
        "--bins".to_string(),
        "--examples".to_string(),
        "-p".to_string(),
        member.name.clone(),
    ]);

    push_cargo_optional_args(&mut cmd_args, args);
    push_test_harness_args(&mut cmd_args, args);

    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect::<Vec<_>>(),
        build_miri_env(args),
        &args.exclude,
        &args.only,
        &format!("Failed to execute unit test for '{}'", member.name),
        Some("no library targets found"),
        Some(&format!(
            "No library found to test for in the crate '{}'.",
            member.name
        )),
    )?;

    endgroup!();
    Ok(())
}

pub fn run_integration(target: &Target, args: &TestCmdArgs) -> Result<()> {
    if args.miri.is_some() {
        ensure_miri_ready()?;
    }

    match target {
        Target::Workspace => {
            info!("Workspace Integration Tests");

            let test = args.test.as_deref().unwrap_or("*");
            let mut cmd_args = Vec::new();

            push_test_command_prefix(&mut cmd_args, args);
            cmd_args.extend([
                "--workspace".to_string(),
                "--test".to_string(),
                test.to_string(),
            ]);

            push_cargo_optional_args(&mut cmd_args, args);
            push_test_harness_args(&mut cmd_args, args);

            run_process_for_workspace(
                "cargo",
                &cmd_args.iter().map(String::as_str).collect::<Vec<_>>(),
                build_miri_env(args),
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
                .filter(|target| *target != Target::AllPackages && *target != Target::Workspace)
                .try_for_each(|target| run_integration(&target, args))?;
        }
    }

    Ok(())
}

fn run_integration_test(member: &WorkspaceMember, args: &TestCmdArgs) -> Result<()> {
    group!("Integration Tests: {}", member.name);

    let mut cmd_args = Vec::new();

    push_test_command_prefix(&mut cmd_args, args);
    cmd_args.extend([
        "--test".to_string(),
        "*".to_string(),
        "-p".to_string(),
        member.name.clone(),
    ]);

    push_cargo_optional_args(&mut cmd_args, args);
    push_test_harness_args(&mut cmd_args, args);

    run_process_for_package(
        "cargo",
        &member.name,
        &cmd_args.iter().map(String::as_str).collect::<Vec<_>>(),
        build_miri_env(args),
        &args.exclude,
        &args.only,
        &format!("Failed to execute integration test for '{}'", member.name),
        Some("no test target matches pattern"),
        Some(&format!(
            "No integration tests found for '{}'.",
            member.name
        )),
    )?;

    endgroup!();
    Ok(())
}

fn ensure_miri_ready() -> Result<()> {
    if !is_current_toolchain_nightly() {
        anyhow::bail!("{CARGO_NIGHTLY_MSG}");
    }

    rustup_add_component("miri")?;
    rustup_add_component("rust-src")?;
    Ok(())
}
