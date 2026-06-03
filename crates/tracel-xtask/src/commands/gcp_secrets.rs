/// Manage GCP Secret Manager secrets.
use std::{collections::HashMap, fmt::Write as _, fs, io, path::PathBuf};

use anyhow::Context as _;

use tracel_xtask_utils::{
    environment::Environment,
    gcp::cli::{
        secret_manager_create_secret, secret_manager_get_secret_string,
        secret_manager_list_secret_versions_json, secret_manager_put_secret_string,
    },
};

use crate::context::Context;

const FALLBACK_EDITOR: &str = "vi";

#[tracel_xtask_macros::declare_command_args(None, GcpSecretsSubCommand)]
pub struct GcpSecretsCmdArgs {}

impl Default for GcpSecretsSubCommand {
    fn default() -> Self {
        GcpSecretsSubCommand::View(GcpSecretsViewSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsCreateSubCmdArgs {
    /// GCP project where the secret will be created
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Secret name to create with an initial empty JSON value
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
    /// Optional comma-separated user-managed replication locations.
    /// If omitted, automatic replication is used.
    #[arg(long)]
    pub replication_locations: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsCopySubCmdArgs {
    /// GCP project where the secrets live
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Source secret identifier
    #[arg(long, value_name = "FROM_SECRET_ID")]
    pub from: String,
    /// Target secret identifier
    #[arg(long, value_name = "TO_SECRET_ID")]
    pub to: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsEditSubCmdArgs {
    /// GCP project where the secret lives
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Secret identifier
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
    /// Push the new secret version without asking for confirmation
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsEnvFileSubCmdArgs {
    /// Output file path. If omitted, writes to stdout.
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
    /// GCP project where the secrets live
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Secret identifiers, can provide multiple ones.
    #[arg(value_name = "SECRET_ID", num_args(1..), required = true)]
    pub secret_ids: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsListSubCmdArgs {
    /// GCP project where the secret lives
    #[arg(long)]
    pub project: String,

    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,

    /// Secret identifier
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpSecretsPushSubCmdArgs {
    /// GCP project where the secret lives
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Secret identifier
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
pub struct GcpSecretsViewSubCmdArgs {
    /// GCP project where the secret lives
    #[arg(long)]
    pub project: String,
    /// Optional GCP Secret Manager location for regional secrets.
    /// Omit for regular project-scoped secrets.
    #[arg(long)]
    pub location: Option<String>,
    /// Secret identifier
    #[arg(value_name = "SECRET_ID")]
    pub secret_id: String,
}

pub fn handle_command(
    args: GcpSecretsCmdArgs,
    _env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        GcpSecretsSubCommand::Create(create_args) => create(create_args),
        GcpSecretsSubCommand::Copy(copy_args) => copy(copy_args),
        GcpSecretsSubCommand::Edit(edit_args) => edit(edit_args),
        GcpSecretsSubCommand::EnvFile(env_args) => env_file(env_args),
        GcpSecretsSubCommand::List(list_args) => list(list_args),
        GcpSecretsSubCommand::Push(push_args) => push(push_args),
        GcpSecretsSubCommand::View(view_args) => view(view_args),
    }
}

/// Create a secret and attach an initial empty JSON (`{}`) version as plain text.
fn create(args: GcpSecretsCreateSubCmdArgs) -> anyhow::Result<()> {
    secret_manager_create_secret(
        &args.secret_id,
        &args.project,
        args.location.as_deref(),
        args.replication_locations.as_deref(),
    )?;

    secret_manager_put_secret_string(
        &args.secret_id,
        &args.project,
        args.location.as_deref(),
        "{}",
    )?;

    eprintln!(
        "✅ Created secret '{}' in project '{}' with an initial empty JSON value.",
        args.secret_id, args.project
    );

    Ok(())
}

/// Copy a secret value from one secret ID to another in the same project/location.
pub fn copy(args: GcpSecretsCopySubCmdArgs) -> anyhow::Result<()> {
    if args.from == args.to {
        eprintln!(
            "Source and target secrets are identical ('{}'), nothing to do.",
            args.from
        );
        return Ok(());
    }

    eprintln!(
        "Fetching source secret '{}' in project '{}'...",
        args.from, args.project
    );

    let value =
        secret_manager_get_secret_string(&args.from, &args.project, args.location.as_deref())?;

    let target_has_version =
        secret_manager_get_secret_string(&args.to, &args.project, args.location.as_deref()).is_ok();

    if target_has_version {
        eprintln!(
            "Secret '{}' already has a current version in project '{}'.",
            args.to, args.project
        );

        if !confirm_push()? {
            eprintln!("Aborting: new secret version was not pushed.");
            return Ok(());
        }
    }

    eprintln!(
        "Writing target secret '{}' in project '{}'...",
        args.to, args.project
    );

    secret_manager_put_secret_string(&args.to, &args.project, args.location.as_deref(), &value)?;

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
fn edit(args: GcpSecretsEditSubCmdArgs) -> anyhow::Result<()> {
    let original_raw =
        secret_manager_get_secret_string(&args.secret_id, &args.project, args.location.as_deref())?;
    let original_raw_trimmed = original_raw.trim_end_matches('\n');

    let to_edit =
        pretty_json(original_raw_trimmed).unwrap_or_else(|| original_raw_trimmed.to_string());

    let tmp_path = temp_file_path(&args.secret_id);
    fs::write(&tmp_path, &to_edit)?;

    eprintln!(
        "Editing secret '{}' in project '{}' using temporary file:\n  {}",
        args.secret_id,
        args.project,
        tmp_path.display()
    );

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

    let edited_raw = fs::read_to_string(&tmp_path)?;
    fs::remove_file(&tmp_path).ok();

    let edited_raw_trimmed = edited_raw.trim_end_matches('\n');

    let original_norm_json = normalize_json(original_raw_trimmed);
    let edited_norm_json = normalize_json(edited_raw_trimmed);

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

        secret_manager_put_secret_string(
            &args.secret_id,
            &args.project,
            args.location.as_deref(),
            &edited_norm,
        )?;

        eprintln!(
            "✅ New JSON version pushed for secret '{}' in project '{}'.",
            args.secret_id, args.project
        );

        return Ok(());
    }

    if edited_raw_trimmed == original_raw_trimmed {
        eprintln!("No changes detected, not pushing a new secret version.");
        return Ok(());
    }

    eprintln!("Secret content has changed.");

    if !args.yes && !confirm_push()? {
        eprintln!("Aborting: new secret version was not pushed.");
        return Ok(());
    }

    secret_manager_put_secret_string(
        &args.secret_id,
        &args.project,
        args.location.as_deref(),
        edited_raw_trimmed,
    )?;

    eprintln!(
        "✅ New version pushed for secret '{}' in project '{}'.",
        args.secret_id, args.project
    );

    Ok(())
}

pub fn env_file(args: GcpSecretsEnvFileSubCmdArgs) -> anyhow::Result<()> {
    if args.secret_ids.is_empty() {
        eprintln!("No secrets provided.");
        return Ok(());
    }

    let mut merged: HashMap<String, String> = HashMap::new();

    for id in &args.secret_ids {
        eprintln!("Fetching secret '{id}'...");

        let s = secret_manager_get_secret_string(id, &args.project, args.location.as_deref())?;
        let s = s.trim();

        if s.is_empty() {
            continue;
        }

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

    let merged = expand_env_map(&merged);

    let mut entries: Vec<(String, String)> = merged.into_iter().collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

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

/// List all versions of the secret.
fn list(args: GcpSecretsListSubCmdArgs) -> anyhow::Result<()> {
    eprintln!(
        "Listing versions for secret '{}' in project '{}'...",
        args.secret_id, args.project
    );

    let json = secret_manager_list_secret_versions_json(
        &args.secret_id,
        &args.project,
        args.location.as_deref(),
    )?;

    let versions: serde_json::Value = serde_json::from_str(&json)
        .context("Parsing Secret Manager versions JSON should succeed")?;

    let versions = versions.as_array().ok_or_else(|| {
        anyhow::anyhow!(
            "GCP response for secret '{}' should be a JSON array",
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
        state: String,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut id_w = "VersionId".len();
    let mut created_w = "Created".len();
    let mut state_w = "State".len();

    for ver in versions {
        let name = ver.get("name").and_then(|x| x.as_str()).unwrap_or("");

        let id = name
            .rsplit('/')
            .next()
            .filter(|x| !x.is_empty())
            .unwrap_or(name)
            .to_string();

        let created = ver
            .get("createTime")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        let state = ver
            .get("state")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        id_w = id_w.max(id.len());
        created_w = created_w.max(created.len());
        state_w = state_w.max(state.len());

        rows.push(Row { id, created, state });
    }

    println!(
        "{:<id_w$}  {:<created_w$}  {:<state_w$}",
        "VersionId",
        "Created",
        "State",
        id_w = id_w,
        created_w = created_w,
        state_w = state_w,
    );

    println!(
        "{:-<id_w$}  {:-<created_w$}  {:-<state_w$}",
        "",
        "",
        "",
        id_w = id_w,
        created_w = created_w,
        state_w = state_w,
    );

    for r in rows {
        println!(
            "{:<id_w$}  {:<created_w$}  {:<state_w$}",
            r.id,
            r.created,
            r.state,
            id_w = id_w,
            created_w = created_w,
            state_w = state_w,
        );
    }

    Ok(())
}

/// Push updates to a JSON secret by setting one or more KEY=VALUE pairs.
/// The secret must be a JSON object and updated value is stored on a single line.
pub fn push(args: GcpSecretsPushSubCmdArgs) -> anyhow::Result<()> {
    eprintln!(
        "Fetching secret '{}' in project '{}'...",
        args.secret_id, args.project
    );

    let original =
        secret_manager_get_secret_string(&args.secret_id, &args.project, args.location.as_deref())?;

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

        if let Some(existing_val) = existing {
            if existing_val.is_string() && existing_val.as_str() == Some(val) {
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

    eprintln!("Secret JSON content has changed.");

    if !args.yes && !confirm_push()? {
        eprintln!("Aborting: new secret version was not pushed.");
        return Ok(());
    }

    let normalized =
        serde_json::to_string(&value).context("Serializing updated JSON secret should succeed")?;

    secret_manager_put_secret_string(
        &args.secret_id,
        &args.project,
        args.location.as_deref(),
        &normalized,
    )?;

    eprintln!(
        "✅ Updated secret '{}' in project '{}' with {} key(s).",
        args.secret_id,
        args.project,
        args.kv.len()
    );

    Ok(())
}

/// Fetch and print the secret.
fn view(args: GcpSecretsViewSubCmdArgs) -> anyhow::Result<()> {
    let value =
        secret_manager_get_secret_string(&args.secret_id, &args.project, args.location.as_deref())?;

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
    let filename = format!("tracel-gcp-secret-{base}-{pid}.tmp");

    std::env::temp_dir().join(filename)
}

/// Detect the editor to use $VISUAL then $EDITOR then falling back to "vi".
fn detect_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| FALLBACK_EDITOR.to_string())
}

/// Try to pretty-print JSON.
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
        out.push_str(&rest[..start]);

        let after = &rest[start + 2..];

        if let Some(end_rel) = after.find('}') {
            let var_name = &after[..end_rel];

            if let Some(val) = vars.get(var_name) {
                out.push_str(val);
            } else {
                out.push_str("${");
                out.push_str(var_name);
                out.push('}');
            }

            rest = &after[end_rel + 1..];
        } else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }

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
        merged.insert(
            "RUST_LOG_QUOTED_TEST".to_string(),
            " \"xtask=${LOG_LEVEL_TEST},server=${LOG_LEVEL_TEST}\" ".to_string(),
        );
        merged.insert("CRON_TEST".to_string(), "'0 0 0 * * *'".to_string());

        let expanded = expand_env_map(&merged);

        let rust_log = expanded
            .get("RUST_LOG_QUOTED_TEST")
            .expect("RUST_LOG_QUOTED_TEST should be present after expansion");
        let cron = expanded
            .get("CRON_TEST")
            .expect("CRON_TEST should be present after expansion");

        let rust_trimmed = rust_log.trim();

        assert!(
            rust_trimmed.starts_with('"') && rust_trimmed.ends_with('"'),
            "RUST_LOG_QUOTED_TEST should still be double-quoted, got: {rust_log}"
        );
        assert!(
            rust_trimmed.contains("xtask=info"),
            "RUST_LOG_QUOTED_TEST should contain the expanded value; got: {rust_trimmed}"
        );

        assert_eq!(
            cron, "'0 0 0 * * *'",
            "CRON_TEST should keep its single quotes and content"
        );
    }
}
