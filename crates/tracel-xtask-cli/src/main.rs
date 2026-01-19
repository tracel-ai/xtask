use anyhow::{Context as _, anyhow, bail};
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
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    // Snap to git root to make it work from anywhere in the repo
    let git_root = git_toplevel().context("xtask should run inside a git repository")?;

    // Help mode
    if is_help_invocation(&args) {
        return show_all_help(&git_root);
    }

    // Discover workspaces and dispatch commands
    // If the first argument matches a child workspace directory name, use it (this is a monorepo)
    // else if a root workspace exists then run there (not a monorepo)
    // in any other care we error out
    let discovery = discover_workspaces(&git_root)?;
    let target = match first_arg_basename(&args) {
        Some(name) if discovery.children.iter().any(|ws| ws.dir_name == name) => {
            // monorepo
            args.remove(0);
            let ws = discovery
                .children
                .into_iter()
                .find(|ws| ws.dir_name == name)
                .expect("already checked workspace exists");
            ws.path
        }
        _ => {
            if let Some(root) = discovery.root {
                // standard repository
                root
            } else {
                bail!(
                    "No xtask workspace found at git root, and the first argument does not match any monorepo workspace.\n\
                     Git root: {}\n\
                     Try: xtask -h",
                    git_root.display()
                );
            }
        }
    };
    // dispatch
    exec_cargo_xtask(&target, &args)
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

fn git_toplevel() -> anyhow::Result<PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to execute git")?;
    if !out.status.success() {
        return Err(anyhow!(
            "git rev-parse --show-toplevel failed (are you inside a git repository?)"
        ));
    }
    let s = String::from_utf8(out.stdout).context("git output should be valid UTF-8")?;
    let p = s.trim();
    if p.is_empty() {
        bail!("git toplevel path is empty");
    }
    Ok(PathBuf::from(p))
}

fn discover_workspaces(git_root: &Path) -> anyhow::Result<Discovery> {
    let mut root = None;
    if is_workspace_root(git_root)? {
        root = Some(git_root.to_path_buf());
    }

    let mut children = Vec::new();
    for entry in fs::read_dir(git_root).with_context(|| {
        format!(
            "failed to read git root directory listing: {}",
            git_root.display()
        )
    })? {
        let entry = entry.context("failed to read directory entry")?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Only immediate children.
        let dir_name = entry.file_name().to_string_lossy().to_string();

        if is_workspace_root(&path)? {
            children.push(Workspace { path, dir_name });
        }
    }

    children.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));

    Ok(Discovery { root, children })
}

fn is_workspace_root(dir: &Path) -> anyhow::Result<bool> {
    let cargo_toml = dir.join("Cargo.toml");
    let xtask_dir = dir.join("xtask");

    Ok(cargo_toml.is_file() && xtask_dir.is_dir())
}

fn show_all_help(git_root: &Path) -> anyhow::Result<ExitCode> {
    let discovery = discover_workspaces(git_root)?;

    if discovery.root.is_none() && discovery.children.is_empty() {
        bail!(
            "No xtask workspaces found under git root: {}",
            git_root.display()
        );
    }

    // Print a simple top-level help that includes each workspace `cargo xtask -h`.
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

fn run_help_one(dir: &Path) -> anyhow::Result<()> {
    let status = Command::new("cargo")
        .arg("xtask")
        .arg("--help")
        .current_dir(dir)
        .status()
        .with_context(|| format!("failed to execute cargo xtask --help in {}", dir.display()))?;

    if !status.success() {
        eprintln!(
            "warning: cargo xtask --help failed in {} (exit code {:?})",
            dir.display(),
            status.code()
        );
    }

    Ok(())
}

fn exec_cargo_xtask(dir: &Path, args: &[OsString]) -> anyhow::Result<ExitCode> {
    let mut cmd = Command::new("cargo");
    cmd.arg("xtask");
    cmd.args(args);
    cmd.current_dir(dir);

    let status = cmd
        .status()
        .with_context(|| format!("failed to execute cargo xtask in {}", dir.display()))?;

    Ok(exit_code_from_status(status))
}

fn exit_code_from_status(status: std::process::ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) if (0..=255).contains(&code) => ExitCode::from(code as u8),
        Some(_) => ExitCode::from(1),
        None => ExitCode::from(1), // terminated by signal on Unix; keep it simple
    }
}
