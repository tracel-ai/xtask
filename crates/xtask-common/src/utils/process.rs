use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

use anyhow::anyhow;
use rand::Rng;
use regex::Regex;

use crate::{group_info, utils::get_command_line_from_command};
use crate::{endgroup, group};

/// Run a process
pub fn run_process(
    name: &str,
    args: &Vec<&str>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let joined_args = args.join(" ");
    group_info!("Command line: {} {}", name, &joined_args);
    let status = Command::new(name).args(args).status().map_err(|e| {
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
pub fn run_process_for_workspace<'a>(
    name: &str,
    mut args: Vec<&'a str>,
    excluded: &'a [String],
    error_msg: &str,
    group_regexp: Option<&str>,
    group_name: Option<&str>,
) -> anyhow::Result<()> {
    let re: Option<Regex> = group_regexp.map(|r| Regex::new(r).unwrap());
    excluded
        .iter()
        .for_each(|ex| args.extend(["--exclude", ex]));
    group_info!("Command line: cargo {}", args.join(" "));
    let mut child = Command::new(name)
        .args(&args)
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

    let mut close_group = false;
    if let Some(stderr) = child.stderr.take() {
        let reader = BufReader::new(stderr);
        reader.lines().for_each(|line| {
            if let Ok(line) = line {
                if let Some(rx) = &re {
                    let cleaned_line = standardize_slashes(&remove_ansi_codes(&line));
                    if let Some(caps) = rx.captures(&cleaned_line) {
                        let crate_name = &caps[1];
                        if close_group {
                            endgroup!();
                        }
                        group!("{}: {}", group_name.unwrap_or("Group"), crate_name);
                    }
                }
                eprintln!("{}", line);
                close_group = true;
            }
        });
    }
    if close_group {
        endgroup!();
    }
    let status = child
        .wait()
        .expect("Should be able to wait for the process to finish.");
    if status.success() {
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

/// Spawn a process from passed command
pub fn run_process_command(command: &mut Command, error: &str) -> anyhow::Result<()> {
    // Handle cargo child process
    let command_line = get_command_line_from_command(command);
    group_info!("{command_line}\n");
    let process = command.spawn().expect(error);
    let error = format!(
        "{} process should run flawlessly",
        command.get_program().to_str().unwrap()
    );
    handle_child_process(process, &error)
}

/// Run a command
pub fn run_command(
    command: &str,
    args: &[&str],
    command_error: &str,
    child_error: &str,
) -> anyhow::Result<()> {
    // Format command
    group_info!("{command} {}\n\n", args.join(" "));

    // Run command as child process
    let command = Command::new(command)
        .args(args)
        .stdout(Stdio::inherit()) // Send stdout directly to terminal
        .stderr(Stdio::inherit()) // Send stderr directly to terminal
        .spawn()
        .expect(command_error);

    // Handle command child process
    handle_child_process(command, child_error)
}

/// Handle child process
pub fn handle_child_process(mut child: Child, error: &str) -> anyhow::Result<()> {
    // Wait for the child process to finish
    let status = child.wait().expect(error);

    // If exit status is not a success, terminate the process with an error
    if !status.success() {
        // Use the exit code associated to a command to terminate the process,
        // if any exit code had been found, use the default value 1
        std::process::exit(status.code().unwrap_or(1));
    }
    anyhow::Ok(())
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
