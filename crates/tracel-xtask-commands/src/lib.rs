pub mod commands;
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
        pub use tracel_xtask_macros::commands;
        pub use tracel_xtask_macros::declare_command_args;
        pub use tracel_xtask_macros::extend_command_args;
        pub use tracel_xtask_macros::extend_targets;
    }

    pub use crate::init_xtask;
    pub use crate::commands as base_commands;
    pub use crate::commands::build::BuildCmdArgs;
    pub use crate::commands::bump::BumpCmdArgs;
    pub use crate::commands::check::CheckCmdArgs;
    pub use crate::commands::compile::CompileCmdArgs;
    pub use crate::commands::coverage::CoverageCmdArgs;
    pub use crate::commands::dependencies::DependenciesCmdArgs;
    pub use crate::commands::doc::DocCmdArgs;
    pub use crate::commands::fix::FixCmdArgs;
    pub use crate::commands::publish::PublishCmdArgs;
    pub use crate::commands::test::TestCmdArgs;
    pub use crate::commands::vulnerabilities::VulnerabilitiesCmdArgs;
    pub use crate::commands::Target;

}

use crate::logging::init_logger;

// does not re-export strum has it is incompatible with strum macros expansions
use strum;
use strum::{Display, EnumIter, EnumString};

#[macro_use]
extern crate log;

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum ExecutionEnvironment {
    #[strum(to_string = "no-std")]
    NoStd,
    #[default]
    Std,
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct XtaskArgs<C: clap::Subcommand> {
    /// Enable code coverage.
    #[arg(short = 'c', long)]
    pub enable_coverage: bool,
    /// Set execution environment.
    #[arg(short = 'e', long, default_value_t = ExecutionEnvironment::Std)]
    pub execution_environment: ExecutionEnvironment,
    #[command(subcommand)]
    pub command: C,
}

pub fn init_xtask<C: clap::Subcommand>() -> anyhow::Result<XtaskArgs<C>> {
    init_logger().init();
    let args = <XtaskArgs::<C> as clap::Parser>::parse();

    group_info!("Execution environment: {}", args.execution_environment);

    // initialize code coverage
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
