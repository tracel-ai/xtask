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
}

#[derive(Debug)]
struct Discovery {
    root: Option<PathBuf>,
    children: Vec<Workspace>,
}

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
    let mut subrepo = None;
    let discovery = discover_workspaces(&git_root)?;
    let target = match first_arg_basename(&args) {
        Some(name) if discovery.children.iter().any(|ws| ws.dir_name == name) => {
            // monorepo
            subrepo = Some(name.clone());
            args.remove(0);
            discovery
                .children
                .into_iter()
                .find(|ws| ws.dir_name == name)
                .expect("workspace existence already checked")
                .path
        }
        _ => {
            if let Some(root) = discovery.root {
                // standard repository
                root
            } else {
                return Err(format!(
                    "No xtask workspace found at git root, and the first argument does not match any monorepo workspace.\n\
                     Git root: {}\n\
                     Try: xtask -h",
                    git_root.display()
                ));
            }
        }
    };

    exec_cargo_xtask(&target, &args, subrepo.as_deref())
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
    let mut root = None;
    if is_workspace_root(git_root)? {
        root = Some(git_root.to_path_buf());
    }
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

        if is_workspace_root(&path)? {
            children.push(Workspace { path, dir_name });
        }
    }
    children.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));

    Ok(Discovery { root, children })
}

fn is_workspace_root(dir: &Path) -> Result<bool, String> {
    let cargo_toml = dir.join("Cargo.toml");
    let xtask_dir = dir.join("xtask");

    Ok(cargo_toml.is_file() && xtask_dir.is_dir())
}

fn show_all_help(git_root: &Path) -> Result<ExitCode, String> {
    let discovery = discover_workspaces(git_root)?;
    if discovery.root.is_none() && discovery.children.is_empty() {
        return Err(format!(
            "No xtask workspaces found under git root: {}",
            git_root.display()
        ));
    }
    if let Some(root) = discovery.root {
        println!("== xtask @ {} ==", root.display());
        run_help_one(&root)?;
        println!();
    }

    for ws in discovery.children {
        println!("== xtask @ {}/{} ==", git_root.display(), ws.dir_name);
        run_help_one(&ws.path)?;
        println!();
    }

    Ok(ExitCode::SUCCESS)
}

fn run_help_one(dir: &Path) -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("xtask")
        .arg("--help")
        .current_dir(dir)
        .status()
        .map_err(|e| {
            format!(
                "failed to execute cargo xtask --help in {}: {e}",
                dir.display()
            )
        })?;
    if !status.success() {
        eprintln!(
            "warning: cargo xtask --help failed in {} (exit code {:?})",
            dir.display(),
            status.code()
        );
    }

    Ok(())
}

fn exec_cargo_xtask(
    dir: &Path,
    args: &[OsString],
    subrepo: Option<&str>,
) -> Result<ExitCode, String> {
    let (target_dir, bin_name) = match subrepo {
        Some(subrepo) => (OsStr::new("../target/xtask"), format!("xtask-{subrepo}")),
        None => (OsStr::new("target/xtask"), "xtask".to_string()),
    };
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--package")
        .arg("xtask")
        .arg("--bin")
        .arg(bin_name)
        .arg("--")
        .args(args)
        .current_dir(dir);
    if subrepo.is_some() {
        cmd.env("XTASK_MONOREPO", "1");
    }

    let status = cmd.status().map_err(|e| {
        format!(
            "failed to execute cargo run (xtask) in {}: {e}",
            dir.display()
        )
    })?;
    Ok(exit_code_from_status(status))
}

fn exit_code_from_status(status: std::process::ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        _ => ExitCode::from(1),
    }
}
