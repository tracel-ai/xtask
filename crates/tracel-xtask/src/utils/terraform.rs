use std::{
    fs,
    io::{Cursor, Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use home::home_dir;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::prelude::{Environment, ExplicitIndex};

use super::process::run_process;

pub const LOCKFILE: &str = ".infra.lock";

/// Global absolute path to the terraform binary for a given version
/// with the version in the filename.
/// Example (Linux): ~/.cache/xtask/terraform/terraform-1.9.6
/// Example (Windows): %USERPROFILE%\.cache\xtask\terraform\terraform-1.9.6.exe
pub fn terraform_bin_path(version: &str) -> anyhow::Result<PathBuf> {
    let name = format!("terraform-{}{}", version, exe_suffix());
    Ok(terraform_install_dir()?.join(name))
}

/// Return the installation directory of terraform versions
pub fn terraform_install_dir() -> anyhow::Result<PathBuf> {
    let home = home_dir().ok_or_else(|| anyhow::anyhow!("Could not resolve HOME directory"))?;
    Ok(home.join(".cache/xtask/terraform"))
}

/// Source of truth to retrieve the state path
pub fn state_path(
    base_path: &PathBuf,
    infra_env: &Environment<ExplicitIndex>,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(base_path)?;
    let path = base_path.join(infra_env.medium());
    Ok(path)
}

/// Call terraform with a command
pub fn call_terraform(
    path: &PathBuf,
    env: &Environment<ExplicitIndex>,
    args: &[&str],
) -> anyhow::Result<()> {
    // info!("Generating up to date config...");
    // let config_args = ConfigSubCmdArgs {
    //     common: cmd_args.clone(),
    //     show: false,
    // };
    // config(&config_args, env)?;
    let repo_root = std::env::current_dir().context("Failed to get current directory")?;
    let tf = locked_terraform_path(&repo_root)?;
    let workdir = state_path(path, env)?;
    run_process(
        tf.as_str(),
        args,
        None,
        Some(&workdir),
        "Error during terraform init.",
    )
}

/// Return path of lock file for terraform pinned version
pub fn lockfile_path(repo_root: &Path) -> PathBuf {
    repo_root.join(LOCKFILE)
}

/// Write a lock file containing a pinned version of terraform
pub fn write_lockfile(repo_root: &Path, version: &str) -> anyhow::Result<()> {
    let lock_path = lockfile_path(repo_root);
    let tmp_path = lock_path.with_extension("lock.tmp");

    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Creating {}", parent.display()))?;
    }
    // file content
    let lock = Lockfile {
        terraform: TerraformSection {
            version: version.to_string(),
        },
    };
    // write to temp file and atomically rename
    let content = toml::to_string_pretty(&lock).context("Failed to serialize lockfile to TOML")?;
    {
        let mut f = fs::File::create(&tmp_path)
            .with_context(|| format!("Creating {}", tmp_path.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("Writing {}", tmp_path.display()))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp_path, &lock_path)
        .with_context(|| format!("Renaming {} -> {}", tmp_path.display(), lock_path.display()))?;

    Ok(())
}

/// Read the locked file where the version of terraform is
pub fn read_locked_version(repo_root: &Path) -> anyhow::Result<Option<String>> {
    let p = lockfile_path(repo_root);
    if !p.exists() {
        return Ok(None);
    }
    let s = fs::read_to_string(&p).with_context(|| format!("Failed to read {}", p.display()))?;
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let lf: Lockfile = toml::de::from_str(s)
        .with_context(|| format!("Failed to parse TOML in {}", p.display()))?;
    Ok(Some(lf.terraform.version))
}

/// Latest Terraform version via HashCorp checkpoint API.
/// see https://checkpoint-api.hashicorp.com
pub fn fetch_latest_version(client: &Client) -> anyhow::Result<String> {
    let url = "https://checkpoint-api.hashicorp.com/v1/check/terraform";
    let resp: CheckpointResponse = client
        .get(url)
        .send()
        .context("Failed to query HashCorp checkpoint API")?
        .error_for_status()
        .context("Non-success status from checkpoint API")?
        .json()
        .context("Failed to parse checkpoint API JSON")?;
    Ok(resp.current_version)
}

/// Download terraform archive from hashicorp
pub fn download_terraform_zip(client: &Client, version: &str) -> anyhow::Result<Vec<u8>> {
    let os = terraform_target_os();
    let arch = terraform_target_arch();
    let url = format!(
        "https://releases.hashicorp.com/terraform/{v}/terraform_{v}_{os}_{arch}.zip",
        v = version,
        os = os,
        arch = arch
    );
    let resp = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to download {url}"))?
        .error_for_status()
        .with_context(|| format!("Non-success status while downloading {url}"))?;

    let bytes = resp.bytes().context("Failed to read body")?;
    Ok(bytes.to_vec())
}

/// Install terraform archive contents
pub fn extract_and_install(zip_bytes: &[u8], dest_path: &Path) -> anyhow::Result<()> {
    let reader = Cursor::new(zip_bytes);
    let mut zip = ZipArchive::new(reader).context("Failed to read ZIP archive")?;

    // Terraform zips contain a single file named "terraform" or "terraform.exe"
    let entry_name = format!("terraform{}", exe_suffix());
    let mut file = zip
        .by_name(&entry_name)
        .with_context(|| format!("Archive did not contain {}", entry_name))?;

    // Write file
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("Creating {}", parent.display()))?;
    }
    let mut out =
        fs::File::create(dest_path).with_context(|| format!("Creating {}", dest_path.display()))?;
    let mut buf = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut buf)
        .context("Reading file from ZIP")?;
    out.write_all(&buf).context("Writing terraform binary")?;
    drop(out);

    #[cfg(unix)]
    {
        // Make it executable
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(dest_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(dest_path, perms)?;
    }

    Ok(())
}

/// Return al list of all installed version of terraform
pub fn list_installed_versions() -> anyhow::Result<Vec<(String, PathBuf)>> {
    let dir = terraform_install_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("Reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // filenames like terraform-1.9.6
        if let Some(fname) = path.file_name().and_then(|s| s.to_str())
            && let Some(ver) = parse_version_from_filename(fname)
        {
            out.push((ver, path));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// List all the installed versions of terraform
pub fn print_installed_versions_with_lock(lock: &Option<String>) -> anyhow::Result<()> {
    let installed = list_installed_versions()?;
    if installed.is_empty() {
        eprintln!(
            "No terraform binaries found in {}",
            terraform_install_dir()?.display()
        );
        return Ok(());
    }
    eprintln!("Installed terraform versions (* means locked version):");
    for (ver, path) in installed {
        let marker = if lock.as_deref() == Some(ver.as_str()) {
            "(*) "
        } else {
            "    "
        };
        eprintln!("{marker}{ver}\t{}", path.display());
    }
    Ok(())
}

/// Uninstall all installed versions of terraform
pub fn uninstall_all_versions() -> anyhow::Result<usize> {
    let installed = list_installed_versions()?;
    let mut count = 0usize;
    for (_ver, path) in installed {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            count += 1;
        }
    }
    Ok(count)
}

// Paths ---------------------------------------------------------------------

/// Returns path to currently locked version of terraform
fn locked_terraform_path(repo_root: &Path) -> anyhow::Result<String> {
    let ver = read_locked_version(repo_root)?.ok_or_else(|| {
        anyhow::anyhow!("No locked Terraform version found. Run `xtask infra install` first.")
    })?;

    let bin = terraform_bin_path(&ver)?;
    if !bin.exists() {
        return Err(anyhow::anyhow!(
            "Locked Terraform {} is not installed at {}. Run `xtask infra install --version {ver}`.",
            ver,
            bin.display()
        ));
    }
    Ok(bin.to_string_lossy().into_owned())
}

// Version -------------------------------------------------------------------

#[derive(Deserialize)]
struct CheckpointResponse {
    current_version: String,
}

#[derive(Serialize, Deserialize)]
struct Lockfile {
    terraform: TerraformSection,
}

#[derive(Serialize, Deserialize)]
struct TerraformSection {
    version: String,
}

fn parse_version_from_filename(fname: &str) -> Option<String> {
    // Accepts "terraform-<ver>" or "terraform-<ver>.exe"
    if let Some(rest) = fname.strip_prefix("terraform-") {
        let ver = rest.strip_suffix(".exe").unwrap_or(rest);
        if !ver.is_empty() {
            return Some(ver.to_string());
        }
    }
    None
}

// OS stuff ------------------------------------------------------------------

fn terraform_target_os() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => other,
    }
}

fn terraform_target_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => other,
    }
}

fn exe_suffix() -> &'static str {
    if std::env::consts::OS == "windows" {
        ".exe"
    } else {
        ""
    }
}
