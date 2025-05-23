use strum::{Display, EnumIter, EnumString};

use crate::group_info;

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum Environment {
    /// Development environment (alias: dev).
    #[default]
    #[strum(serialize = "dev")]
    #[clap(alias = "dev")]
    Development,
    /// Staging environment (alias: stag).
    #[strum(serialize = "stag")]
    #[clap(alias = "stag")]
    Staging,
    /// Testing environment (alias: test).
    #[strum(serialize = "test")]
    #[clap(alias = "test")]
    Testing,
    /// Production environment (alias: prod).
    #[strum(serialize = "prod")]
    #[clap(alias = "prod")]
    Production,
}

impl Environment {
    pub(crate) fn get_dotenv_filename(&self) -> String {
        format!(".env.{}", self)
    }

    pub(crate) fn get_dotenv_secrets_filename(&self) -> String {
        format!("{}.secrets", self.get_dotenv_filename())
    }

    pub(crate) fn load(&self) -> anyhow::Result<()> {
        let filename = self.get_dotenv_filename();
        let secrets_filename = self.get_dotenv_secrets_filename();
        if dotenvy::from_filename(".env").is_ok() {
            group_info!("loading '.env' file...");
        }
        if dotenvy::from_filename(&filename).is_ok() {
            group_info!("loading {filename} file...");
        }
        if dotenvy::from_filename(&secrets_filename).is_ok() {
            group_info!("loading {secrets_filename} file...");
        }
        Ok(())
    }
}

#[derive(EnumString, EnumIter, Default, Display, Clone, PartialEq, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum ExecutionEnvironment {
    /// Set the execution environment to all
    All,
    #[strum(to_string = "no-std")]
    /// Set the execution environment to no-std (no Rust standard library available).
    NoStd,
    /// Set the execution environment to std (Rust standard library is available).
    #[default]
    Std,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use rstest::rstest;
    use serial_test::serial;

    fn expected_vars(env: &Environment) -> Vec<(String, String)> {
        let suffix = match env {
            Environment::Development => "DEV",
            Environment::Staging => "STAG",
            Environment::Testing => "TEST",
            Environment::Production => "PROD",
        };

        vec![
            ("FROM_DOTENV".to_string(), ".env".to_string()),
            (format!("FROM_DOTENV_{suffix}").to_string(), env.get_dotenv_filename()),
            (format!("FROM_DOTENV_{suffix}_SECRETS").to_string(), env.get_dotenv_secrets_filename()),
        ]
    }

    #[rstest]
    #[case::dev(Environment::Development)]
    #[case::stag(Environment::Staging)]
    #[case::test(Environment::Testing)]
    #[case::prod(Environment::Production)]
    #[serial]
    fn test_environment_load(#[case] env: Environment) {
        // Remove possible prior values
        for (key, _) in expected_vars(&env) {
            env::remove_var(key);
        }

        // Run the actual function under test
        env.load().expect("Environment load should succeed");

        // Assert each expected env var is present and has the correct value
        for (key, expected_value) in expected_vars(&env) {
            let actual_value = env::var(&key).unwrap_or_else(|_| panic!("Missing expected env var: {}", key));
            assert_eq!(
                actual_value, expected_value,
                "Environment variable {} should be set to {} but was {}",
                key, expected_value, actual_value
            );
        }
    }
}
