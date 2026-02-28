mod args;
mod deps;
mod emojis;

use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use toml_edit::DocumentMut;

#[derive(Debug, Clone)]
enum XtaskInvocation {
    /// The xtask crate is a real workspace member, so we can invoke it via:
    /// `cargo run --package <package> --bin <bin> -- ...`
    WorkspaceMember { package: String },
    /// The xtask crate is not a workspace member (commonly because it's under `[workspace].exclude`),
    /// so we must invoke it via:
    /// `cargo run --manifest-path <path/to/Cargo.toml> --bin <bin> -- ...`
    ManifestPath {
        manifest_path: PathBuf,
        package: String,
    },
}

impl XtaskInvocation {
    fn package_name(&self) -> &str {
        match self {
            XtaskInvocation::WorkspaceMember { package } => package,
            XtaskInvocation::ManifestPath { package, .. } => package,
        }
    }
}

#[derive(Debug)]
struct Workspace {
    path: PathBuf,
    dir_name: String,
    xtask_bin: String,
    xtask: XtaskInvocation,
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

    if is_cli_help_invocation(&args) {
        match show_xtask_cli_help(&git_root) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        }
    } else if is_transparent_help_invocation(&args) {
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
            let xtask = is_workspace(&subrepo_root)?.ok_or_else(|| {
                format!(
                    "Subrepo '{}' is not a valid xtask workspace (expected Cargo workspace with an xtask* crate).\n\
                     Path: {}",
                    sel,
                    subrepo_root.display()
                )
            })?;

            let ws = Workspace {
                path: subrepo_root,
                dir_name: sel.clone(),
                xtask_bin: format!("xtask-{sel}"),
                xtask,
            };
            return exec_cargo_xtask(git_root, &ws, args);
        }
    }

    // No selector provided
    // Behavior depends on standard repo vs monorepo
    let root_xtask = is_workspace(git_root)?;
    if let Some(xtask) = root_xtask {
        // Standard repo -> execute at git root
        let xtask_bin = xtask.package_name().to_string();
        let ws = Workspace {
            path: git_root.to_path_buf(),
            dir_name: "root".to_string(),
            xtask_bin,
            xtask,
        };
        exec_cargo_xtask(git_root, &ws, args)
    } else {
        // Monorepo:
        if let Some(ws) = find_subrepo_workspace_root(&cwd, git_root)? {
            // inside a subrepo workspace at any depth then we execute in that subrepo.
            exec_cargo_xtask(git_root, &ws, args)
        } else {
            // At monorepo root we dispatch to all subrepos after confirmation
            let subrepos = list_subrepo_workspaces(git_root)?;
            if subrepos.is_empty() {
                return Err(format!(
                    "No xtask workspaces found under git root: {}",
                    git_root.display()
                ));
            }
            if !confirm_dispatch_all()? {
                return Ok(ExitCode::SUCCESS);
            }
            exec_cargo_xtask_all(git_root, args, &subrepos)
        }
    }
}

/// Sync dependency versions from the root fake Dependencies.toml
fn sync_monorepo_dependencies(git_root: &Path, subrepos: &[Workspace]) -> Result<(), String> {
    let deps_toml = git_root.join("Dependencies.toml");
    if !deps_toml.exists() {
        return Ok(());
    }
    eprintln!(
        "🔗 Syncing dependencies from {}...",
        deps_toml.file_name().unwrap().to_string_lossy()
    );
    let subrepo_roots: Vec<PathBuf> = subrepos.iter().map(|ws| ws.path.clone()).collect();
    let report = deps::sync_subrepos(&deps_toml, &subrepo_roots)
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

fn confirm_dispatch_all() -> Result<bool, String> {
    eprintln!(
        "⚠️ This will run the command in all subrepos (to suppress this prompt use the ':all' selector)"
    );
    eprint!("Continue? [y/N] ");

    std::io::stderr().flush().ok();
    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .map_err(|e| format!("failed to read confirmation from stdin: {e}"))?;
    let answer = buf.trim().to_ascii_lowercase();
    Ok(answer == "y" || answer == "yes")
}

fn is_cli_help_invocation(args: &[OsString]) -> bool {
    args.is_empty()
}

fn is_transparent_help_invocation(args: &[OsString]) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceEntryOrigin {
    Members,
    Exclude,
}

/// Resolve workspace entries (strings), supporting the common "path/*" glob.
fn collect_workspace_dirs(
    workspace_root: &Path,
    items: &toml_edit::Item,
) -> Result<Vec<PathBuf>, String> {
    let arr = items
        .as_array()
        .ok_or_else(|| "workspace members/exclude should be an array".to_string())?;

    let mut out: Vec<PathBuf> = Vec::new();

    for it in arr.iter() {
        let s = it
            .as_str()
            .ok_or_else(|| "workspace entry should be a string".to_string())?;

        if let Some((prefix, suffix)) = s.split_once('*') {
            // Only handle the common "path/*" form
            if suffix.is_empty() {
                let base = workspace_root.join(prefix);
                if base.is_dir() {
                    let entries = fs::read_dir(&base).map_err(|e| {
                        format!("failed to read directory listing {}: {e}", base.display())
                    })?;
                    for entry in entries {
                        let entry =
                            entry.map_err(|e| format!("failed to read directory entry: {e}"))?;
                        let p = entry.path();
                        if p.is_dir() {
                            out.push(p);
                        }
                    }
                }
            }
            continue;
        }

        out.push(workspace_root.join(s));
    }

    Ok(out)
}

/// Returns how to invoke an xtask-like crate if `dir` is a Cargo workspace root that contains one.
///
/// Detection:
/// - Reads `Cargo.toml` and requires `[workspace].members` to exist (same behavior as before).
/// - Scans candidate directories from:
///   - `[workspace].members`
///   - `[workspace].exclude` (important for repos that do `members = ["crates/*"]` and `exclude = ["xtask"]`)
/// - For each candidate directory, if it has a `Cargo.toml` with `package.name` starting with `"xtask"`
///   (case-insensitive), it is considered a match.
///
/// Selection (deterministic):
/// - Prefer an exact `package.name == "xtask"` (case-insensitive).
/// - Otherwise choose the lexicographically smallest xtask-like package name.
///
/// Invocation mode:
/// - If the selected xtask-like crate came from `workspace.members`, returns:
///   `Some(XtaskInvocation::WorkspaceMember { package })`
///   (we can safely run via `cargo run --package <package> ...`)
/// - If it came only from `workspace.exclude`, returns:
///   `Some(XtaskInvocation::ManifestPath { manifest_path: <crate_dir>/Cargo.toml, package })`
fn is_workspace(dir: &Path) -> Result<Option<XtaskInvocation>, String> {
    let workspace_toml = dir.join("Cargo.toml");
    if !workspace_toml.is_file() {
        return Ok(None);
    }

    let root_src = fs::read_to_string(&workspace_toml)
        .map_err(|e| format!("failed to read {}: {e}", workspace_toml.display()))?;
    let root_doc = root_src
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse {}: {e}", workspace_toml.display()))?;

    let Some(ws) = root_doc.get("workspace") else {
        return Ok(None);
    };

    // Keep current behavior: not a workspace if workspace.members is missing.
    let Some(members_item) = ws.get("members") else {
        return Ok(None);
    };

    // Track candidate dirs with origin; dedupe by path.
    // If a path appears in both, prefer Members.
    let mut candidates: BTreeMap<PathBuf, WorkspaceEntryOrigin> = BTreeMap::new();

    for p in collect_workspace_dirs(dir, members_item)? {
        candidates.insert(p, WorkspaceEntryOrigin::Members);
    }

    if let Some(exclude_item) = ws.get("exclude") {
        for p in collect_workspace_dirs(dir, exclude_item)? {
            candidates.entry(p).or_insert(WorkspaceEntryOrigin::Exclude);
        }
    }

    // Scan for xtask-like crates; store (package_name, origin, manifest_path)
    let mut matches: Vec<(String, WorkspaceEntryOrigin, PathBuf)> = Vec::new();

    for (candidate_dir, origin) in candidates {
        let candidate_manifest = candidate_dir.join("Cargo.toml");
        if !candidate_manifest.is_file() {
            continue;
        }

        let src = fs::read_to_string(&candidate_manifest)
            .map_err(|e| format!("failed to read {}: {e}", candidate_manifest.display()))?;
        let doc = src
            .parse::<DocumentMut>()
            .map_err(|e| format!("failed to parse {}: {e}", candidate_manifest.display()))?;

        let package_name = doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str());

        if let Some(name) = package_name
            && name.to_ascii_lowercase().starts_with("xtask")
        {
            matches.push((name.to_string(), origin, candidate_manifest));
        }
    }

    if matches.is_empty() {
        return Ok(None);
    }

    matches.sort_by(|a, b| a.0.cmp(&b.0));

    // Prefer exact "xtask"
    let chosen = if let Some(idx) = matches
        .iter()
        .position(|(n, _, _)| n.eq_ignore_ascii_case("xtask"))
    {
        matches.remove(idx)
    } else {
        matches.remove(0)
    };

    let (package, origin, manifest_path) = chosen;

    Ok(Some(match origin {
        WorkspaceEntryOrigin::Members => XtaskInvocation::WorkspaceMember { package },
        WorkspaceEntryOrigin::Exclude => XtaskInvocation::ManifestPath {
            manifest_path,
            package,
        },
    }))
}

fn find_subrepo_workspace_root(start: &Path, git_root: &Path) -> Result<Option<Workspace>, String> {
    let mut cur = start.to_path_buf();

    loop {
        // The root of the repository cannot be a subrepo
        if cur == *git_root {
            return Ok(None);
        }

        if let Some(xtask) = is_workspace(&cur)? {
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

            // Keep your convention for subrepo bin names.
            // If you want package-driven bin names, swap this to `xtask.package_name().to_string()`.
            let xtask_bin = format!("xtask-{subrepo}");

            return Ok(Some(Workspace {
                path: cur,
                dir_name: subrepo,
                xtask_bin,
                xtask,
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

        if let Some(xtask) = is_workspace(&path)? {
            // Keep your convention: xtask-<subrepo>
            let xtask_bin = format!("xtask-{dir_name}");

            subrepos.push(Workspace {
                path,
                dir_name: dir_name.clone(),
                xtask_bin,
                xtask,
            });
        }
    }

    subrepos.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    Ok(subrepos)
}

fn show_all_help(git_root: &Path, args: &mut Vec<OsString>) -> Result<ExitCode, String> {
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
            let xtask = is_workspace(&subrepo_root)?.ok_or_else(|| {
                format!(
                    "Subrepo '{}' is not a valid xtask workspace (expected Cargo workspace with an xtask* crate).\n\
                     Path: {}",
                    sel,
                    subrepo_root.display()
                )
            })?;

            let ws = Workspace {
                path: subrepo_root,
                dir_name: sel.clone(),
                xtask_bin: format!("xtask-{sel}"),
                xtask,
            };
            run_help_one(&ws)
        }
    } else {
        // No selector, behavior depends on standard repo vs monorepo.
        let root_xtask = is_workspace(git_root)?;
        if let Some(xtask) = root_xtask {
            // Standard repo: help at git root
            let ws = Workspace {
                path: git_root.to_path_buf(),
                dir_name: "root".to_string(),
                xtask_bin: xtask.package_name().to_string(),
                xtask,
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

                if !confirm_dispatch_all()? {
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

    eprintln!("🔧 Compiling xtask:{}...", ws.dir_name);

    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--target-dir").arg(target_dir);

    match &ws.xtask {
        XtaskInvocation::WorkspaceMember { package } => {
            cmd.arg("--package").arg(package);
        }
        XtaskInvocation::ManifestPath { manifest_path, .. } => {
            cmd.arg("--manifest-path").arg(manifest_path);
        }
    }

    cmd.arg("--bin")
        .arg(&ws.xtask_bin)
        .arg("--")
        .arg("--help")
        .env("XTASK_CLI", "1")
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
    let is_subrepo = ws.dir_name != "root";

    let target_path = format!("target/{}", ws.xtask.package_name());
    let target_dir = Path::new(&target_path);

    if is_subrepo {
        emojis::print_run_header(&emojis::format_repo_label(&ws.dir_name));
    };

    sync_monorepo_dependencies(git_root, std::slice::from_ref(ws))?;

    eprintln!("🔧 Compiling xtask:{}...", ws.dir_name);

    let mut cmd = Command::new("cargo");
    cmd.arg("run").arg("--target-dir").arg(target_dir);

    match &ws.xtask {
        XtaskInvocation::WorkspaceMember { package } => {
            cmd.arg("--package").arg(package);
        }
        XtaskInvocation::ManifestPath { manifest_path, .. } => {
            cmd.arg("--manifest-path").arg(manifest_path);
        }
    }

    cmd.arg("--bin")
        .arg(&ws.xtask_bin)
        .arg("--")
        .args(args)
        .env("XTASK_CLI", "1")
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

/// Try to retrieve xtask CLI binary name, otherwise fallback to xtask
fn cli_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "xtask".to_string())
}

fn cli_help_header() {
    let name = cli_name();
    let version = env!("CARGO_PKG_VERSION");
    let authors = env!("CARGO_PKG_AUTHORS");
    let author = authors.split(',').next().unwrap_or(authors);
    eprintln!("{name} v{version} by {author}");
}

fn cli_help_fooder() {
    println!("LICENSE");
    println!("-------");
    println!("  This project is dual-licensed under the Apache 2.0 and MIT licenses.");
    println!("  You may choose either license when using, modifying, or distributing it.");
    println!();
    println!("  Repository: https://github.com/tracel-ai/xtask");
    println!("  See LICENSE-APACHE and LICENSE-MIT for full license texts.");
    println!();
}

fn show_xtask_cli_help(git_root: &Path) -> Result<ExitCode, String> {
    let cwd = env::current_dir().map_err(|e| format!("failed to read current directory: {e}"))?;
    let cli_name = cli_name();
    let root_xtask = is_workspace(git_root)?;
    let is_monorepo = root_xtask.is_none();

    cli_help_header();
    println!();
    println!("A transparent wrapper around `cargo xtask` alias for standard repos and monorepos.");
    println!("It discovers xtask workspaces and dispatches your command to the right place.");
    println!();

    println!("USAGE");
    println!("-----");
    println!("  {cli_name} [:<subrepo>|:all] [<xtask args...>]");
    println!();
    println!("BEHAVIOR");
    println!("--------");
    println!("  - With a selector:");
    println!("      :<subrepo>  Runs xtask in that subrepo workspace.");
    println!("      :all        Runs xtask in all subrepos.");
    println!("  - Without a selector:");
    println!("      Standard repo: runs xtask at the git root.");
    println!("      Monorepo: if you're inside a subrepo, runs in that subrepo context,");
    println!("                otherwise prompts then run the command in all the subrepos.");
    println!();
    println!("HELP");
    println!("----");
    println!("  - `{cli_name}`                   Shows this screen.");
    println!("  - `{cli_name} --help`            Shows underlying xtask help (transparent mode).");
    println!("  - `{cli_name} <command> --help`  Shows help of <command>.");
    println!();

    if !is_monorepo {
        let xtask_pkg = root_xtask
            .as_ref()
            .map(|x| x.package_name().to_string())
            .unwrap_or_else(|| "xtask".to_string());

        println!("CONTEXT");
        println!("-------");
        println!("  Current Repository mode: standard repository");
        println!("  Git root: {}", git_root.display());
        println!("  Xtask package: {xtask_pkg}");
        println!();

        println!("EXAMPLES");
        println!("--------");
        println!("  {cli_name} build");
        println!("      Run the `build` xtask command at the repository root.");
        println!("      Equivalent to `cargo xtask build`.");
        println!();
        println!("  {cli_name} test all");
        println!("      Run the `test` xtask command with argument `all`.");
        println!("      Arguments are forwarded transparently to xtask.");
        println!();
        println!("  {cli_name} fix -y all");
        println!("      Run the `fix` xtask command, auto-confirming prompts (`-y`),");
        println!("      and applying fixes to all supported targets.");
        println!();

        cli_help_fooder();
        return Ok(ExitCode::SUCCESS);
    }

    // Monorepo context
    let subrepos = list_subrepo_workspaces(git_root)?;
    let located = find_subrepo_workspace_root(&cwd, git_root)?;

    // Pick real example subrepos found in this context.
    let ex1 = subrepos
        .first()
        .map(|ws| ws.dir_name.as_str())
        .unwrap_or("backend");
    let ex2 = subrepos
        .get(1)
        .map(|ws| ws.dir_name.as_str())
        .unwrap_or("frontend");

    println!("CONTEXT");
    println!("-------");
    println!("  Git root: {}", git_root.display());
    println!("  Current Repository mode: monorepo");
    match located {
        Some(ws) => {
            println!("  Current location: inside subrepo `{}`", ws.dir_name);
            println!("  Current xtask package: {}", ws.xtask.package_name());
        }
        None => {
            if cwd == git_root {
                println!("  Current location: monorepo root");
            } else {
                println!("  Current location: outside a recognized subrepo workspace");
            }
        }
    }
    println!();

    println!("SUBREPOS");
    println!("--------");
    if subrepos.is_empty() {
        println!("  (none found)");
    } else {
        for ws in &subrepos {
            println!(
                "  - {:<16}  xtask package: {:<12}  path: {}",
                ws.dir_name,
                ws.xtask.package_name(),
                ws.path.display()
            );
        }
    }
    println!();

    println!("EXAMPLES");
    println!("--------");
    println!("  {cli_name} :{ex1} build");
    println!("      Run `build` in the `{ex1}` subrepo, regardless of current directory within");
    println!("      the monorepo.");
    println!();
    println!("  {cli_name} :{ex2} test all");
    println!("      Run both unit and integration tests scoped to the `{ex2}` subrepo only.");
    println!();
    println!("  {cli_name} :all fix -y all");
    println!("      Run all available fixes (lint, format, audit, ...) across all subrepos,");
    println!("      auto-confirming prompts and applying fixes everywhere.");
    println!();
    println!("  {cli_name} :all build");
    println!("      Run `build` xtask command in every subrepo, regardless of current");
    println!("      directory within the monorepo. Useful to easily sync the dependencies");
    println!("      of `Dependencies.toml` with all the subrepos and verify that they all");
    println!("      still build without errors.");
    println!();

    println!("NOTES");
    println!("-----");
    println!(
        "  - If `Dependencies.toml` exists at the monorepo root, xtask will sync dependency specs"
    );
    println!("    before running subrepo commands.");
    println!(
        "  - This wrapper is designed to remain transparent: it forwards your arguments to the"
    );
    println!("    underlying xtask binary in the selected workspace(s).");
    println!();

    cli_help_fooder();
    Ok(ExitCode::SUCCESS)
}
