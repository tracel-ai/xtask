use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::Context;

pub fn git_repo_root_or_cwd() -> anyhow::Result<PathBuf> {
    let out = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8(o.stdout).context("utf8 git rev-parse output")?;
            Ok(PathBuf::from(s.trim()))
        }
        _ => std::env::current_dir().context("getting current directory"),
    }
}

/// Returns the current commit hash of `HEAD`.
pub fn git_current_commit_hash(len: Option<usize>) -> anyhow::Result<String> {
    let cwd = git_repo_root_or_cwd()?;
    let out = Command::new("git")
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("running `git rev-parse --verify HEAD`")?;

    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let s = String::from_utf8(out.stdout).context("utf8 git rev-parse output")?;
    let sha = s.trim().to_string();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!("unexpected commit hash format: {sha}");
    }

    let max_len = 40;
    let final_len = len.map_or(max_len, |n| n.min(max_len));

    Ok(sha[..final_len].to_string())
}

/// Return true if the current repository is dirty
pub fn git_is_repo_dirty() -> anyhow::Result<bool> {
    let cwd = git_repo_root_or_cwd()?;
    let out = Command::new("git")
        .arg("status")
        .arg("--porcelain") // stable machine-readable output
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("running `git status --porcelain`")?;

    if !out.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let s = String::from_utf8(out.stdout).context("utf8 git status output")?;
    Ok(!s.trim().is_empty())
}
