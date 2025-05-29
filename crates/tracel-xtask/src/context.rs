use strum::{Display, EnumIter, EnumString};

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Context {
    /// Set the context to all
    All,
    #[strum(to_string = "no-std")]
    /// Set the context to no-std (no Rust standard library available).
    NoStd,
    /// Set the context to std (Rust standard library is available).
    #[default]
    Std,
}
