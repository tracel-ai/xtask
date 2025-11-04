use std::path::PathBuf;

use anyhow::Context;

use crate::prelude::{Context as XtaskContext, Environment};
use crate::utils::aws_cli::{
    aws_account_id, aws_cli_capture_stdout, ecr_docker_login, ecr_ensure_repo_exists,
    ecr_get_manifest, ecr_put_manifest,
};
use crate::utils::git::repo_root_or_cwd;
use crate::utils::process::run_process;

const AWS_REGION: &str = "us-east-1";

#[tracel_xtask_macros::declare_command_args(None, DockerSubCommand)]
pub struct DockerCmdArgs {}

impl Default for DockerSubCommand {
    fn default() -> Self {
        DockerSubCommand::Build(BuildSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct BuildSubCmdArgs {
    /// Path to Dockerfile relative to context directory
    pub dockerfile: PathBuf,
    /// Docker build context directory (default to repository root)
    #[arg(long)]
    pub context_dir: Option<PathBuf>,
    /// Local image name
    #[arg(long)]
    pub image: String,
    /// Local tag (defaults to "latest" if omitted)
    #[arg(long)]
    pub tag: Option<String>,
    /// Build arguments
    #[arg(long)]
    pub build_args: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct PushSubCmdArgs {
    /// Local image name (the one used in the build command)
    #[arg(long)]
    pub image: String,

    /// Local image tag (the one used when building)
    #[arg(long)]
    pub local_tag: String,

    /// ECR repository name to push into
    #[arg(long)]
    pub repository: String,

    /// Explicit remote tag (if provided, it overrides auto computation)
    #[arg(long)]
    pub remote_tag: Option<String>,

    /// When true, compute the next monotonic tag from ECR instead of reusing the local tag
    #[arg(long)]
    pub auto_remote_tag: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct PromoteSubCmdArgs {
    /// ECR repository name
    #[arg(long)]
    pub repository: String,
    /// Build tag to promote to 'latest'
    #[arg(long)]
    pub tag: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct RollbackSubCmdArgs {
    /// ECR repository name
    #[arg(long)]
    pub repository: String,
}

pub fn handle_command(
    args: DockerCmdArgs,
    _env: Environment,
    _ctx: XtaskContext,
) -> anyhow::Result<()> {
    match args.get_command() {
        DockerSubCommand::Build(build_args) => build(build_args),
        DockerSubCommand::Push(push_args) => push(push_args),
        DockerSubCommand::Promote(promote_args) => promote(promote_args),
        DockerSubCommand::Rollback(rollback_args) => rollback(rollback_args),
    }
}

fn build(build_args: BuildSubCmdArgs) -> anyhow::Result<()> {
    let context_dir = build_args.context_dir.unwrap_or(repo_root_or_cwd()?);
    let dockerfile_path = if build_args.dockerfile.is_absolute() {
        build_args.dockerfile.clone()
    } else {
        context_dir.join(&build_args.dockerfile)
    };

    let tag = build_args.tag.as_deref().unwrap_or("latest");
    let mut args: Vec<String> = vec![
        "build".into(),
        format!("--file={}", dockerfile_path.to_string_lossy()),
        format!("--tag={}:{}", build_args.image, tag),
        // context_dir is positional
        context_dir.to_string_lossy().into(),
    ];
    for kv in build_args.build_args {
        // before context dir
        args.insert(args.len() - 1, format!("--build-arg={kv}"));
    }

    docker_cli(args, None, None, "docker build failed")
}

fn push(push_args: PushSubCmdArgs) -> anyhow::Result<()> {
    ecr_ensure_repo_exists(&push_args.repository, &AWS_REGION)?;

    // Determine remote tag:
    // 1) if --remote-tag is provided then use it
    // 2) else if --auto-remote-tag then compute next numeric tag
    // 3) otherwise reuse the local tag
    let remote_tag = if let Some(explicit) = &push_args.remote_tag {
        explicit.clone()
    } else if push_args.auto_remote_tag {
        let next = compute_next_numeric_tag(&push_args.repository)?;
        eprintln!("➡️  Using computed remote monotonic tag: {}", next);
        next.to_string()
    } else {
        push_args.local_tag.clone()
    };

    let account_id = aws_account_id()?;
    ecr_docker_login(&account_id, &AWS_REGION)?;

    let registry = format!("{}.dkr.ecr.{}.amazonaws.com", account_id, AWS_REGION);
    let remote = format!("{}/{}:{}", registry, push_args.repository, remote_tag);

    // docker tag <local>:<local_tag> <remote>:<remote_tag>
    docker_cli(
        vec![
            "tag".into(),
            format!("{}:{}", push_args.image, push_args.local_tag),
            remote.clone(),
        ],
        None,
        None,
        "docker tag failed",
    )?;

    // docker push <remote>:<remote_tag>
    docker_cli(vec!["push".into(), remote], None, None, "docker push failed")
}

/// promote: N to latest and old latest to rollback
fn promote(promote_args: PromoteSubCmdArgs) -> anyhow::Result<()> {
    let prev_latest = ecr_get_manifest(&promote_args.repository, &AWS_REGION, "latest")?;
    let n_manifest = ecr_get_manifest(&promote_args.repository, &AWS_REGION, &promote_args.tag)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Tag '{}' not found in '{}'",
                promote_args.tag,
                promote_args.repository
            )
        })?;

    ecr_put_manifest(&promote_args.repository, &AWS_REGION, "latest", &n_manifest)?;

    if let Some(prev) = prev_latest {
        ecr_put_manifest(&promote_args.repository, &AWS_REGION, "rollback", &prev)?;
    }

    Ok(())
}

/// rollback: promote rollback to latest
fn rollback(rollback_args: RollbackSubCmdArgs) -> anyhow::Result<()> {
    let rb = ecr_get_manifest(&rollback_args.repository, &AWS_REGION, "rollback")?.ok_or(
        anyhow::anyhow!("No 'rollback' tag found in '{}'", rollback_args.repository),
    )?;
    ecr_put_manifest(&rollback_args.repository, &AWS_REGION, "latest", &rb)
}

/// Fetch the latest numerical tag and return it incremented by 1
fn compute_next_numeric_tag(repository: &str) -> anyhow::Result<u64> {
    let json = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "describe-images".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            AWS_REGION.into(),
            "--query".into(),
            "imageDetails[].imageTags[]".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws ecr describe-images",
        None,
        None,
    )?;

    let v: serde_json::Value =
        serde_json::from_str(&json).context("parsing describe-images output")?;
    let mut max_seen: u64 = 0;
    if let serde_json::Value::Array(tags) = v {
        for t in tags {
            if let Some(s) = t.as_str() {
                if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(n) = s.parse::<u64>() {
                        if n > max_seen {
                            max_seen = n;
                        }
                    }
                }
            }
        }
    }

    Ok(max_seen.saturating_add(1).max(1))
}

fn docker_cli(
    args: Vec<String>,
    envs: Option<std::collections::HashMap<&str, &str>>,
    path: Option<&std::path::Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_process("docker", &arg_refs, envs, path, error_msg)
}
