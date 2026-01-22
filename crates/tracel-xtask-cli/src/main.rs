mod emojis;

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

#[derive(Debug)]
struct Workspace {
    path: PathBuf,
    dir_name: String,
    xtask_crate: String,
    xtask_bin: String,
}

#[derive(Debug)]
struct Discovery {
    root: Option<Workspace>,
    children: Vec<Workspace>,
}

const MAGIC_ARG_ALL: &'static str = ":all";

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    // Snap to git root to make it work from anywhere in the repo
    let git_root =
        git_toplevel().map_err(|e| format!("xtask should run inside a git repository: {e}"))?;
    // Help mode
    if is_help_invocation(&args) {
        return show_all_help(&git_root);
    }

    // Discover workspaces and dispatch commands
    let discovery = discover_workspaces(&git_root)?;
    if let Some(name) = first_arg_basename(&args)
        && name == MAGIC_ARG_ALL
    {
        // :all mode
        args.remove(0);

        if discovery.children.is_empty() {
            Err(format!(
                "xtask all requires a monorepo with at least one subrepo workspace under git root.\n\
                 Git root: {}",
                git_root.display()
            ))
        } else {
            exec_cargo_xtask_all(&args, &discovery.children)
        }
    } else {
        // single target
        let target: Workspace = match first_arg_basename(&args) {
            Some(name) if discovery.children.iter().any(|ws| ws.dir_name == name) => {
                // subrepo in monorepo
                args.remove(0);
                discovery
                    .children
                    .into_iter()
                    .find(|ws| ws.dir_name == name)
                    .expect("workspace existence already checked")
            }
            _ => {
                // standard repo with only one workspace at root
                if let Some(root) = discovery.root {
                    root
                } else {
                    return Err(format!(
                        "No xtask workspace found at git root, and the first argument does not match any subrepo workspace.\n\
                         Git root: {}\n\
                         Try: xtask -h",
                        git_root.display()
                    ));
                }
            }
        };

        exec_cargo_xtask(&target, &args)
    }
}

fn is_help_invocation(args: &[OsString]) -> bool {
    args.is_empty()
        || (args.len() == 1 && (args[0] == OsStr::new("-h") || args[0] == OsStr::new("--help")))
}

fn first_arg_basename(args: &[OsString]) -> Option<String> {
    let s = args.first()?.to_string_lossy();

    if s.starts_with('-')
        || s.contains(std::path::MAIN_SEPARATOR)
        || s.contains('/')
        || s.contains('\\')
    {
        return None;
    }

    Some(s.to_string())
}

fn git_toplevel() -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("failed to execute git: {e}"))?;
    if !out.status.success() {
        return Err(
            "git rev-parse --show-toplevel failed (are you inside a git repository?)".into(),
        );
    }

    let s = String::from_utf8(out.stdout)
        .map_err(|_| "git output should be valid UTF-8".to_string())?;
    let p = s.trim();
    if p.is_empty() {
        return Err("git toplevel path is empty".into());
    }

    Ok(PathBuf::from(p))
}

fn discover_workspaces(git_root: &Path) -> Result<Discovery, String> {
    let root = match is_workspace(git_root)? {
        Some(xtask_crate) => Some(Workspace {
            path: git_root.to_path_buf(),
            dir_name: "root".to_string(),
            xtask_bin: xtask_crate.clone(),
            xtask_crate,
        }),
        None => None,
    };
    let mut children = Vec::new();
    let entries = fs::read_dir(git_root).map_err(|e| {
        format!(
            "failed to read git root directory listing {}: {e}",
            git_root.display()
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();

        if let Some(xtask_crate) = is_workspace(&path)? {
            children.push(Workspace {
                path,
                dir_name: dir_name.clone(),
                xtask_crate,
                xtask_bin: format!("xtask-{dir_name}"),
            });
        }
    }
    children.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));

    Ok(Discovery { root, children })
}

fn is_workspace(dir: &Path) -> Result<Option<String>, String> {
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("failed to read directory listing {}: {e}", dir.display()))?;
    let mut matches: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_ascii_lowercase().contains("xtask") {
            matches.push(name);
        }
    }
    if matches.is_empty() {
        return Ok(None);
    }
    matches.sort();

    if let Some(exact) = matches
        .iter()
        .find(|n| n.eq_ignore_ascii_case("xtask"))
        .cloned()
    {
        return Ok(Some(exact));
    }

    Ok(Some(matches[0].clone()))
}

fn show_all_help(git_root: &Path) -> Result<ExitCode, String> {
    let discovery = discover_workspaces(git_root)?;
    if discovery.root.is_none() && discovery.children.is_empty() {
        return Err(format!(
            "No xtask workspaces found under git root: {}",
            git_root.display()
        ));
    }

    let mut first_failure: Option<ExitCode> = None;
    if let Some(root) = &discovery.root {
        let code = run_help_one(root)?;
        if code != ExitCode::SUCCESS && first_failure.is_none() {
            first_failure = Some(code);
        }
        eprintln!();
    }

    for ws in &discovery.children {
        let code = run_help_one(ws)?;
        if code != ExitCode::SUCCESS && first_failure.is_none() {
            first_failure = Some(code);
        }
        eprintln!();
    }

    Ok(first_failure.unwrap_or(ExitCode::SUCCESS))
}

fn run_help_one(ws: &Workspace) -> Result<ExitCode, String> {
    let is_subrepo = ws.dir_name != "root";
    let target_dir: &Path = if is_subrepo {
        Path::new("../target/xtask")
    } else {
        Path::new("target/xtask")
    };

    if is_subrepo {
        emojis::print_run_header(&emojis::format_repo_label(&ws.dir_name));
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--package")
        .arg(&ws.xtask_crate)
        .arg("--bin")
        .arg(&ws.xtask_bin)
        .arg("--")
        .arg("--help")
        .current_dir(&ws.path);
    if is_subrepo {
        cmd.env("XTASK_MONOREPO", "1");
    }
    let status = cmd.status().map_err(|e| {
        format!(
            "failed to execute cargo run ({} --help): {e}",
            ws.path.display()
        )
    })?;

    Ok(exit_code_from_status(status))
}

fn exec_cargo_xtask_all(args: &[OsString], subrepos: &[Workspace]) -> Result<ExitCode, String> {
    let mut first_failure: Option<ExitCode> = None;
    for ws in subrepos {
        let code = exec_cargo_xtask(ws, args)?;
        if code != ExitCode::SUCCESS && first_failure.is_none() {
            first_failure = Some(code);
        }
    }

    Ok(first_failure.unwrap_or(ExitCode::SUCCESS))
}

fn exec_cargo_xtask(ws: &Workspace, args: &[OsString]) -> Result<ExitCode, String> {
    let is_subrepo = ws.dir_name != "root";
    let target_dir: &Path = if is_subrepo {
        Path::new("../target/xtask")
    } else {
        Path::new("target/xtask")
    };

    if is_subrepo {
        emojis::print_run_header(&emojis::format_repo_label(&ws.dir_name));
    };

    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--package")
        .arg(&ws.xtask_crate)
        .arg("--bin")
        .arg(&ws.xtask_bin)
        .arg("--")
        .args(args)
        .current_dir(&ws.path);
    if is_subrepo {
        cmd.env("XTASK_MONOREPO", "1");
    }
    let status = cmd
        .status()
        .map_err(|e| format!("failed to execute cargo run ({}): {e}", ws.path.display()))?;

    Ok(exit_code_from_status(status))
}

fn exit_code_from_status(status: std::process::ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        _ => ExitCode::from(1),
    }
}
