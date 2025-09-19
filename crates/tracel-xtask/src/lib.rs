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

    pub use crate::commands as base_commands;
    pub use crate::commands::build::BuildCmdArgs;
    pub use crate::commands::bump::BumpCmdArgs;
    pub use crate::commands::bump::BumpSubCommand;
    pub use crate::commands::check::CheckCmdArgs;
    pub use crate::commands::check::CheckSubCommand;
    pub use crate::commands::compile::CompileCmdArgs;
    pub use crate::commands::coverage::CoverageCmdArgs;
    pub use crate::commands::dependencies::DependenciesCmdArgs;
    pub use crate::commands::dependencies::DependenciesSubCommand;
    pub use crate::commands::doc::DocCmdArgs;
    pub use crate::commands::doc::DocSubCommand;
    pub use crate::commands::docker::DockerCmdArgs;
    pub use crate::commands::docker::DockerSubCommand;
    pub use crate::commands::fix::FixCmdArgs;
    pub use crate::commands::fix::FixSubCommand;
    pub use crate::commands::publish::PublishCmdArgs;
    pub use crate::commands::test::TestCmdArgs;
    pub use crate::commands::test::TestSubCommand;
    pub use crate::commands::validate::ValidateCmdArgs;
    pub use crate::commands::vulnerabilities::VulnerabilitiesCmdArgs;
    pub use crate::commands::vulnerabilities::VulnerabilitiesSubCommand;
    pub use crate::commands::Target;
    pub use crate::context::Context;
    pub use crate::endgroup;
    pub use crate::environment::Environment;
    pub use crate::group;
    pub use crate::group_info;
    pub use crate::handle_cleanup;
    pub use crate::init_xtask;
    pub use crate::parse_args;
    pub use crate::register_cleanup;
    pub use crate::utils::cargo::ensure_cargo_crate_is_installed;
    pub use crate::utils::cleanup::CLEANUP_HANDLER;
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
    pub use crate::utils::time::format_duration;
    pub use crate::XtaskArgs;
    // does not re-export strum has it is incompatible with strum macros expansions
}

use crate::context::Context;
use crate::environment::Environment;
use crate::logging::init_logger;

#[macro_use]
extern crate log;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct XtaskArgs<C: clap::Subcommand> {
    /// Enable code coverage for Rust code if available (see coverage command for more info).
    #[arg(long)]
    pub enable_coverage: bool,
    /// Set environment (for commands that support it).
    #[arg(short = 'e', long, default_value_t = Environment::default())]
    pub environment: Environment,
    /// Set context (for commands that support it).
    #[arg(short = 'c', long, default_value_t = Context::default())]
    pub context: Context,
    #[command(subcommand)]
    pub command: C,
}

pub fn parse_args<C: clap::Subcommand>() -> anyhow::Result<XtaskArgs<C>> {
    // init logs early
    init_logger().init();
    let args = <XtaskArgs<C> as clap::Parser>::parse();
    Ok(args)
}

pub fn init_xtask<C: clap::Subcommand>(args: XtaskArgs<C>) -> anyhow::Result<XtaskArgs<C>> {
    // environment
    group_info!("Environment: {}", args.environment);
    args.environment.load(None)?;
    group_info!("Context: {}", args.context);
    // code coverage
    if args.enable_coverage {
        group_info!("Enabling coverage support...");
        setup_coverage()?;
    }
    Ok(args)
}

fn setup_coverage() -> anyhow::Result<()> {
    unsafe {
        std::env::set_var("RUSTFLAGS", "-Cinstrument-coverage");
        std::env::set_var("LLVM_PROFILE_FILE", "burn-%p-%m.profraw");
    }
    Ok(())
}
