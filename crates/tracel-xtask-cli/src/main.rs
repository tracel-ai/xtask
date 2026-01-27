mod args;
mod deps;
mod emojis;

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use toml_edit::DocumentMut;

#[derive(Debug)]
struct Workspace {
    path: PathBuf,
    dir_name: String,
    xtask_crate: String,
    xtask_bin: String,
}

fn main() -> ExitCode {
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    let git_root = match git_repo_root()
        .map_err(|e| format!("xtask should run inside a git repository: {e}"))
    {
        Ok(root) => root,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };

    if is_help_invocation(&args) {
        match show_all_help(&git_root, &mut args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        }
    } else {
        match run(&git_root, &mut args) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        }
    }
}

fn run(git_root: &Path, args: &mut Vec<OsString>) -> Result<ExitCode, String> {
    let yes = args::take_yes_flag(args);
    let selector = args::take_subrepo_selector(args);
    let cwd = env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;

    // Selector provided
    if let Some(sel) = selector {
        if sel == "all" {
            // :all magic selector
            let subrepos = list_subrepo_workspaces(git_root)?;
            if subrepos.is_empty() {
                return Err(format!(
                    "xtask :all requires at least one subrepo workspace under git root.\n\
                     Git root: {}",
                    git_root.display()
                ));
            }
            return exec_cargo_xtask_all(git_root, args, &subrepos);
        } else {
            // :<subrepo> selector
            let subrepo_root = git_root.join(&sel);
            let xtask_crate = is_workspace(&subrepo_root)?.ok_or_else(|| {
                format!(
                    "Subrepo '{}' is not a valid xtask workspace (expected Cargo.toml and an xtask* directory).\n\
                     Path: {}",
                    sel,
                    subrepo_root.display()
                )
            })?;

            let ws = Workspace {
                path: subrepo_root,
                dir_name: sel.clone(),
                xtask_bin: format!("xtask-{sel}"),
                xtask_crate,
            };
            return exec_cargo_xtask(git_root, &ws, args);
        }
    }

    // No selector provided
    // Behavior depends on standard repo vs monorepo
    let root_xtask = is_workspace(git_root)?;
    if let Some(xtask_crate) = root_xtask {
        // Standard repo -> execute at git root
        let ws = Workspace {
            path: git_root.to_path_buf(),
            dir_name: "root".to_string(),
            xtask_bin: xtask_crate.clone(),
            xtask_crate,
        };
        exec_cargo_xtask(git_root, &ws, args)
    } else {
        // Monorepo:
        if let Some(ws) = find_subrepo_workspace_root(&cwd, git_root)? {
            // inside a subrepo workspace at any depth then we execute in that subrepo.
            exec_cargo_xtask(git_root, &ws, args)
        } else {
            //  At monorepo root we dispatch to all subrepos after confirmation
            let subrepos = list_subrepo_workspaces(git_root)?;
            if subrepos.is_empty() {
                return Err(format!(
                    "No xtask workspaces found under git root: {}",
                    git_root.display()
                ));
            }
            if !confirm_dispatch_all(yes)? {
                return Ok(ExitCode::SUCCESS);
            }
            exec_cargo_xtask_all(git_root, args, &subrepos)
        }
    }
}

/// Sync dependency versions from the root fake Cargo.toml
fn sync_monorepo_dependencies(git_root: &Path, subrepos: &[Workspace]) -> Result<(), String> {
    let root_manifest = git_root.join("Cargo.toml");

    let subrepo_roots: Vec<PathBuf> = subrepos.iter().map(|ws| ws.path.clone()).collect();

    let report = deps::sync_subrepos(&root_manifest, &subrepo_roots)
        .map_err(|e| format!("dependency sync should succeed: {e}"))?;

    for (manifest, table_path, dep) in report.missing_canonical_dependencies {
        eprintln!(
            "warning: {} declares dependency '{}' in [{}] but it is missing from root [workspace.dependencies]",
            manifest.display(),
            dep,
            table_path,
        );
    }

    Ok(())
}

fn confirm_dispatch_all(yes: bool) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }

    eprint!("This will run the command in all subrepos. Continue? [y/N] ");

    std::io::stderr().flush().ok();
    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .map_err(|e| format!("failed to read confirmation from stdin: {e}"))?;
    let answer = buf.trim().to_ascii_lowercase();
    Ok(answer == "y" || answer == "yes")
}

fn is_help_invocation(args: &[OsString]) -> bool {
    args.is_empty()
        || (args.len() == 1 && (args[0] == OsStr::new("-h") || args[0] == OsStr::new("--help")))
}

fn git_repo_root() -> Result<PathBuf, String> {
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

/// Returns the xtask *package name* if this directory is a Cargo workspace that contains
/// an xtask-like member.
fn is_workspace(dir: &Path) -> Result<Option<String>, String> {
    let workspace_toml = dir.join("Cargo.toml");
    if !workspace_toml.is_file() {
        return Ok(None);
    }
    let root_src = fs::read_to_string(&workspace_toml)
        .map_err(|e| format!("failed to read {}: {e}", workspace_toml.display()))?;
    let root_doc = root_src
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse {}: {e}", workspace_toml.display()))?;

    // Not a workspace if workspace.members is missing.
    let Some(members_item) = root_doc.get("workspace").and_then(|w| w.get("members")) else {
        return Ok(None);
    };

    let members = members_item
        .as_array()
        .ok_or_else(|| "workspace.members should be an array".to_string())?;
    // Resolve members (with minimal support for "dir/*" globs)
    let mut member_dirs: Vec<PathBuf> = Vec::new();
    for m in members.iter() {
        let s = m
            .as_str()
            .ok_or_else(|| "workspace member should be a string".to_string())?;

        if let Some((prefix, suffix)) = s.split_once('*') {
            // Only handle the common "path/*" form
            if suffix.is_empty() {
                let base = dir.join(prefix);
                if base.is_dir() {
                    let entries = fs::read_dir(&base).map_err(|e| {
                        format!("failed to read directory listing {}: {e}", base.display())
                    })?;
                    for entry in entries {
                        let entry =
                            entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
                        let p = entry.path();
                        if p.is_dir() {
                            member_dirs.push(p);
                        }
                    }
                }
            }
            continue;
        }

        member_dirs.push(dir.join(s));
    }

    // Find xtask-like member(s) by package.name
    let mut matches: Vec<String> = Vec::new();
    for member_dir in member_dirs {
        let member_toml = member_dir.join("Cargo.toml");
        if !member_toml.is_file() {
            continue;
        }

        let src = fs::read_to_string(&member_toml)
            .map_err(|e| format!("failed to read {}: {e}", member_toml.display()))?;
        let doc = src
            .parse::<DocumentMut>()
            .map_err(|e| format!("failed to parse {}: {e}", member_toml.display()))?;

        let package_name = doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str());

        if let Some(name) = package_name {
            if name.to_ascii_lowercase().starts_with("xtask") {
                matches.push(name.to_string());
            }
        }
    }
    if matches.is_empty() {
        return Ok(None);
    }
    matches.sort();
    // Prefer exact "xtask"
    if let Some(exact) = matches
        .iter()
        .find(|n| n.eq_ignore_ascii_case("xtask"))
        .cloned()
    {
        return Ok(Some(exact));
    }

    Ok(Some(matches[0].clone()))
}

fn find_subrepo_workspace_root(start: &Path, git_root: &Path) -> Result<Option<Workspace>, String> {
    let mut cur = start.to_path_buf();

    loop {
        // The root of the repository cannot be a subrepo
        if cur == *git_root {
            return Ok(None);
        }

        if let Some(xtask_crate) = is_workspace(&cur)? {
            // subrepo dir name is the first path segment under git_root
            let rel = cur.strip_prefix(git_root).map_err(|_| {
                format!(
                    "internal error: {} is not under git root {}",
                    cur.display(),
                    git_root.display()
                )
            })?;

            let subrepo = rel
                .components()
                .next()
                .ok_or_else(|| {
                    "internal error: workspace root has empty relative path".to_string()
                })?
                .as_os_str()
                .to_string_lossy()
                .to_string();

            return Ok(Some(Workspace {
                path: cur,
                dir_name: subrepo.clone(),
                xtask_bin: xtask_crate.clone(),
                xtask_crate,
            }));
        }

        if !cur.pop() {
            return Ok(None);
        }
    }
}

fn list_subrepo_workspaces(git_root: &Path) -> Result<Vec<Workspace>, String> {
    let entries = fs::read_dir(git_root).map_err(|e| {
        format!(
            "failed to read git root directory listing {}: {e}",
            git_root.display()
        )
    })?;
    let mut subrepos = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();

        if let Some(xtask_crate) = is_workspace(&path)? {
            subrepos.push(Workspace {
                path,
                dir_name: dir_name.clone(),
                xtask_bin: xtask_crate.clone(),
                xtask_crate,
            });
        }
    }

    subrepos.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    Ok(subrepos)
}

fn show_all_help(git_root: &Path, args: &mut Vec<OsString>) -> Result<ExitCode, String> {
    let yes = args::take_yes_flag(args);
    let selector = args::take_subrepo_selector(args);
    let cwd = env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;

    // Selector
    if let Some(sel) = selector {
        if sel == "all" {
            // :all magic selector
            let subrepos = list_subrepo_workspaces(git_root)?;
            if subrepos.is_empty() {
                return Err(format!(
                    "xtask :all requires at least one subrepo workspace under git root.\n\
                     Git root: {}",
                    git_root.display()
                ));
            }
            run_help_all(&subrepos)
        } else {
            // :<subrepo> selector
            let subrepo_root = git_root.join(&sel);
            let xtask_crate = is_workspace(&subrepo_root)?.ok_or_else(|| {
                format!(
                    "Subrepo '{}' is not a valid xtask workspace (expected Cargo.toml and an xtask* directory).\n\
                     Path: {}",
                    sel,
                    subrepo_root.display()
                )
            })?;

            let ws = Workspace {
                path: subrepo_root,
                dir_name: sel.clone(),
                xtask_bin: xtask_crate.clone(),
                xtask_crate,
            };
            run_help_one(&ws)
        }
    } else {
        // No selector, behovior depends on standard repo vs monorepo.
        let root_xtask = is_workspace(git_root)?;
        if let Some(xtask_crate) = root_xtask {
            // Standard repo: help at git root
            let ws = Workspace {
                path: git_root.to_path_buf(),
                dir_name: "root".to_string(),
                xtask_bin: xtask_crate.clone(),
                xtask_crate,
            };
            run_help_one(&ws)
        } else {
            // Monorepo:
            if let Some(ws) = find_subrepo_workspace_root(&cwd, git_root)? {
                // if inside a subrepo workspace (any depth), show help for that subrepo.
                run_help_one(&ws)
            } else {
                // At monorepo root we show help for all after confirmation
                let subrepos = list_subrepo_workspaces(git_root)?;
                if subrepos.is_empty() {
                    return Err(format!(
                        "No xtask workspaces found under git root: {}",
                        git_root.display()
                    ));
                }

                if !confirm_dispatch_all(yes)? {
                    return Ok(ExitCode::SUCCESS);
                }

                run_help_all(&subrepos)
            }
        }
    }
}

fn run_help_all(subrepos: &[Workspace]) -> Result<ExitCode, String> {
    let mut first_failure: Option<ExitCode> = None;

    for ws in subrepos {
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

fn exec_cargo_xtask_all(
    git_root: &Path,
    args: &[OsString],
    subrepos: &[Workspace],
) -> Result<ExitCode, String> {
    sync_monorepo_dependencies(git_root, &subrepos)?;
    let mut first_failure: Option<ExitCode> = None;
    for ws in subrepos {
        let code = exec_cargo_xtask(git_root, ws, args)?;
        if code != ExitCode::SUCCESS && first_failure.is_none() {
            first_failure = Some(code);
        }
    }

    Ok(first_failure.unwrap_or(ExitCode::SUCCESS))
}

fn exec_cargo_xtask(
    git_root: &Path,
    ws: &Workspace,
    args: &[OsString],
) -> Result<ExitCode, String> {
    sync_monorepo_dependencies(git_root, std::slice::from_ref(&ws))?;
    let is_subrepo = ws.dir_name != "root";
    let target_path = format!("target/{}", ws.xtask_crate);
    let target_dir = Path::new(&target_path);
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
