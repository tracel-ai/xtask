pub mod commands;
pub mod context;
pub mod environment;
pub mod logging;
pub mod utils;
mod versions;

// re-exports
pub mod prelude {
    pub use anyhow;
    pub use clap;
    pub use derive_more;
    pub use env_logger;
    pub use rand;
    pub use serde_json;
    pub use tracing_subscriber;

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
    pub use crate::commands::build::BuildCmdArgs;
    pub use crate::commands::bump::BumpCmdArgs;
    pub use crate::commands::bump::BumpSubCommand;
    pub use crate::commands::check::CheckCmdArgs;
    pub use crate::commands::check::CheckSubCommand;
    pub use crate::commands::compile::CompileCmdArgs;
    pub use crate::commands::container::ContainerCmdArgs;
    pub use crate::commands::container::ContainerSubCommand;
    pub use crate::commands::coverage::CoverageCmdArgs;
    pub use crate::commands::dependencies::DependenciesCmdArgs;
    pub use crate::commands::dependencies::DependenciesSubCommand;
    pub use crate::commands::doc::DocCmdArgs;
    pub use crate::commands::doc::DocSubCommand;
    pub use crate::commands::docker_compose::DockerComposeCmdArgs;
    pub use crate::commands::docker_compose::DockerComposeSubCommand;
    pub use crate::commands::fix::FixCmdArgs;
    pub use crate::commands::fix::FixSubCommand;
    pub use crate::commands::host::HostCmdArgs;
    pub use crate::commands::host::HostSubCommand;
    pub use crate::commands::infra::InfraCmdArgs;
    pub use crate::commands::infra::InfraSubCommand;
    pub use crate::commands::publish::PublishCmdArgs;
    pub use crate::commands::secrets::SecretsCmdArgs;
    pub use crate::commands::secrets::SecretsSubCommand;
    pub use crate::commands::test::TestCmdArgs;
    pub use crate::commands::test::TestSubCommand;
    pub use crate::commands::validate::ValidateCmdArgs;
    pub use crate::commands::vulnerabilities::VulnerabilitiesCmdArgs;
    pub use crate::commands::vulnerabilities::VulnerabilitiesSubCommand;
    pub use crate::context::Context;
    pub use crate::endgroup;
    pub use crate::environment::Environment;
    pub use crate::environment::EnvironmentIndex;
    pub use crate::environment::EnvironmentName;
    pub use crate::environment::ExplicitIndex;
    pub use crate::group;
    pub use crate::group_info;
    pub use crate::handle_cleanup;
    pub use crate::init_xtask;
    pub use crate::parse_args;
    pub use crate::register_cleanup;
    pub use crate::utils::aws;
    pub use crate::utils::cargo::ensure_cargo_crate_is_installed;
    pub use crate::utils::cleanup::CLEANUP_HANDLER;
    pub use crate::utils::git;
    pub use crate::utils::helpers;
    pub use crate::utils::process::random_port;
    pub use crate::utils::process::run_process;
    pub use crate::utils::process::run_process_for_package;
    pub use crate::utils::process::run_process_for_workspace;
    pub use crate::utils::prompt::ask_once;
    pub use crate::utils::rustup::is_current_toolchain_nightly;
    pub use crate::utils::rustup::rustup_add_component;
    pub use crate::utils::rustup::rustup_add_target;
    pub use crate::utils::rustup::rustup_get_installed_targets;
    pub use crate::utils::terraform;
    pub use crate::utils::time::format_duration;
    // does not re-export strum has it is incompatible with strum macros expansions
}

use environment::EnvironmentName;
use prelude::Environment;

use crate::context::Context;
use crate::logging::init_logger;

#[macro_use]
extern crate log;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct XtaskArgs<C: clap::Subcommand> {
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

pub fn parse_args<C: clap::Subcommand>() -> anyhow::Result<(XtaskArgs<C>, Environment)> {
    // init logs early
    init_logger().init();
    let args = <XtaskArgs<C> as clap::Parser>::parse();
    let env = Environment::new(args.environment_name.clone(), args.environment_index);
    Ok((args, env))
}

pub fn init_xtask<C: clap::Subcommand>(
    config: (XtaskArgs<C>, Environment),
) -> anyhow::Result<(XtaskArgs<C>, Environment)> {
    let args = config.0;
    let env = config.1;
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
