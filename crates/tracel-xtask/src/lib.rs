pub mod commands;
pub mod context;
mod versions;
// Re-export utility macros for backward compatibility when requested.
#[cfg(feature = "utils-cleanup")]
pub use crate::utils::handle_cleanup;
#[cfg(feature = "utils-cleanup")]
pub use crate::utils::register_cleanup;
pub use tracel_xtask_utils as utils;

// re-exports
pub mod prelude {
    pub use anyhow;
    pub use clap;
    #[cfg(any(
        feature = "aws-container",
        feature = "aws-secrets",
        feature = "gcp-secrets"
    ))]
    pub use serde_json;
    #[cfg(any(feature = "infra", feature = "publish"))]
    pub use ureq;

    pub mod macros {
        pub use tracel_xtask_macros::base_commands;
        pub use tracel_xtask_macros::declare_command_args;
        pub use tracel_xtask_macros::extend_command_args;
        pub use tracel_xtask_macros::extend_subcommands;
        pub use tracel_xtask_macros::extend_targets;
    }

    pub use crate::XtaskArgs;
    pub use crate::commands as base_commands;
    pub use crate::commands::Target;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerBuildSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerHostSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerListSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerLogsSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerPromoteSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerPullSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerPushSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerRollbackSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerRolloutSubCmdArgs;
    #[cfg(feature = "aws-container")]
    pub use crate::commands::aws_container::AwsContainerSubCommand;
    #[cfg(feature = "aws-secrets")]
    pub use crate::commands::aws_secrets::AwsSecretsCmdArgs;
    #[cfg(feature = "aws-secrets")]
    pub use crate::commands::aws_secrets::AwsSecretsSubCommand;
    #[cfg(feature = "build")]
    pub use crate::commands::build::BuildCmdArgs;
    #[cfg(feature = "bump")]
    pub use crate::commands::bump::BumpCmdArgs;
    #[cfg(feature = "bump")]
    pub use crate::commands::bump::BumpSubCommand;
    #[cfg(feature = "check")]
    pub use crate::commands::check::CheckCmdArgs;
    #[cfg(feature = "check")]
    pub use crate::commands::check::CheckSubCommand;
    #[cfg(feature = "clean")]
    pub use crate::commands::clean::CleanCmdArgs;
    #[cfg(feature = "compile")]
    pub use crate::commands::compile::CompileCmdArgs;
    #[cfg(feature = "coverage")]
    pub use crate::commands::coverage::CoverageCmdArgs;
    #[cfg(feature = "dependencies")]
    pub use crate::commands::dependencies::DependenciesCmdArgs;
    #[cfg(feature = "dependencies")]
    pub use crate::commands::dependencies::DependenciesSubCommand;
    #[cfg(feature = "doc")]
    pub use crate::commands::doc::DocCmdArgs;
    #[cfg(feature = "doc")]
    pub use crate::commands::doc::DocSubCommand;
    #[cfg(feature = "docker-compose")]
    pub use crate::commands::docker_compose::DockerComposeCmdArgs;
    #[cfg(feature = "docker-compose")]
    pub use crate::commands::docker_compose::DockerComposeSubCommand;
    #[cfg(feature = "fix")]
    pub use crate::commands::fix::FixCmdArgs;
    #[cfg(feature = "fix")]
    pub use crate::commands::fix::FixSubCommand;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerBuildSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerListSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerPromoteSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerPullSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerPushSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerRollbackSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerRolloutSubCmdArgs;
    #[cfg(feature = "gcp-container")]
    pub use crate::commands::gcp_container::GcpContainerSubCommand;
    #[cfg(feature = "host")]
    pub use crate::commands::host::HostCmdArgs;
    #[cfg(feature = "host")]
    pub use crate::commands::host::HostConnectSubCmdArgs;
    #[cfg(feature = "host")]
    pub use crate::commands::host::HostPrivateIpSubCmdArgs;
    #[cfg(feature = "host")]
    pub use crate::commands::host::HostSubCommand;
    #[cfg(feature = "icons")]
    pub use crate::commands::icons::IconsCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageBuildSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageCleanSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageHostSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageListSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImagePromoteSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageRollbackSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageRolloutSubCmdArgs;
    #[cfg(feature = "image")]
    pub use crate::commands::image::ImageSubCommand;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraApplySubCmdArgs;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraCmdArgs;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraDestroySubCmdArgs;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraOutputSubCmdArgs;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraProvidersSubCmdArgs;
    #[cfg(feature = "infra")]
    pub use crate::commands::infra::InfraSubCommand;
    #[cfg(feature = "publish")]
    pub use crate::commands::publish::PublishCmdArgs;
    #[cfg(feature = "test")]
    pub use crate::commands::test::MiriMode;
    #[cfg(feature = "test")]
    pub use crate::commands::test::TestCmdArgs;
    #[cfg(feature = "test")]
    pub use crate::commands::test::TestSubCommand;
    #[cfg(feature = "validate")]
    pub use crate::commands::validate::ValidateCmdArgs;
    #[cfg(feature = "vulnerabilities")]
    pub use crate::commands::vulnerabilities::VulnerabilitiesCmdArgs;
    #[cfg(feature = "vulnerabilities")]
    pub use crate::commands::vulnerabilities::VulnerabilitiesSubCommand;
    pub use crate::context::Context;
    #[cfg(feature = "utils-cleanup")]
    pub use crate::handle_cleanup;
    pub use crate::init_xtask;
    pub use crate::parse_args;
    #[cfg(feature = "utils-cleanup")]
    pub use crate::register_cleanup;
    pub use crate::utils as base_utils;
    #[cfg(any(
        feature = "aws-container",
        feature = "aws-secrets",
        feature = "host",
        feature = "image",
        feature = "utils-aws",
        feature = "utils-aws-asg-instance-picker",
        feature = "utils-aws-cli",
        feature = "utils-aws-ec2-tag-instance-picker",
        feature = "utils-aws-images",
        feature = "utils-aws-instance-logs",
        feature = "utils-aws-instance-system-log",
        feature = "utils-aws-naming",
        feature = "utils-aws-regions"
    ))]
    pub use crate::utils::aws;
    #[cfg(feature = "utils-helpers")]
    pub use crate::utils::build_helpers;
    #[cfg(feature = "utils-cargo")]
    pub use crate::utils::cargo::ensure_cargo_crate_is_installed;
    #[cfg(feature = "utils-cleanup")]
    pub use crate::utils::cleanup::CLEANUP_HANDLER;
    pub use crate::utils::endgroup;
    pub use crate::utils::environment::Environment;
    pub use crate::utils::environment::EnvironmentIndex;
    pub use crate::utils::environment::EnvironmentName;
    pub use crate::utils::environment::ExplicitIndex;
    #[cfg(any(
        feature = "gcp-container",
        feature = "gcp-secrets",
        feature = "utils-gcp",
        feature = "utils-gcp-cli",
        feature = "utils-gcp-naming",
        feature = "utils-gcp-regions"
    ))]
    pub use crate::utils::gcp;
    pub use crate::utils::git;
    pub use crate::utils::group;
    pub use crate::utils::group_info;
    #[cfg(feature = "utils-process")]
    pub use crate::utils::process;
    #[cfg(feature = "utils-process")]
    pub use crate::utils::process::random_port;
    #[cfg(feature = "utils-process")]
    pub use crate::utils::process::run_process;
    #[cfg(feature = "utils-process")]
    pub use crate::utils::process::run_process_for_package;
    #[cfg(feature = "utils-process")]
    pub use crate::utils::process::run_process_for_workspace;
    #[cfg(feature = "utils-prompt")]
    pub use crate::utils::prompt::ask_once;
    #[cfg(feature = "utils-rustup")]
    pub use crate::utils::rustup::is_current_toolchain_nightly;
    #[cfg(feature = "utils-rustup")]
    pub use crate::utils::rustup::rustup_add_component;
    #[cfg(feature = "utils-rustup")]
    pub use crate::utils::rustup::rustup_add_target;
    #[cfg(feature = "utils-rustup")]
    pub use crate::utils::rustup::rustup_get_installed_targets;
    #[cfg(feature = "utils-terraform")]
    pub use crate::utils::terraform;
    #[cfg(feature = "utils-time")]
    pub use crate::utils::time::format_duration;
    // does not re-export strum has it is incompatible with strum macros expansions
}

use std::fmt::Display;

use crate::context::Context;
use crate::utils::{
    environment::{Environment, EnvironmentName},
    group_info,
    logging::init_logger,
};
use clap::{CommandFactory as _, FromArgMatches as _};

const HELP_PREFIX: &str = "💡 Help";
const XTASK_CLI_ENVVAR: &str = "XTASK_CLI";

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct XtaskArgs<C: clap::Subcommand + Display> {
    /// Enable code coverage for Rust code if available (see coverage command for more info).
    #[arg(long)]
    pub enable_coverage: bool,
    /// Set environment.
    #[arg(short = 'e', long = "env_name", default_value_t = EnvironmentName::default())]
    pub environment_name: EnvironmentName,
    /// Set environment index, must be between 1 and 255 inclusive
    #[arg(short = 'i', long = "env_index", default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=255))]
    pub environment_index: u8,
    /// Set context.
    #[arg(short = 'c', long, default_value_t = Context::default())]
    pub context: Context,
    #[command(subcommand)]
    pub command: C,
}

pub fn parse_args<C>() -> anyhow::Result<(XtaskArgs<C>, Environment)>
where
    C: clap::Subcommand + std::fmt::Display,
{
    // init logs early
    init_logger().init();
    // Let clap do its normal parsing/help/version handling but with our help screen prefix
    let mut cmd = XtaskArgs::<C>::command();
    if std::env::var(XTASK_CLI_ENVVAR).is_ok() {
        add_help_prefix(&mut cmd);
    }
    let matches = cmd.get_matches();
    let args = XtaskArgs::<C>::from_arg_matches(&matches)?;
    let env = Environment::new(args.environment_name.clone(), args.environment_index);
    Ok((args, env))
}

pub fn init_xtask<C: clap::Subcommand + Display>(
    config: (XtaskArgs<C>, Environment),
) -> anyhow::Result<(XtaskArgs<C>, Environment)> {
    let args = config.0;
    let env = config.1;
    if std::env::var(XTASK_CLI_ENVVAR).is_ok() {
        eprintln!("{} {}", add_command_prefix(), args.command);
    }
    group_info!("Environment: {}", env.long());
    env.load(None)?;
    group_info!("Context: {}", args.context);
    // code coverage
    if args.enable_coverage {
        group_info!("Enabling coverage support...");
        setup_coverage()?;
    }
    Ok((args, env))
}

fn setup_coverage() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("RUSTFLAGS", "-Cinstrument-coverage");
        std::env::set_var("LLVM_PROFILE_FILE", "burn-%p-%m.profraw");
    }
    Ok(())
}

fn add_command_prefix() -> &'static str {
    if is_current_toolchain_nightly() {
        "⚡️🌙"
    } else {
        "⚡️"
    }
}

fn is_current_toolchain_nightly() -> bool {
    if let Ok(toolchain) = std::env::var("RUSTUP_TOOLCHAIN") {
        let toolchain = toolchain.trim();
        if toolchain == "nightly" || toolchain.starts_with("nightly-") {
            return true;
        }
    }

    std::process::Command::new("rustup")
        .args(["show", "active-toolchain"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .is_some_and(|toolchain| toolchain == "nightly" || toolchain.starts_with("nightly-"))
}

fn add_help_prefix(cmd: &mut clap::Command) {
    let mut owned = std::mem::take(cmd);
    owned = owned.before_help(HELP_PREFIX).before_long_help(HELP_PREFIX);
    // Recurse into subcommands to append the help prefix
    for sub in owned.get_subcommands_mut() {
        add_help_prefix(sub);
    }
    *cmd = owned;
}
