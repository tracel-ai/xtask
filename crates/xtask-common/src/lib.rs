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
    #[command(subcommand)]
    pub command: C,
}

pub fn init_xtask() {
    init_logger().init();
}
