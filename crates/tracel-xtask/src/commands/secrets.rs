/// Manage AWS Secrets Manager secrets.
use std::{collections::HashMap, fmt::Write as _, fs, io, path::PathBuf};

use anyhow::Context as _;

use crate::prelude::{Context, Environment};
use crate::utils::aws::cli::{
    secretsmanager_create_empty_secret, secretsmanager_get_secret_string,
    secretsmanager_list_secret_versions_json, secretsmanager_put_secret_string,
};

const FALLBACK_EDITOR: &str = "vi";

#[tracel_xtask_macros::declare_command_args(None, SecretsSubCommand)]
pub struct SecretsCmdArgs {}

impl Default for SecretsSubCommand {
    fn default() -> Self {
        SecretsSubCommand::View(SecretsViewSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsCreateSubCmdArgs {
    /// Region where the secret will be created
    #[arg(long)]
    pub region: String,

    /// Secret name to create with an initial empty JSON value
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,

    /// Optional description for the secret
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsCopySubCmdArgs {
    /// Region where the secrets live
    #[arg(long)]
    pub region: String,

    /// Source secret identifier (name or ARN)
    #[arg(long, value_name = "FROM_SECRET_ID")]
    pub from: String,

    /// Target secret identifier (name or ARN)
    #[arg(long, value_name = "TO_SECRET_ID")]
    pub to: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsEditSubCmdArgs {
    /// Region where the secret lives
    #[arg(long)]
    pub region: String,

    /// Secret identifier (name or ARN)
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,

    /// Push the new secret version without asking for confirmation
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsEnvFileSubCmdArgs {
    /// Output file path. If omitted, writes to stdout.
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,

    /// Region where the secrets live
    #[arg(long)]
    pub region: String,

    /// Secret identifiers (names or ARN), can provide multiple ones.
    #[arg(value_name = "SECRET_ID", num_args(1..), required = true)]
    pub secret_ids: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsListSubCmdArgs {
    /// Region where the secret lives
    #[arg(long)]
    pub region: String,

    /// Secret identifier (name or ARN)
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsPushSubCmdArgs {
    /// Region where the secret lives
    #[arg(long)]
    pub region: String,

    /// Secret identifier (name or ARN)
    #[arg(long, value_name = "SECRET_ID")]
    pub secret_id: String,

    /// Push the new secret version without asking for confirmation
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Key-value updates in the form KEY=VALUE
    #[arg(value_name = "KEY=VALUE", num_args(1..), required = true)]
    pub kv: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct SecretsViewSubCmdArgs {
    /// Region where the secret lives
    #[arg(long)]
    pub region: String,

    /// Secret identifier (name or ARN)
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
}

pub fn handle_command(
    args: SecretsCmdArgs,
    _env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        SecretsSubCommand::Create(create_args) => create(create_args),
        SecretsSubCommand::Copy(copy_args) => copy(copy_args),
        SecretsSubCommand::Edit(edit_args) => edit(edit_args),
        SecretsSubCommand::EnvFile(env_args) => env_file(env_args),
        SecretsSubCommand::List(list_args) => list(list_args),
        SecretsSubCommand::Push(push_args) => push(push_args),
        SecretsSubCommand::View(view_args) => view(view_args),
    }
}

/// Create a secret and attach an initial empty JSON (`{}`) version as plain text.
fn create(args: SecretsCreateSubCmdArgs) -> anyhow::Result<()> {
    // create the secret metadata
    secretsmanager_create_empty_secret(&args.secret_id, &args.region, args.description.as_deref())?;
    // add a first version as an empty JSON object.
    secretsmanager_put_secret_string(&args.secret_id, &args.region, "{}")?;
    eprintln!(
        "✅ Created secret '{}' in region '{}' with an initial empty JSON value.",
        args.secret_id, args.region
    );
    Ok(())
}

/// Copy a secret value from one secret ID to another in the same region.
pub fn copy(args: SecretsCopySubCmdArgs) -> anyhow::Result<()> {
    if args.from == args.to {
        eprintln!(
            "Source and target secrets are identical ('{}'), nothing to do.",
            args.from
        );
        return Ok(());
    }

    eprintln!(
        "Fetching source secret '{}' in region '{}'...",
        args.from, args.region
    );
    let value = secretsmanager_get_secret_string(&args.from, &args.region, "text")?;

    // Check if the target already has a current version.
    // If we can successfully fetch it, we consider that a current version exists
    // and ask for confirmation before creating a new one.
    let target_has_version =
        secretsmanager_get_secret_string(&args.to, &args.region, "text").is_ok();
    if target_has_version {
        eprintln!(
            "Secret '{}' already has a current version in region '{}'.",
            args.to, args.region
        );
        if !confirm_push()? {
            eprintln!("Aborting: new secret version was not pushed.");
            return Ok(());
        }
    }

    eprintln!(
        "Writing target secret '{}' in region '{}'...",
        args.to, args.region
    );
    secretsmanager_put_secret_string(&args.to, &args.region, &value)?;
    eprintln!(
        "✅ Copied secret value from '{}' to '{}'.",
        args.from, args.to
    );

    Ok(())
}

/// Fetch secret into a temp file, open editor,
/// ask to commit or discard on close and then push a new version if confirmed.
///
/// Behavior:
/// - If the secret is JSON, it is pretty-printed for editing and stored back
///   minified on a single line.
/// - If the secret is not JSON, it is treated as an opaque string.
fn edit(args: SecretsEditSubCmdArgs) -> anyhow::Result<()> {
    // 1) fetch current secret value
    let original_raw = secretsmanager_get_secret_string(&args.secret_id, &args.region, "text")?;
    let original_raw_trimmed = original_raw.trim_end_matches('\n');
    // 2) make things pretty if possible
    let to_edit =
        pretty_json(original_raw_trimmed).unwrap_or_else(|| original_raw_trimmed.to_string());
    // 3) write the secrets to a temp file for editing
    let tmp_path = temp_file_path(&args.secret_id);
    fs::write(&tmp_path, &to_edit)?;
    eprintln!(
        "Editing secret '{}' in region '{}' using temporary file:\n  {}",
        args.secret_id,
        args.region,
        tmp_path.display()
    );
    // 4) open editor
    let editor = detect_editor();
    let mut parts = editor.split_whitespace();
    let cmd = parts.next().unwrap_or(FALLBACK_EDITOR);
    let mut command = std::process::Command::new(cmd);
    for arg in parts {
        command.arg(arg);
    }
    command.arg(&tmp_path);
    let status = command
        .status()
        .map_err(|e| anyhow::anyhow!("launching editor '{editor}' should succeed: {e}"))?;
    if !status.success() {
        fs::remove_file(&tmp_path).ok();
        return Err(anyhow::anyhow!(
            "editor '{editor}' should exit successfully (exit status {status})"
        ));
    }
    // 5) read updated contents of file and do some cleanup
    let edited_raw = fs::read_to_string(&tmp_path)?;
    fs::remove_file(&tmp_path).ok();
    let edited_raw_trimmed = edited_raw.trim_end_matches('\n');
    // Try to treat content as JSON on both sides
    let original_norm_json = normalize_json(original_raw_trimmed);
    let edited_norm_json = normalize_json(edited_raw_trimmed);
    // If both are valid JSON, compare and store minified JSON
    if let (Some(orig_norm), Some(edited_norm)) = (original_norm_json, edited_norm_json) {
        if orig_norm == edited_norm {
            eprintln!(
                "No changes detected (JSON content unchanged), not pushing a new secret version."
            );
            return Ok(());
        }
        eprintln!("Secret JSON content has changed.");
        if !args.yes && !confirm_push()? {
            eprintln!("Aborting: new secret version was not pushed.");
            return Ok(());
        }
        // Store minified JSON
        secretsmanager_put_secret_string(&args.secret_id, &args.region, &edited_norm)?;
        eprintln!(
            "✅ New JSON version pushed for secret '{}' in region '{}'.",
            args.secret_id, args.region
        );
        return Ok(());
    }
    // 6) Fallback for non-JSON secrets
    if edited_raw_trimmed == original_raw_trimmed {
        eprintln!("No changes detected, not pushing a new secret version.");
        return Ok(());
    }
    eprintln!("Secret content has changed.");
    if !args.yes && !confirm_push()? {
        eprintln!("Aborting: new secret version was not pushed.");
        return Ok(());
    }
    secretsmanager_put_secret_string(&args.secret_id, &args.region, edited_raw_trimmed)?;
    eprintln!(
        "✅ New version pushed for secret '{}' in region '{}'.",
        args.secret_id, args.region
    );

    Ok(())
}

pub fn env_file(args: SecretsEnvFileSubCmdArgs) -> anyhow::Result<()> {
    if args.secret_ids.is_empty() {
        eprintln!("No secrets provided.");
        return Ok(());
    }
    // In case of multiple same secret names, the latest wins
    let mut merged: HashMap<String, String> = HashMap::new();
    for id in &args.secret_ids {
        eprintln!("Fetching secret '{id}'...");
        let s = secretsmanager_get_secret_string(id, &args.region, "text")?;
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        // 1) try JSON object
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(s) {
            if let Some(obj) = value.as_object() {
                for (k, v) in obj {
                    let v_str = v
                        .as_str()
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| v.to_string());
                    merged.insert(k.clone(), v_str);
                }
                continue;
            }
        }
        // 2) fallback to .env style format
        for line in s.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                merged.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }

    // Expand variable inside values
    let merged = expand_env_map(&merged);

    // Sort the env vars for deterministic ordering
    let mut entries: Vec<(String, String)> = merged.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    // Write to passed path or to stdout if no path has been passed
    let mut buf = String::new();
    for (k, v) in entries {
        writeln!(&mut buf, "{k}={v}")?;
    }
    if let Some(path) = args.output {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, buf)?;
        eprintln!("Wrote env file to {}", path.display());
    } else {
        print!("{buf}");
    }

    Ok(())
}

/// list all versions of the secrets.
fn list(args: SecretsListSubCmdArgs) -> anyhow::Result<()> {
    eprintln!(
        "Listing versions for secret '{}' in region '{}'...",
        args.secret_id, args.region
    );
    let json = secretsmanager_list_secret_versions_json(&args.secret_id, &args.region)?;
    let v: serde_json::Value = serde_json::from_str(&json).context(
        "Parsing Secrets Manager list-secret-version-ids response as JSON should succeed",
    )?;
    let versions = v
        .get("Versions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "AWS response for secret '{}' should contain a 'Versions' array",
                args.secret_id
            )
        })?;

    if versions.is_empty() {
        println!("No versions found for secret '{}'.", args.secret_id);
        return Ok(());
    }

    struct Row {
        id: String,
        created: String,
        stages: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut id_w = "VersionId".len();
    let mut created_w = "Created".len();
    let mut stages_w = "Stages".len();

    for ver in versions {
        let id = ver
            .get("VersionId")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        let created = match ver.get("CreatedDate") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Number(n)) => n.to_string(),
            Some(other) => other.to_string(),
            None => "".to_string(),
        };

        let stages = ver
            .get("VersionStages")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();

        id_w = id_w.max(id.len());
        created_w = created_w.max(created.len());
        stages_w = stages_w.max(stages.len());

        rows.push(Row {
            id,
            created,
            stages,
        });
    }

    // Header
    println!(
        "{:<id_w$}  {:<created_w$}  {:<stages_w$}",
        "VersionId",
        "Created",
        "Stages",
        id_w = id_w,
        created_w = created_w,
        stages_w = stages_w,
    );

    // Separator
    println!(
        "{:-<id_w$}  {:-<created_w$}  {:-<stages_w$}",
        "",
        "",
        "",
        id_w = id_w,
        created_w = created_w,
        stages_w = stages_w,
    );

    // Rows
    for r in rows {
        println!(
            "{:<id_w$}  {:<created_w$}  {:<stages_w$}",
            r.id,
            r.created,
            r.stages,
            id_w = id_w,
            created_w = created_w,
            stages_w = stages_w,
        );
    }

    Ok(())
}

/// Push updates to a JSON secret by setting one or more KEY=VALUE pairs.
/// The secret must be a JSON object and updated value is stored on a single line.
pub fn push(args: SecretsPushSubCmdArgs) -> anyhow::Result<()> {
    // 1) fetch current secrets
    eprintln!(
        "Fetching secret '{}' in region '{}'...",
        args.secret_id, args.region
    );
    let original = secretsmanager_get_secret_string(&args.secret_id, &args.region, "text")?;
    let original_trimmed = original.trim_end_matches('\n');
    let mut value: serde_json::Value =
        serde_json::from_str(original_trimmed).with_context(|| {
            format!(
                "Parsing secret '{}' as JSON should succeed to use the 'push' subcommand",
                args.secret_id
            )
        })?;
    let obj = value.as_object_mut().ok_or_else(|| {
        anyhow::anyhow!(
            "Secret '{}' should be a JSON object to use the 'push' subcommand",
            args.secret_id
        )
    })?;

    // 2) Add key-value pairs to secret
    let mut changed = false;
    eprintln!("Changed entries to update:");
    for kv in &args.kv {
        let (key, val) = kv.split_once('=').ok_or_else(|| {
            anyhow::anyhow!(
                "Key/value argument '{kv}' should use the KEY=VALUE format for secret '{}'",
                args.secret_id
            )
        })?;
        let key = key.trim();
        let val = val.trim();
        if key.is_empty() {
            anyhow::bail!(
                "Key in '{kv}' should not be empty for secret '{}'",
                args.secret_id
            );
        }
        let existing = obj.get(key);
        // If existing value is the same string, skip
        if let Some(existing_val) = existing {
            if existing_val.is_string() && existing_val.as_str() == Some(val) {
                // skip value if it has not changed
                continue;
            }
        }
        obj.insert(key.to_string(), serde_json::Value::String(val.to_string()));
        changed = true;
        eprintln!("  - {key}");
    }
    if !changed {
        eprintln!("None.");
        eprintln!(
            "No changes detected (JSON content unchanged), not pushing a new secret version."
        );
        return Ok(());
    }

    // 3) Confirmation prompt
    eprintln!("Secret JSON content has changed.");
    if !args.yes && !confirm_push()? {
        eprintln!("Aborting: new secret version was not pushed.");
        return Ok(());
    }

    // 4) Store the new version of the secrets as a minified JSON format
    let normalized =
        serde_json::to_string(&value).context("Serializing updated JSON secret should succeed")?;
    secretsmanager_put_secret_string(&args.secret_id, &args.region, &normalized)?;
    eprintln!(
        "✅ Updated secret '{}' in region '{}' with {} key(s).",
        args.secret_id,
        args.region,
        args.kv.len()
    );

    Ok(())
}

/// fetch and print the secret.
fn view(args: SecretsViewSubCmdArgs) -> anyhow::Result<()> {
    let value = secretsmanager_get_secret_string(&args.secret_id, &args.region, "text")?;
    let trimmed = value.trim_end_matches('\n');

    if let Some(pretty) = pretty_json(trimmed) {
        println!("{pretty}");
    } else {
        println!("{value}");
    }

    Ok(())
}

/// Build a temp file path for editing a secret.
fn temp_file_path(secret_id: &str) -> PathBuf {
    let mut base: String = secret_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if base.len() > 64 {
        base.truncate(64);
    }
    let pid = std::process::id();
    let filename = format!("tracel-secret-{base}-{pid}.tmp");
    std::env::temp_dir().join(filename)
}

/// Detect the editor to use $VISUAL then $EDITOR then falling back to "vi".
fn detect_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| FALLBACK_EDITOR.to_string())
}

/// Try to pretty-print JSON
fn pretty_json(s: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(s).ok()?;
    serde_json::to_string_pretty(&value).ok()
}

/// Normalize JSON to a canonical minified form.
fn normalize_json(s: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(s).ok()?;
    serde_json::to_string(&value).ok()
}

/// Ask user to confirm pushing a new secret version.
fn confirm_push() -> anyhow::Result<bool> {
    use std::io::Write as _;

    print!("Do you want to push a new secret version? [y/N]: ");
    io::stdout().flush().ok();

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

/// Expand `${VAR}` placeholders in values using the given map.
fn expand_value(input: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = input;

    while let Some(start) = rest.find("${") {
        // keep everything before the placeholder
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        // find the closing brace
        if let Some(end_rel) = after.find('}') {
            let var_name = &after[..end_rel];
            if let Some(val) = vars.get(var_name) {
                // known var: substitute
                out.push_str(val);
            } else {
                // unknown var: keep the placeholder as-is
                out.push_str("${");
                out.push_str(var_name);
                out.push('}');
            }
            // continue after the closing brace
            rest = &after[end_rel + 1..];
        } else {
            // no closing brace, keep the rest as-is
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    // trailing part without placeholders
    out.push_str(rest);
    out
}

/// Expand `${VAR}` placeholders in a merged env map.
/// Note: this is a single pass expansion, preserving quotes and formatting.
fn expand_env_map(merged: &HashMap<String, String>) -> HashMap<String, String> {
    let mut expanded = HashMap::new();
    for (key, value) in merged {
        let new_val = expand_value(value, merged);
        expanded.insert(key.clone(), new_val);
    }
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_expand_env_map_simple_expansion() {
        // Clean any prior values that might confuse debugging
        unsafe {
            env::remove_var("LOG_LEVEL_TEST");
            env::remove_var("RUST_LOG_TEST");
        }

        let mut merged: HashMap<String, String> = HashMap::new();
        merged.insert("LOG_LEVEL_TEST".to_string(), "info".to_string());
        merged.insert(
            "RUST_LOG_TEST".to_string(),
            "xtask=${LOG_LEVEL_TEST},server=${LOG_LEVEL_TEST}".to_string(),
        );

        let expanded = expand_env_map(&merged);

        let log_level = expanded
            .get("LOG_LEVEL_TEST")
            .expect("LOG_LEVEL_TEST should be present after expansion");
        let rust_log = expanded
            .get("RUST_LOG_TEST")
            .expect("RUST_LOG_TEST should be present after expansion");

        assert_eq!(
            log_level, "info",
            "LOG_LEVEL_TEST should keep its literal value after expansion"
        );
        assert!(
            !rust_log.contains("${LOG_LEVEL_TEST}"),
            "RUST_LOG_TEST should not contain the raw placeholder '${{LOG_LEVEL_TEST}}', got: {rust_log}"
        );
        assert!(
            rust_log.contains(log_level),
            "RUST_LOG_TEST should contain the expanded LOG_LEVEL_TEST value; LOG_LEVEL_TEST={log_level}, RUST_LOG_TEST={rust_log}"
        );
    }

    #[test]
    #[serial]
    fn test_expand_env_map_mixed_values_and_non_expanded_keys() {
        unsafe {
            env::remove_var("LOG_LEVEL_TEST");
            env::remove_var("RUST_LOG_TEST");
            env::remove_var("PLAIN_KEY_TEST");
        }

        let mut merged: HashMap<String, String> = HashMap::new();
        merged.insert("LOG_LEVEL_TEST".to_string(), "debug".to_string());
        merged.insert(
            "RUST_LOG_TEST".to_string(),
            "xtask=${LOG_LEVEL_TEST},other=${LOG_LEVEL_TEST}".to_string(),
        );
        merged.insert("PLAIN_KEY_TEST".to_string(), "no_placeholders".to_string());

        let expanded = expand_env_map(&merged);

        let log_level = expanded
            .get("LOG_LEVEL_TEST")
            .expect("LOG_LEVEL_TEST should be present after expansion");
        let rust_log = expanded
            .get("RUST_LOG_TEST")
            .expect("RUST_LOG_TEST should be present after expansion");
        let plain = expanded
            .get("PLAIN_KEY_TEST")
            .expect("PLAIN_KEY_TEST should be present after expansion");

        assert_eq!(log_level, "debug");
        assert!(
            !rust_log.contains("${LOG_LEVEL_TEST}"),
            "RUST_LOG_TEST should not contain the raw placeholder '${{LOG_LEVEL_TEST}}', got: {rust_log}"
        );
        assert!(
            rust_log.contains(log_level),
            "RUST_LOG_TEST should contain the expanded LOG_LEVEL_TEST value; LOG_LEVEL_TEST={log_level}, RUST_LOG_TEST={rust_log}"
        );
        assert_eq!(
            plain, "no_placeholders",
            "PLAIN_KEY_TEST should remain unchanged when there are no placeholders"
        );
    }

    #[test]
    #[serial]
    fn test_expand_env_map_unknown_placeholder_is_left_intact() {
        unsafe {
            env::remove_var("UNKNOWN_PLACEHOLDER_TEST");
            env::remove_var("USES_UNKNOWN_TEST");
        }

        let mut merged: HashMap<String, String> = HashMap::new();
        merged.insert(
            "USES_UNKNOWN_TEST".to_string(),
            "value=${UNKNOWN_PLACEHOLDER_TEST}".to_string(),
        );

        let expanded = expand_env_map(&merged);

        let uses_unknown = expanded
            .get("USES_UNKNOWN_TEST")
            .expect("USES_UNKNOWN_TEST should be present after expansion");

        // Unknown placeholders should be preserved exactly
        assert_eq!(
            uses_unknown, "value=${UNKNOWN_PLACEHOLDER_TEST}",
            "Unknown placeholder should be left intact"
        );
    }

    #[test]
    #[serial]
    fn test_expand_env_map_preserves_quotes_around_values() {
        unsafe {
            env::remove_var("LOG_LEVEL_TEST");
            env::remove_var("RUST_LOG_QUOTED_TEST");
            env::remove_var("CRON_TEST");
        }

        let mut merged: HashMap<String, String> = HashMap::new();
        merged.insert("LOG_LEVEL_TEST".to_string(), "info".to_string());
        // placeholder inside double quotes
        merged.insert(
            "RUST_LOG_QUOTED_TEST".to_string(),
            " \"xtask=${LOG_LEVEL_TEST},server=${LOG_LEVEL_TEST}\" ".to_string(),
        );
        // value that contains spaces and is already quoted
        merged.insert("CRON_TEST".to_string(), "'0 0 0 * * *'".to_string());

        let expanded = expand_env_map(&merged);

        let rust_log = expanded
            .get("RUST_LOG_QUOTED_TEST")
            .expect("RUST_LOG_QUOTED_TEST should be present after expansion");
        let cron = expanded
            .get("CRON_TEST")
            .expect("CRON_TEST should be present after expansion");

        // RUST_LOG_QUOTED_TEST should still start and end with a double quote (after trimming)
        let rust_trimmed = rust_log.trim();
        assert!(
            rust_trimmed.starts_with('"') && rust_trimmed.ends_with('"'),
            "RUST_LOG_QUOTED_TEST should still be double-quoted, got: {rust_log}"
        );
        assert!(
            rust_trimmed.contains("xtask=info"),
            "RUST_LOG_QUOTED_TEST should contain the expanded value; got: {rust_trimmed}"
        );

        // CRON_TEST should be unchanged, quotes preserved
        assert_eq!(
            cron, "'0 0 0 * * *'",
            "CRON_TEST should keep its single quotes and content"
        );
    }
}
