use strum::{Display, EnumIter, EnumString};

use crate::{group_error, group_info};

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
    Test,
    /// Production environment (alias: prod).
    #[strum(serialize = "prod")]
    #[clap(alias = "prod")]
    Production,
}

impl Environment {
    pub fn get_dotenv_filename(&self) -> String {
        format!(".env.{self}")
    }

    pub fn get_dotenv_secrets_filename(&self) -> String {
        format!("{}.secrets", self.get_dotenv_filename())
    }

    pub fn get_env_files(&self) -> [String; 3] {
        let filename = self.get_dotenv_filename();
        let secrets_filename = self.get_dotenv_secrets_filename();
        [".env".to_owned(), filename.to_owned(), secrets_filename.to_owned()]
    }

    pub(crate) fn load(&self) -> anyhow::Result<()> {
        let files = self.get_env_files();
        files.iter().for_each(|f| {
            let path = std::path::Path::new(f);
            if path.exists() {
                match dotenvy::from_filename(f) {
                    Ok(_) => {
                        group_info!("loading '{}' file...", f);
                    }
                    Err(e) => {
                        group_error!("error while loading '{}' file ({})", f, e);
                    }
                }
            } else {
                group_info!("environment file '{}' does not exist, skipping...", f);
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;
    use std::env;

    fn expected_vars(env: &Environment) -> Vec<(String, String)> {
        let suffix = match env {
            Environment::Development => "DEV",
            Environment::Staging => "STAG",
            Environment::Test => "TEST",
            Environment::Production => "PROD",
        };

        vec![
            ("FROM_DOTENV".to_string(), ".env".to_string()),
            (
                format!("FROM_DOTENV_{suffix}").to_string(),
                env.get_dotenv_filename(),
            ),
            (
                format!("FROM_DOTENV_{suffix}_SECRETS").to_string(),
                env.get_dotenv_secrets_filename(),
            ),
        ]
    }

    #[rstest]
    #[case::dev(Environment::Development)]
    #[case::stag(Environment::Staging)]
    #[case::test(Environment::Test)]
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
            let actual_value =
                env::var(&key).unwrap_or_else(|_| panic!("Missing expected env var: {key}"));
            assert_eq!(
                actual_value, expected_value,
                "Environment variable {key} should be set to {expected_value} but was {actual_value}"
            );
        }
    }
}
