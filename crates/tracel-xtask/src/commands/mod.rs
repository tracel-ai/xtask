#[cfg(feature = "aws-container")]
pub mod aws_container;
#[cfg(feature = "aws-secrets")]
pub mod aws_secrets;
#[cfg(feature = "build")]
pub mod build;
#[cfg(feature = "bump")]
pub mod bump;
#[cfg(feature = "check")]
pub mod check;
#[cfg(all(not(feature = "check"), feature = "validate"))]
mod check;
#[cfg(feature = "clean")]
pub mod clean;
#[cfg(feature = "compile")]
pub mod compile;
#[cfg(feature = "coverage")]
pub mod coverage;
#[cfg(feature = "dependencies")]
pub mod dependencies;
#[cfg(feature = "doc")]
pub mod doc;
#[cfg(feature = "docker-compose")]
pub mod docker_compose;
#[cfg(feature = "fix")]
pub mod fix;
#[cfg(feature = "gcp-container")]
pub mod gcp_container;
#[cfg(feature = "gcp-secrets")]
pub mod gcp_secrets;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "image")]
pub mod image;
#[cfg(feature = "infra")]
pub mod infra;
#[cfg(feature = "publish")]
pub mod publish;
#[cfg(feature = "test")]
pub mod test;
#[cfg(all(not(feature = "test"), feature = "validate"))]
mod test;
#[cfg(feature = "validate")]
pub mod validate;
#[cfg(feature = "vulnerabilities")]
pub mod vulnerabilities;

// use crate::declare_target;
use clap::ValueEnum;
use strum::{Display, EnumIter, EnumString};

pub const CARGO_NIGHTLY_MSG: &str = "You must use 'cargo +nightly' to run nightly checks.
Install a nightly toolchain with 'rustup toolchain install nightly'.";
pub const WARN_IGNORED_EXCLUDE_AND_ONLY_ARGS: &str = "'--target workspace' ignores the arguments '--exclude' and '--only'. Use '--target crates' instead.";
pub const WARN_IGNORED_ONLY_ARGS: &str =
    "'--target workspace' ignores the arguments '--only'. Use '--target crates' instead.";

#[tracel_xtask_macros::declare_targets]
pub enum Target {}

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Profile {
    All,
    #[default]
    Debug,
    Release,
}
