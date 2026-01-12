use anyhow::Context as _;
use std::{fs, io::Write as _, path::PathBuf};

use crate::{context::Context, prelude::Environment, utils::terraform};

const DEFAULT_PATH: &str = "./infra";

#[tracel_xtask_macros::declare_command_args(None, InfraSubCommand)]
pub struct InfraCmdArgs {
    /// Path where to generate or read the infra configuration.
    #[arg(long, default_value = DEFAULT_PATH)]
    pub path: PathBuf,

    /// Path to the Terraform plan file used by `plan` and `apply`.
    #[arg(long, default_value = "tfplan")]
    pub out: PathBuf,
}

#[derive(clap::Args, Clone, Default, PartialEq)]
struct InfraInstallSubCmdArgs {
    /// Install a specific Terraform version (e.g. 1.9.6) and update the lockfile to that version
    #[arg(long)]
    version: Option<String>,
}

#[derive(clap::Args, Clone, Default, PartialEq)]
pub struct InfraOutputSubCmdArgs {
    /// If specified, use JSON format for outputs format
    #[arg(short, long)]
    json: bool,
}

#[derive(clap::Args, Clone, Default, PartialEq)]
pub struct InfraProvidersSubCmdArgs {
    /// The command to pass to provider.
    command: TerraformProvidersCommand,
}

#[derive(clap::ValueEnum, Copy, Clone, Debug, Default, PartialEq)]
pub enum TerraformProvidersCommand {
    #[default]
    /// Output the provider schema in JSON format.
    Schema,
}

#[derive(clap::Args, Clone, Default, PartialEq)]
struct InfraUninstallSubCmdArgs {
    /// Uninstall all installed Terraform binaries under ~/.cache/xtask/terraform
    #[arg(long)]
    all: bool,
    /// List installed terraform versions and exit
    #[arg(short, long)]
    list: bool,
    /// Uninstall a specific Terraform version (e.g. 1.9.6)
    #[arg(long)]
    version: Option<String>,
}

pub fn handle_command(args: InfraCmdArgs, _env: Environment, _ctx: Context) -> anyhow::Result<()> {
    match args.get_command() {
        InfraSubCommand::Apply => {
            apply(&args)?;
            Ok(())
        }
        InfraSubCommand::Destroy => destroy(&args),
        InfraSubCommand::Init => init(&args),
        InfraSubCommand::Install(cmd_args) => install(&cmd_args),
        InfraSubCommand::List => list(),
        InfraSubCommand::Output(cmd_args) => output(&args, &cmd_args),
        InfraSubCommand::Providers(cmd_args) => providers(&args, &cmd_args),
        InfraSubCommand::Plan => plan(&args),
        InfraSubCommand::Uninstall(cmd_args) => uninstall(&cmd_args),
        InfraSubCommand::Update => update(),
    }
}

// Commands ------------------------------------------------------------------

/// Returns true if the user confirmed to apply the plan
pub fn apply(args: &InfraCmdArgs) -> anyhow::Result<bool> {
    let out = args.out.to_string_lossy().to_string();

    // 1) Run plan
    let tf_args = ["plan", "-out", out.as_str()];
    terraform::call_terraform(&args.path, &tf_args)?;

    // 2) Ask the user if they want to run apply.
    eprintln!();
    eprint!("Apply this Terraform plan? [y/N]: ");
    std::io::stderr().flush()?;

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("Failed to read confirmation from stdin")?;

    let answer = answer.trim().to_ascii_lowercase();
    let proceed = matches!(answer.as_str(), "y" | "yes");

    if !proceed {
        eprintln!("Skipping apply.");
        return Ok(false);
    }

    // 3) User approved: apply the *saved* plan file (no re-planning).
    let apply_args = ["apply", "-auto-approve", out.as_str()];
    terraform::call_terraform(&args.path, &apply_args)?;

    Ok(true)
}

pub fn destroy(args: &InfraCmdArgs) -> anyhow::Result<()> {
    terraform::call_terraform(&args.path, &["destroy"])
}

pub fn init(args: &InfraCmdArgs) -> anyhow::Result<()> {
    terraform::call_terraform(&args.path, &["init"])
}

fn install(args: &InfraInstallSubCmdArgs) -> anyhow::Result<()> {
    let agent = ureq::agent();
    let repo_root = std::env::current_dir().context("Failed to get current directory")?;

    // Decide version + lock policy
    enum LockAction<'a> {
        /// Do not touch existing lock (already present).
        Keep,
        /// Write a new lock because there wasn't one.
        WriteNew(&'a str),
        /// Overwrite/update lock to this version (explicit user request).
        WriteUpdate(&'a str),
    }

    let (version, lock_action) = if let Some(explicit) = args.version.as_deref() {
        // Explicit version requested: install and update lockfile unconditionally
        (explicit.to_string(), LockAction::WriteUpdate(explicit))
    } else {
        // No explicit version: follow lock if present, otherwise install latest
        if let Some(locked) = terraform::read_locked_version(&repo_root)? {
            (locked, LockAction::Keep)
        } else {
            let latest = terraform::fetch_latest_version(&agent)?;
            (
                latest.clone(),
                LockAction::WriteNew(Box::leak(latest.into_boxed_str())),
            )
        }
    };

    let dest = terraform::terraform_bin_path(&version)?;
    if dest.exists() {
        eprintln!(
            "terraform {} already installed at {}",
            version,
            dest.display()
        );
    } else {
        eprintln!("Installing terraform {}...", version);
        let bytes = terraform::download_terraform_zip(&agent, &version)?;
        terraform::extract_and_install(&bytes, &dest)?;
        eprintln!("Installed terraform {} to {}", version, dest.display());
    }

    // Apply lock policy
    match lock_action {
        LockAction::Keep => { /* do nothing */ }
        LockAction::WriteNew(v) | LockAction::WriteUpdate(v) => {
            terraform::write_lockfile(&repo_root, v)?;
            eprintln!("Wrote {} with version {v}", terraform::LOCKFILE);
        }
    }

    Ok(())
}

fn list() -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().context("Failed to get current directory")?;
    let locked = terraform::read_locked_version(&repo_root)?;
    terraform::print_installed_versions_with_lock(&locked)
}

pub fn output(args: &InfraCmdArgs, output_args: &InfraOutputSubCmdArgs) -> anyhow::Result<()> {
    let mut tf_args = vec!["output"];
    if output_args.json {
        tf_args.push("-json");
    }
    terraform::call_terraform(&args.path, &tf_args)
}

pub fn plan(args: &InfraCmdArgs) -> anyhow::Result<()> {
    let out = args.out.to_string_lossy().to_string();
    let tf_args = ["plan", "-out", out.as_str()];
    terraform::call_terraform(&args.path, &tf_args)
}

fn providers(args: &InfraCmdArgs, provider_args: &InfraProvidersSubCmdArgs) -> anyhow::Result<()> {
    let mut tf_args = vec!["providers"];
    match provider_args.command {
        TerraformProvidersCommand::Schema => {
            tf_args.extend(vec!["schema", "-json", "-no-color"]);
            terraform::call_terraform(&args.path, &tf_args)
        }
    }
}

fn uninstall(args: &InfraUninstallSubCmdArgs) -> anyhow::Result<()> {
    let repo_root = std::env::current_dir().context("Failed to get current directory")?;

    // --list, print installed versions
    if args.list {
        let locked = terraform::read_locked_version(&repo_root)?;
        return terraform::print_installed_versions_with_lock(&locked);
    }

    // --all, remove everything
    if args.all {
        let removed = terraform::uninstall_all_versions()?;
        if removed == 0 {
            eprintln!(
                "No terraform binaries found in {}",
                terraform::terraform_install_dir()?.display()
            );
        } else {
            eprintln!("Removed {} terraform binaries.", removed);
        }
        // Remove lockfile if present
        let lf = terraform::lockfile_path(&repo_root);
        if lf.exists() {
            fs::remove_file(&lf).ok();
            eprintln!("Removed {}", lf.display());
        }
        return Ok(());
    }

    // --version, uninstall specific version
    if let Some(ver) = &args.version {
        let path = terraform::terraform_bin_path(ver)?;
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            eprintln!("Removed {}", path.display());
            // If lockfile matches this version, remove it too.
            if terraform::read_locked_version(&repo_root)?.as_deref() == Some(ver.as_str()) {
                let lf = terraform::lockfile_path(&repo_root);
                if lf.exists() {
                    fs::remove_file(&lf).ok();
                    eprintln!("Removed {}", lf.display());
                }
            }
        } else {
            eprintln!("Terraform {} not found at {}", ver, path.display());
        }
        return Ok(());
    }

    // default if no option is provided:
    // a) if lock exists, uninstall that version and delete the lockfile.
    if let Some(locked) = terraform::read_locked_version(&repo_root)? {
        let path = terraform::terraform_bin_path(&locked)?;
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            eprintln!("Removed {}", path.display());
        } else {
            eprintln!(
                "Locked terraform {} not found at {}",
                locked,
                path.display()
            );
        }
        // Remove lockfile
        let lf = terraform::lockfile_path(&repo_root);
        if lf.exists() {
            fs::remove_file(&lf).ok();
            eprintln!("Removed {}", lf.display());
        }
        return Ok(());
    }
    // b) if no lock and exactly one version installed, uninstall that one.
    let installed = terraform::list_installed_versions()?;
    match installed.len() {
        0 => {
            eprintln!(
                "No terraform binaries found in {}",
                terraform::terraform_install_dir()?.display()
            );
            Ok(())
        }
        1 => {
            let (ver, path) = &installed[0];
            fs::remove_file(path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            eprintln!("Removed {} ({})", path.display(), ver);
            Ok(())
        }
        // c) if multiple versions installed and no lock, list them and exit without action.
        _ => {
            eprintln!(
                "Multiple terraform versions are installed; specify one with --version or use --all:"
            );
            for (ver, path) in installed {
                eprintln!("  {ver}\t{}", path.display());
            }
            Ok(())
        }
    }
}

fn update() -> anyhow::Result<()> {
    let agent = ureq::agent();
    let repo_root = std::env::current_dir().context("Failed to get current directory")?;
    let latest = terraform::fetch_latest_version(&agent)?;
    let locked = terraform::read_locked_version(&repo_root)?;

    if locked.as_deref() == Some(latest.as_str()) {
        eprintln!("Terraform is already at latest: {}", latest);
    } else {
        let dest = terraform::terraform_bin_path(&latest)?;
        if dest.exists() {
            eprintln!(
                "terraform {} already installed at {}",
                latest,
                dest.display()
            );
        } else {
            eprintln!("Installing terraform {}...", &latest);
            let bytes = terraform::download_terraform_zip(&agent, &latest)?;
            terraform::extract_and_install(&bytes, &dest)?;
            eprintln!("Installed terraform {} to {}", latest, dest.display());
        }

        terraform::write_lockfile(&repo_root, &latest)?;
        match locked {
            Some(prev) => eprintln!("Updated {} from {prev} -> {latest}", terraform::LOCKFILE),
            None => eprintln!("Wrote {} with version {latest}", terraform::LOCKFILE),
        }
    }

    Ok(())
}
