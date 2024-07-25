pub mod commands;
pub mod logging;
pub mod utils;

// re-exports
pub use anyhow;
pub use clap;
use clap::{Parser, Subcommand};
pub use derive_more;
pub use env_logger;
pub use rand;
pub use serde_json;
pub use strum;
pub use tracing_subscriber;

use crate::logging::init_logger;

#[macro_use]
extern crate log;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct XtaskArgs<C: Subcommand> {
    /// Enable code coverage.
    #[arg(short = 'c', long)]
    pub enable_coverage: bool,
    #[command(subcommand)]
    pub command: C,
}

pub fn init_xtask<C: Subcommand>() -> anyhow::Result<XtaskArgs<C>> {
    init_logger().init();
    let args = XtaskArgs::<C>::parse();

    // initialize code coverage
    if args.enable_coverage {
        info!("Enabling coverage support...");
        setup_coverage()?;
    }

    Ok(args)
}

fn setup_coverage() -> anyhow::Result<()> {
    std::env::set_var("RUSTFLAGS", "-Cinstrument-coverage");
    std::env::set_var("LLVM_PROFILE_FILE", "burn-%p-%m.profraw");
    Ok(())
}
