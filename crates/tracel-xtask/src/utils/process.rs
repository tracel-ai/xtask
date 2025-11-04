use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::Path,
    process::{Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
};

use anyhow::{self, Context};
use rand::Rng;
use regex::Regex;

use crate::group_info;
use crate::{endgroup, group};

/// A custom error for failed subprocesses.
///
/// To get the `ExitStatus`, downcast the error at call sites.
#[derive(Debug)]
pub struct ProcessExitError {
    pub message: String,
    pub status: ExitStatus,
    pub signal: Option<ExitSignal>,
}

#[derive(Debug)]
pub struct ExitSignal {
    pub code: u32,
    pub name: String,
    pub description: String,
}

impl std::fmt::Display for ProcessExitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.status)
    }
}

impl std::fmt::Display for ExitSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "signal: {}, {}: {}",
            self.code, self.name, self.description
        )
    }
}

impl std::error::Error for ProcessExitError {}

fn return_process_error(
    error_msg: &str,
    status: ExitStatus,
    signal: Option<ExitSignal>,
) -> anyhow::Result<()> {
    Err(ProcessExitError {
        message: error_msg.to_string(),
        status,
        signal,
    }
    .into())
}

fn extract_exit_signal(line: &str) -> Option<ExitSignal> {
    // Matches: (signal: 11, SIGSEGV: invalid memory reference)
    let re = Regex::new(r"\(signal:\s*(\d+),\s*(SIG[A-Z]+):\s*([^)]+)\)").ok()?;
    let caps = re.captures(line)?;
    let code = caps.get(1)?.as_str().parse::<u32>().ok()?;
    let name = caps.get(2)?.as_str().to_string();
    let description = caps.get(3)?.as_str().trim().to_string();

    Some(ExitSignal {
        code,
        name,
        description,
    })
}

/// Run a process
pub fn run_process(
    name: &str,
    args: &[&str],
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
        anyhow::anyhow!(
            "Failed to execute {} {}: {}",
            name,
            args.first().unwrap(),
            e
        )
    })?;
    if !status.success() {
        return return_process_error(error_msg, status, None);
    }
    anyhow::Ok(())
}

pub fn run_process_capture_stdout(cmd: &mut Command, label: &str) -> anyhow::Result<String> {
    let out = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .with_context(|| format!("running {label}"))?;
    if !out.status.success() {
        return Err(anyhow::anyhow!("{label} failed with status {}", out.status));
    }
    Ok(String::from_utf8(out.stdout).context("non-UTF8 output")?)
}

/// Run a process for workspace
/// regexp must have one capture group if defined
#[allow(clippy::too_many_arguments)]
pub fn run_process_for_workspace<'a>(
    name: &str,
    args: &[&'a str],
    excluded: &'a [String],
    group_regexp: Option<&str>,
    group_name: Option<&str>,
    error_msg: &str,
    ignore_log: Option<&str>,
    ignore_msg: Option<&str>,
) -> anyhow::Result<()> {
    let group_rx: Option<Regex> = group_regexp.map(|r| Regex::new(r).unwrap());
    // split the args between cargo args and binary args so that we can extend the cargo args
    // and then append the binary args back.
    let (cargo_args, binary_args) = split_vector(args, "--");
    let mut cmd_args = cargo_args.to_owned();
    excluded
        .iter()
        .for_each(|ex| cmd_args.extend(["--exclude", ex]));
    cmd_args.extend(binary_args);
    group_info!("Command line: cargo {}", cmd_args.join(" "));
    // process
    let mut child = Command::new(name)
        .args(&cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to start {} {}: {}",
                name,
                cmd_args.first().unwrap(),
                e
            ))
        })?;

    // handle stdout and stderr in dedicated threads using a MPSC channel for synchronization
    let (tx, rx) = mpsc::channel();
    // stdout processing thread
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                tx.send((line, false)).unwrap();
            }
        });
    }
    // stderr processing thread
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                tx.send((line, true)).unwrap();
            }
        });
    }
    // Drop the sender once all the logs have been processed to close the channel
    drop(tx);

    // Process the stdout to inject log groups
    let mut ignore_error = false;
    let mut close_group = false;
    let mut signal = None;
    for (line, _is_stderr) in rx.iter() {
        let mut skip_line = false;

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
                    warn!("{msg}");
                }
                ignore_error = true;
                skip_line = true;
            }
        }

        if line.contains("(signal:") {
            signal = extract_exit_signal(&line);
        }

        if !skip_line {
            println!("{line}");
        }
    }

    let status = child
        .wait()
        .expect("Should be able to wait for the process to finish.");

    if status.success() || ignore_error {
        if close_group {
            endgroup!();
        }
        anyhow::Ok(())
    } else {
        return_process_error(error_msg, status, signal)
    }
}

/// Run a process command for a package
#[allow(clippy::too_many_arguments)]
pub fn run_process_for_package(
    name: &str,
    package: &String,
    args: &[&str],
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

    let mut child = Command::new(name)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to start {} {}: {}",
                name,
                args.first().unwrap(),
                e
            ))
        })?;

    // handle stdout and stderr in dedicated threads using a MPSC channel for synchronization
    let (tx, rx) = mpsc::channel();
    // stdout processing thread
    if let Some(stdout) = child.stdout.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                tx.send((line, false)).unwrap();
            }
        });
    }
    // stderr processing thread
    if let Some(stderr) = child.stderr.take() {
        let tx = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                tx.send((line, true)).unwrap();
            }
        });
    }
    // Drop the sender once all the logs have been processed to close the channel
    drop(tx);

    // Process the stdout to inject log groups
    let mut ignore_error = false;
    let mut skip_line = false;
    let mut signal = None;
    for (line, is_stderr) in rx.iter() {
        if let Some(log) = ignore_log {
            if !is_stderr {
                // skip the lines until a non stderr line is encountered
                skip_line = false;
            }
            if line.contains(log) {
                if let Some(msg) = ignore_msg {
                    warn!("{msg}");
                    ignore_error = true;
                    skip_line = true;
                }
            }
        }
        if line.contains("(signal:") {
            signal = extract_exit_signal(&line);
        }

        if !skip_line {
            println!("{line}");
        }
    }

    let status = child
        .wait()
        .expect("Should be able to wait for the process to finish.");

    if status.success() || ignore_error {
        anyhow::Ok(())
    } else {
        return_process_error(error_msg, status, signal)
    }
}

/// Return a random port between 3000 and 9999
pub fn random_port() -> u16 {
    let mut rng = rand::rng();
    rng.random_range(3000..=9999)
}

fn remove_ansi_codes(s: &str) -> String {
    let re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
    re.replace_all(s, "").to_string()
}

fn standardize_slashes(s: &str) -> String {
    s.replace('\\', "/")
}

/// Split given VEC into a left and right vectors where SPLIT belongs to the right vector.
/// If SPLIT does not exist in VEC then left is a VEC slice and right is empty.
fn split_vector<T: PartialEq>(vec: &[T], split: T) -> (&[T], &[T]) {
    let mut left = vec;
    let mut right = &vec[vec.len()..];
    if let Some(pos) = vec.iter().position(|e| *e == split) {
        left = &vec[..pos];
        right = &vec[pos..];
    }
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn test_random_port_in_range() {
        for _ in 0..10000 {
            let port = random_port();
            assert!(
                (3000..=9999).contains(&port),
                "Port should be between 3000 and 9999, got {port}"
            );
        }
    }

    #[rstest]
    #[case::simple_escape_code("\x1b[31mRed Text\x1b[0m", "Red Text")]
    #[case::complex_escape_code("\x1b[1;34mBold Blue Text\x1b[0m", "Bold Blue Text")]
    #[case::no_escape_code("No ANSI Codes", "No ANSI Codes")]
    fn test_remove_ansi_codes(#[case] input: &str, #[case] expected: &str) {
        let result = remove_ansi_codes(input);
        assert_eq!(
            result, expected,
            "Expected '{expected}', but got '{result}'"
        );
    }

    #[rstest]
    #[case::windows_path(r"C:\path\to\file", "C:/path/to/file")]
    #[case::network_path(r"\\network\share\file", "//network/share/file")]
    #[case::already_standard_path("/already/standard/path", "/already/standard/path")]
    fn test_standardize_slashes(#[case] input: &str, #[case] expected: &str) {
        let result = standardize_slashes(input);
        assert_eq!(
            result, expected,
            "Expected '{expected}', but got '{result}'"
        );
    }

    #[rstest]
    #[case::element_found(vec!["a", "b", "c", "d", "e", "f"], "d", vec!["a", "b", "c"], vec!["d", "e", "f"])]
    #[case::element_not_found(vec!["a", "b", "c", "d", "e", "f"], "z", vec!["a", "b", "c", "d", "e", "f"], vec![])]
    #[case::element_at_start(vec!["a", "b", "c", "d", "e", "f"], "a", vec![], vec!["a", "b", "c", "d", "e", "f"])]
    #[case::element_at_end(vec!["a", "b", "c", "d", "e", "f"], "f", vec!["a", "b", "c", "d", "e"], vec!["f"])]
    #[case::empty_vector(vec![], "x", vec![], vec![])]
    #[case::cargo_with_binary_args(vec!["cargo", "build", "--exclude", "crate", "--workpspace", "--", "--color", "always"], "--", vec!["cargo", "build", "--exclude", "crate", "--workpspace"], vec!["--", "--color", "always"])]
    #[case::cargo_without_binary_args(vec!["cargo", "build", "--exclude", "crate", "--workpspace"], "--", vec!["cargo", "build", "--exclude", "crate", "--workpspace"], vec![])]
    fn test_split_vector(
        #[case] vec: Vec<&str>,
        #[case] split_elem: &str,
        #[case] expected_left: Vec<&str>,
        #[case] expected_right: Vec<&str>,
    ) {
        let (left, right) = split_vector(&vec, split_elem);

        assert_eq!(left, &expected_left);
        assert_eq!(right, &expected_right);
    }
}
