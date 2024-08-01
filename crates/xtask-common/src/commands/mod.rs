pub mod build;
pub mod bump;
pub mod check;
pub mod compile;
pub mod coverage;
pub mod dependencies;
pub mod doc;
pub mod fix;
pub mod publish;
pub mod pull_request_checks;
pub mod test;
pub mod vulnerabilities;

use clap::ValueEnum;
use strum::{Display, EnumIter, EnumString};

pub const CARGO_NIGHTLY_MSG: &str = "You must use 'cargo +nightly' to run nightly checks.
Install a nightly toolchain with 'rustup toolchain install nightly'.";
pub const WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS: &str =
    "--target workspace ignores the arguments --exclude and --only. Use --target all-packages instead.";
pub const WARN_IGNORED_ONLY_ARGS: &str =
    "--target workspace ignores the arguments --only. Use --target all-packages instead.";

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Target {
    AllPackages,
    Crates,
    Examples,
    #[default]
    Workspace,
}

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Profile {
    All,
    #[default]
    Debug,
    Release,
}
