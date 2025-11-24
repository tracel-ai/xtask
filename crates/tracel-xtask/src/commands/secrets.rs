/// Manage AWS Secrets Manager secrets.
use std::{collections::HashMap, fmt::Write as _, fs, io, path::PathBuf};

use crate::prelude::{Context, Environment};
use crate::utils::aws::cli::{
    secretsmanager_create_empty_secret, secretsmanager_get_secret_string,
    secretsmanager_put_secret_string,
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

    /// Secret name to create (metadata only, no initial version)
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
        SecretsSubCommand::View(view_args) => view(view_args),
    }
}

/// create an empty secret (metadata only, no version).
fn create(args: SecretsCreateSubCmdArgs) -> anyhow::Result<()> {
    secretsmanager_create_empty_secret(&args.secret_id, &args.region, args.description.as_deref())?;

    eprintln!(
        "✅ Created empty secret '{}' in region '{}'.",
        args.secret_id, args.region
    );
    Ok(())
}

/// Copy a secret value from one secret ID to another in the same region.
fn copy(args: SecretsCopySubCmdArgs) -> anyhow::Result<()> {
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
    let value = secretsmanager_get_secret_string(&args.from, &args.region)?;

    // Check if the target already has a current version.
    // If we can successfully fetch it, we consider that a current version exists
    // and ask for confirmation before creating a new one.
    let target_has_version = secretsmanager_get_secret_string(&args.to, &args.region).is_ok();
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
    let original_raw = secretsmanager_get_secret_string(&args.secret_id, &args.region)?;
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
        if !confirm_push()? {
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
    if !confirm_push()? {
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
        let s = secretsmanager_get_secret_string(id, &args.region)?;
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

/// `view` subcommand: fetch and print the secret.
fn view(args: SecretsViewSubCmdArgs) -> anyhow::Result<()> {
    let value = secretsmanager_get_secret_string(&args.secret_id, &args.region)?;
    println!("{value}");
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
