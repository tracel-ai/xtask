pub mod bump;
pub mod check;
pub mod ci;
pub mod publish;
pub mod pull_request_checks;
pub mod test;

use clap::ValueEnum;
use strum::{Display, EnumIter, EnumString};

#[derive(EnumString, EnumIter, Display, Clone, PartialEq, ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Target {
    All,
    Crates,
    Examples,
}
