use std::{
    collections::HashMap,
    fmt::{self, Display, Write as _},
    path::PathBuf,
};

use strum::{EnumIter, EnumString};

use crate::{group_error, group_info, utils::git};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Environment {
    pub name: EnvironmentName,
    pub index: EnvironmentIndex,
}

impl Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.medium())
    }
}

impl Environment {
    pub fn new(name: EnvironmentName, index: u8) -> Self {
        Self {
            name,
            index: index.into(),
        }
    }

    pub fn long(&self) -> String {
        format!("{}{}", self.name.long(), self.index)
    }

    pub fn medium(&self) -> String {
        format!("{}{}", self.name.medium(), self.index)
    }

    pub fn short(&self) -> String {
        format!("{}{}", self.name.short(), self.index)
    }

    pub fn index(&self) -> u8 {
        self.index.index
    }

    pub fn get_dotenv_filename(&self) -> String {
        format!(".env.{self}")
    }

    pub fn get_dotenv_secrets_filename(&self) -> String {
        format!("{}.secrets", self.get_dotenv_filename())
    }

    pub fn get_env_files(&self) -> [String; 3] {
        let filename = self.get_dotenv_filename();
        let secrets_filename = self.get_dotenv_secrets_filename();
        [
            ".env".to_owned(),
            filename.to_owned(),
            secrets_filename.to_owned(),
        ]
    }

    /// Load the .env environment files family.
    pub fn load(&self, prefix: Option<&str>) -> anyhow::Result<()> {
        let files = self.get_env_files();
        files.iter().for_each(|f| {
            let path = if let Some(p) = prefix {
                std::path::PathBuf::from(p).join(f)
            } else {
                std::path::PathBuf::from(f)
            };
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

    /// Merge all the .env files of the environment with all variable expanded
    pub fn merge_env_files(&self) -> anyhow::Result<PathBuf> {
        let repo_root = git::git_repo_root_or_cwd()?;
        let files = self.get_env_files();
        // merged set of env vars, the later files override earlier ones
        // we sort keys to have a more deterministic merged file result
        let mut merged: HashMap<String, String> = HashMap::new();
        for filename in files {
            let path = repo_root.join(&filename);
            if !path.exists() {
                eprintln!(
                    "⚠️ Warning: environment file '{}' ({}) not found, skipping...",
                    filename,
                    path.display()
                );
                continue;
            }
            for item in dotenvy::from_path_iter(&path)? {
                let (key, value) = item?;
                std::env::set_var(&key, &value);
                merged.insert(key, value);
            }
        }
        let mut keys: Vec<_> = merged.keys().cloned().collect();
        keys.sort();
        // write merged file
        let mut out = String::new();
        for key in keys {
            let val = &merged[&key];
            writeln!(&mut out, "{key}={val}")?;
        }
        let tmp_path = std::env::temp_dir().join(format!("merged-env-{}.tmp", std::process::id()));
        std::fs::write(&tmp_path, out)?;
        Ok(tmp_path)
    }
}

#[derive(EnumString, EnumIter, Default, Clone, Debug, PartialEq, clap::ValueEnum)]
#[strum(serialize_all = "lowercase")]
pub enum EnvironmentName {
    /// Development environment (alias: dev).
    #[default]
    #[clap(alias = "dev")]
    Development,
    /// Staging environment (alias: stag).
    #[clap(alias = "stag")]
    Staging,
    /// Testing environment (alias: test).
    #[clap(alias = "test")]
    Test,
    /// Production environment (alias: prod).
    #[clap(alias = "prod")]
    Production,
}

impl Display for EnvironmentName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.medium())
    }
}

impl EnvironmentName {
    fn long(&self) -> &'static str {
        match self {
            EnvironmentName::Development => "development",
            EnvironmentName::Staging => "staging",
            EnvironmentName::Test => "test",
            EnvironmentName::Production => "production",
        }
    }

    fn medium(&self) -> &'static str {
        match self {
            EnvironmentName::Development => "dev",
            EnvironmentName::Staging => "stag",
            EnvironmentName::Test => "test",
            EnvironmentName::Production => "prod",
        }
    }

    fn short(&self) -> char {
        match self {
            EnvironmentName::Development => 'd',
            EnvironmentName::Staging => 's',
            EnvironmentName::Test => 't',
            EnvironmentName::Production => 'p',
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentIndex {
    pub index: u8,
}

impl Default for EnvironmentIndex {
    fn default() -> Self {
        Self { index: 1 }
    }
}

impl Display for EnvironmentIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.index == 1 {
            write!(f, "")
        } else {
            write!(f, "{}", self.index)
        }
    }
}

impl From<u8> for EnvironmentIndex {
    fn from(index: u8) -> Self {
        Self { index }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serial_test::serial;
    use std::env;

    fn expected_vars(env: &Environment) -> Vec<(String, String)> {
        let suffix = match env.name {
            EnvironmentName::Development => "DEV",
            EnvironmentName::Staging => "STAG",
            EnvironmentName::Test => "TEST",
            EnvironmentName::Production => "PROD",
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
    #[case::dev(Environment { name: EnvironmentName::Development, index: EnvironmentIndex { index: 1 } })]
    #[case::stag(Environment { name: EnvironmentName::Staging, index: EnvironmentIndex { index: 1 } })]
    #[case::test(Environment { name: EnvironmentName::Test, index: EnvironmentIndex { index: 1 } })]
    #[case::prod(Environment { name: EnvironmentName::Production, index: EnvironmentIndex { index: 1 } })]
    #[serial]
    fn test_environment_load(#[case] env: Environment) {
        // Remove possible prior values
        for (key, _) in expected_vars(&env) {
            env::remove_var(key);
        }

        // Run the actual function under test
        env.load(Some("../.."))
            .expect("Environment load should succeed");

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

    #[rstest]
    #[case::dev(Environment { name: EnvironmentName::Development, index: EnvironmentIndex { index: 1 } })]
    #[case::stag(Environment { name: EnvironmentName::Staging, index: EnvironmentIndex { index: 1 } })]
    #[case::test(Environment { name: EnvironmentName::Test, index: EnvironmentIndex { index: 1 } })]
    #[case::prod(Environment { name: EnvironmentName::Production, index: EnvironmentIndex { index: 1 } })]
    #[serial]
    fn test_environment_merge_env_files(#[case] env: Environment) {
        // Make sure we start from a clean state
        for (key, _) in expected_vars(&env) {
            env::remove_var(key);
        }
        // Generate the merged env file
        let merged_path = env
            .merge_env_files()
            .expect("merge_env_files should succeed");
        assert!(
            merged_path.exists(),
            "Merged env file should exist at {}",
            merged_path.display()
        );
        // Parse the merged file as a .env file again
        let mut merged_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for item in
            dotenvy::from_path_iter(&merged_path).expect("Reading merged env file should succeed")
        {
            let (key, value) = item.expect("Parsing key/value from merged env file should succeed");
            merged_map.insert(key, value);
        }
        // All the vars we expect from the individual files must be present
        for (key, expected_value) in expected_vars(&env) {
            let actual_value = merged_map
                .get(&key)
                .unwrap_or_else(|| panic!("Missing expected merged env var: {key}"));
            assert_eq!(
                actual_value, &expected_value,
                "Merged env var {key} should be {expected_value} but was {actual_value}"
            );
        }
    }

    #[test]
    #[serial]
    fn test_environment_merge_env_files_expansion() {
        let env = Environment {
            name: EnvironmentName::Staging,
            index: EnvironmentIndex { index: 1 },
        };
        // Clean any prior values that could interfere
        env::remove_var("LOG_LEVEL_TEST");
        env::remove_var("RUST_LOG_TEST");
        env::remove_var("RUST_LOG_STAG_TEST");

        let merged_path = env
            .merge_env_files()
            .expect("merge_env_files should succeed");
        let mut merged_map: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for item in
            dotenvy::from_path_iter(&merged_path).expect("Reading merged env file should succeed")
        {
            let (key, value) = item.expect("Parsing key/value from merged env file should succeed");
            merged_map.insert(key, value);
        }

        let log_level = merged_map
            .get("LOG_LEVEL_TEST")
            .expect("LOG_LEVEL_TEST should be present in merged env file");
        let rust_log = merged_map
            .get("RUST_LOG_TEST")
            .expect("RUST_LOG_TEST should be present in merged env file");

        // 1) We should not see the raw placeholder anymore
        assert!(
            !rust_log.contains("${LOG_LEVEL_TEST}"),
            "RUST_LOG_TEST should not contain the raw placeholder '${{LOG_LEVEL}}', got: {rust_log}"
        );
        // 2) The expanded LOG_LEVEL_TEST value should appear in RUST_LOG_TEST
        assert!(
            rust_log.contains(log_level),
            "RUST_LOG_TEST should contain the expanded LOG_LEVEL_TEST value; LOG_LEVEL_TEST={log_level}, RUST_LOG_TEST={rust_log}"
        );
        // Cross-file expansion with RUST_LOG_STAG_TEST that references LOG_LEVEL_TEST from base .env
        let rust_log_stag = merged_map
            .get("RUST_LOG_STAG_TEST")
            .expect("RUST_LOG_STAG_TEST should be present in merged env file");
        // 3) No raw placeholder in the cross-file value either
        assert!(
            !rust_log_stag.contains("${LOG_LEVEL_TEST}"),
            "RUST_LOG_STAG_TEST should not contain the raw placeholder '${{LOG_LEVEL_TEST}}', got: {rust_log_stag}"
        );
        // 4) The expanded LOG_LEVEL_TEST value should appear in RUST_LOG_STAG_TEST
        assert!(
            rust_log_stag.contains(log_level),
            "RUST_LOG_STAG_TEST should contain the expanded LOG_LEVEL_TEST value; LOG_LEVEL_TEST={log_level}, RUST_LOG_STAG_TEST={rust_log_stag}"
        );
    }
}
