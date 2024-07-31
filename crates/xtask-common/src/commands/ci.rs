use anyhow::Ok;
use clap::{Args, Subcommand};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator};

use crate::{
    commands::{WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS, WARN_IGNORED_ONLY_ARGS},
    endgroup, group,
    utils::{
        cargo::ensure_cargo_crate_is_installed,
        process::{run_process, run_process_for_package, run_process_for_workspace},
        workspace::{get_workspace_members, WorkspaceMemberType},
    },
};

use super::{
    check::run_compile,
    test::{run_documentation, run_integration, run_unit},
    Target,
};

#[derive(Args, Clone)]
pub struct CICmdArgs {
    /// Target to check for.
    #[arg(short, long, value_enum, default_value_t = Target::Workspace)]
    pub target: Target,
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
    pub command: CICommand,
}

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, Subcommand)]
#[strum(serialize_all = "lowercase")]
pub enum CICommand {
    /// Run all the checks.
    All,
    /// Run all tests.
    AllTests,
    /// Run audit command.
    Audit,
    /// Build the targets.
    Build,
    /// Compile the targets (does not write actual binaries).
    Compile,
    /// Run documentation tests.
    DocTests,
    /// Run format command.
    Format,
    /// Run integration tests.
    IntegrationTests,
    /// Run lint command.
    Lint,
    /// Report typos in source code.
    Typos,
    /// Run unit tests.
    UnitTests,
}

pub fn handle_command(args: CICmdArgs) -> anyhow::Result<()> {
    match args.command {
        CICommand::Build
        | CICommand::AllTests
        | CICommand::DocTests
        | CICommand::IntegrationTests
        | CICommand::UnitTests => {
            if args.target == Target::Workspace && !args.only.is_empty() {
                warn!("{}", WARN_IGNORED_ONLY_ARGS);
            }
        }
        _ => {
            if args.target == Target::Workspace
                && (!args.exclude.is_empty() || !args.only.is_empty())
            {
                warn!("{}", WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS);
            }
        }
    }

    match args.command {
        CICommand::Audit => run_audit(),
        CICommand::Build => run_build(&args.target, &args.exclude, &args.only),
        CICommand::Compile => run_compile(&args.target, &args.exclude, &args.only, None),
        CICommand::DocTests => run_documentation(&args.target, &args.exclude, &args.only),
        CICommand::Format => run_format(&args.target, &args.exclude, &args.only),
        CICommand::IntegrationTests => run_integration(&args.target, &args.exclude, &args.only),
        CICommand::Lint => run_lint(&args.target, &args.exclude, &args.only),
        CICommand::Typos => run_typos(),
        CICommand::UnitTests => run_unit(&args.target, &args.exclude, &args.only),
        CICommand::AllTests => run_all_tests(&args.target, &args.exclude, &args.only),
        CICommand::All => CICommand::iter()
            .filter(|c| *c != CICommand::All && *c != CICommand::AllTests)
            .try_for_each(|c| {
                handle_command(CICmdArgs {
                    command: c,
                    target: args.target.clone(),
                    exclude: args.exclude.clone(),
                    only: args.only.clone(),
                })
            }),
    }
}

fn run_audit() -> anyhow::Result<()> {
    group!("Audit Rust Dependencies");
    ensure_cargo_crate_is_installed("cargo-audit", Some("fix"), None, false)?;
    run_process(
        "cargo",
        &vec!["audit", "-q", "--color", "always"],
        "Audit check execution failed",
        true,
    )?;
    endgroup!();
    Ok(())
}

fn run_build(
    target: &Target,
    excluded: &[String],
    only: &[String],
) -> std::prelude::v1::Result<(), anyhow::Error> {
    match target {
        Target::Workspace => {
            group!("Build Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["build", "--workspace"],
                excluded,
                "Workspace build failed",
                None,
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
                group!("Build: {}", member.name);
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["build", "-p", &member.name],
                    excluded,
                    only,
                    &format!("Build command failed for {}", &member.name),
                    None,
                    None,
                )?;
                endgroup!();
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_build(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_format(target: &Target, excluded: &[String], only: &[String]) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Format Workspace");
            run_process_for_workspace(
                "cargo",
                vec!["fmt", "--check"],
                &[],
                "Workspace format failed",
                None,
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
                group!("Format: {}", member.name);
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec!["fmt", "--check", "-p", &member.name],
                    excluded,
                    only,
                    &format!("Format check execution failed for {}", &member.name),
                    None,
                    None,
                )?;
                endgroup!();
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_format(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_lint(target: &Target, excluded: &[String], only: &[String]) -> anyhow::Result<()> {
    match target {
        Target::Workspace => {
            group!("Lint Workspace");
            run_process_for_workspace(
                "cargo",
                vec![
                    "clippy",
                    "--no-deps",
                    "--color=always",
                    "--",
                    "--deny",
                    "warnings",
                ],
                &[],
                "Workspace lint failed",
                None,
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
                group!("Lint: {}", member.name);
                run_process_for_package(
                    "cargo",
                    &member.name,
                    &vec![
                        "clippy",
                        "--no-deps",
                        "--color=always",
                        "-p",
                        &member.name,
                        "--",
                        "--deny",
                        "warnings",
                    ],
                    excluded,
                    only,
                    &format!("Lint fix execution failed for {}", &member.name),
                    None,
                    None,
                )?;
                endgroup!();
            }
        }
        Target::AllPackages => {
            Target::iter()
                .filter(|t| *t != Target::AllPackages && *t != Target::Workspace)
                .try_for_each(|t| run_lint(&t, excluded, only))?;
        }
    }
    Ok(())
}

fn run_typos() -> anyhow::Result<()> {
    group!("Typos");
    run_process(
        "typos",
        &vec!["--diff", "--color", "always"],
        "Typos check execution failed",
        true,
    )?;
    endgroup!();
    Ok(())
}

fn run_all_tests(target: &Target, excluded: &[String], only: &[String]) -> anyhow::Result<()> {
    run_unit(target, excluded, only)?;
    run_integration(target, excluded, only)?;
    run_documentation(target, excluded, only)?;
    Ok(())
}
