#[cfg(feature = "coverage")]
pub(crate) const GRCOV_VERSION: &str = "0.8.19";
#[cfg(any(feature = "check", feature = "fix", feature = "validate"))]
pub(crate) const TYPOS_VERSION: &str = "1.39.0";
