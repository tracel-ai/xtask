use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, Stdio},
};

use anyhow::anyhow;
use rand::Rng;
use regex::Regex;

use crate::group_info;
use crate::{endgroup, group};

/// Run a process
pub fn run_process(
    name: &str,
    args: &Vec<&str>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let joined_args = args.join(" ");
    group_info!("Command line: {} {}", name, &joined_args);
    let mut command = Command::new(name);
    if let Some(path) = path {
        command.current_dir(path);
    }
    if let Some(envs) = envs {
        command.envs(&envs);
    }
    let status = command.args(args).status().map_err(|e| {
        anyhow!(
            "Failed to execute {} {}: {}",
            name,
            args.first().unwrap(),
            e
        )
    })?;
    if !status.success() {
        return Err(anyhow!("{}", error_msg));
    }
    anyhow::Ok(())
}

/// Run a process for workspace
/// regexp must have one capture group if defined
#[allow(clippy::too_many_arguments)]
pub fn run_process_for_workspace<'a>(
    name: &str,
    mut args: Vec<&'a str>,
    excluded: &'a [String],
    group_regexp: Option<&str>,
    group_name: Option<&str>,
    error_msg: &str,
    ignore_log: Option<&str>,
    ignore_msg: Option<&str>,
) -> anyhow::Result<()> {
    let group_rx: Option<Regex> = group_regexp.map(|r| Regex::new(r).unwrap());
    excluded
        .iter()
        .for_each(|ex| args.extend(["--exclude", ex]));
    group_info!("Command line: cargo {}", args.join(" "));
    let mut child = Command::new(name)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow!(format!(
                "Failed to start {} {}: {}",
                name,
                args.first().unwrap(),
                e
            ))
        })?;

    let mut ignore_error = false;
    let mut close_group = false;
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        reader.lines().for_each(|line| {
            if let Ok(line) = line {
                println!("{}", line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        reader.lines().for_each(|line| {
            let mut skip_line = false;
            if let Ok(line) = line {
                if let Some(rx) = &group_rx {
                    let cleaned_line = standardize_slashes(&remove_ansi_codes(&line));
                    if let Some(caps) = rx.captures(&cleaned_line) {
                        let crate_name = &caps[1];
                        if close_group {
                            endgroup!();
                        }
                        close_group = true;
                        group!("{}: {}", group_name.unwrap_or("Group"), crate_name);
                    }
                }
                if let Some(log) = ignore_log {
                    if line.contains(log) {
                        if let Some(msg) = ignore_msg {
                            warn!("{}", msg);
                        }
                        ignore_error = true;
                        skip_line = true;
                    }
                }
                if !skip_line {
                    eprintln!("{}", line);
                }
            }
        });
    }
    if close_group {
        endgroup!();
    }
    let status = child
        .wait()
        .expect("Should be able to wait for the process to finish.");
    if status.success() || ignore_error {
        anyhow::Ok(())
    } else {
        Err(anyhow!("{}", error_msg))
    }
}

/// Run a process command for a package
#[allow(clippy::too_many_arguments)]
pub fn run_process_for_package(
    name: &str,
    package: &String,
    args: &Vec<&str>,
    excluded: &[String],
    only: &[String],
    error_msg: &str,
    ignore_log: Option<&str>,
    ignore_msg: Option<&str>,
) -> anyhow::Result<()> {
    if excluded.contains(package) || (!only.is_empty() && !only.contains(package)) {
        group_info!("Skip '{}' because it has been excluded!", package);
        return anyhow::Ok(());
    }
    let joined_args = args.join(" ");
    group_info!("Command line: cargo {}", &joined_args);
    let output = Command::new("cargo")
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| anyhow!("Failed to execute process for '{}': {}", name, e))?;

    if output.status.success() {
        return anyhow::Ok(());
    } else if let Some(log) = ignore_log {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains(log) {
            if let Some(msg) = ignore_msg {
                warn!("{}", msg);
            }
            endgroup!();
            return anyhow::Ok(());
        }
    }
    Err(anyhow!("{}", error_msg))
}

/// Return a random port between 3000 and 9999
pub fn random_port() -> u16 {
    let mut rng = rand::thread_rng();
    rng.gen_range(3000..=9999)
}

fn remove_ansi_codes(s: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

fn standardize_slashes(s: &str) -> String {
    s.replace('\\', "/")
}
