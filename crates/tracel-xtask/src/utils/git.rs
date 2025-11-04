use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::Context;

pub fn repo_root_or_cwd() -> anyhow::Result<PathBuf> {
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
