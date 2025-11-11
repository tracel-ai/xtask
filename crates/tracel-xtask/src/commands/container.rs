/// Manage containers.
/// Current implementation uses `docker` and `AWS ECR` as container registry.
use std::path::PathBuf;

use crate::prelude::{ecr_image_url, Context as XtaskContext, Environment};
use crate::utils::aws_cli::{
    aws_account_id, ec2_autoscaling_latest_instance_refresh_status,
    ec2_autoscaling_start_instance_refresh, ecr_compute_next_numeric_tag, ecr_docker_login,
    ecr_ensure_repo_exists, ecr_get_commit_sha_tag_from_alias_tag,
    ecr_get_last_pushed_commit_sha_tag, ecr_get_manifest, ecr_put_manifest,
};
use crate::utils::git::git_repo_root_or_cwd;
use crate::utils::process::run_process;

#[tracel_xtask_macros::declare_command_args(None, ContainerSubCommand)]
pub struct ContainerCmdArgs {}

impl Default for ContainerSubCommand {
    fn default() -> Self {
        ContainerSubCommand::Build(BuildSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct BuildSubCmdArgs {
    /// Path to build file relative to context directory (i.e. a Dockerfile)
    pub build_file: PathBuf,
    /// Build context directory (default to repository root)
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
pub struct ListSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct PushSubCmdArgs {
    /// Local image name (the one used in the build command)
    #[arg(long)]
    pub image: String,
    /// Local image tag (the one used when building)
    #[arg(long)]
    pub local_tag: String,
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name to push into
    #[arg(long)]
    pub repository: String,
    /// Explicit remote tag (if provided, it overrides auto computation)
    #[arg(long)]
    pub remote_tag: Option<String>,
    /// When true, compute the next monotonic tag from the container repository instead of reusing the local tag
    #[arg(long)]
    pub auto_remote_tag: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct PromoteSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// Build tag to promote to 'latest'
    #[arg(long)]
    pub tag: String,
}

#[derive(clap::Args, Clone, PartialEq, Debug)]
pub struct RolloutSubCmdArgs {
    /// Region of the Auto Scaling Group
    #[arg(long)]
    pub region: String,

    /// Name of the Auto Scaling Group to refresh
    #[arg(long, value_name = "ASG_NAME")]
    pub asg: String,

    /// Strategy for instance refresh (Rolling is the standard choice for zero-downtime rollouts)
    #[arg(long, value_name = "Rolling", default_value_t = RolloutSubCmdArgs::default().strategy)]
    pub strategy: String,

    /// Seconds for instance warmup
    #[arg(long, value_name = "SECS", default_value_t = RolloutSubCmdArgs::default().instance_warmup)]
    pub instance_warmup: u64,

    /// Minimum healthy percentage during the rollout
    #[arg(long, value_name = "PCT", default_value_t = RolloutSubCmdArgs::default().min_healthy_percentage)]
    pub min_healthy_percentage: u8,

    /// If set, skip replacing instances that already match the launch template/config
    #[arg(long, default_value_t = RolloutSubCmdArgs::default().skip_matching)]
    pub skip_matching: bool,

    /// Wait until the refresh completes
    #[arg(long, default_value_t = RolloutSubCmdArgs::default().wait)]
    pub wait: bool,

    /// Max seconds to wait when --wait is set
    #[arg(long, default_value_t = RolloutSubCmdArgs::default().wait_timeout_secs)]
    pub wait_timeout_secs: u64,

    /// Poll interval seconds when --wait is set
    #[arg(long, default_value_t = RolloutSubCmdArgs::default().wait_poll_secs)]
    pub wait_poll_secs: u64,
}

impl Default for RolloutSubCmdArgs {
    fn default() -> Self {
        Self {
            region: String::new(),
            asg: String::new(),
            strategy: "Rolling".to_string(),
            instance_warmup: 120,
            min_healthy_percentage: 90,
            skip_matching: true,
            wait: false,
            wait_timeout_secs: 1800,
            wait_poll_secs: 10,
        }
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct RollbackSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
}

pub fn handle_command(
    args: ContainerCmdArgs,
    _env: Environment,
    _ctx: XtaskContext,
) -> anyhow::Result<()> {
    match args.get_command() {
        ContainerSubCommand::Build(build_args) => build(build_args),
        ContainerSubCommand::List(list_args) => list(list_args),
        ContainerSubCommand::Push(push_args) => push(push_args),
        ContainerSubCommand::Promote(promote_args) => promote(promote_args),
        ContainerSubCommand::Rollback(rollback_args) => rollback(rollback_args),
        ContainerSubCommand::Rollout(rollout_args) => rollout(rollout_args),
    }
}

fn build(build_args: BuildSubCmdArgs) -> anyhow::Result<()> {
    let context_dir = build_args.context_dir.unwrap_or(git_repo_root_or_cwd()?);
    let build_file_path = if build_args.build_file.is_absolute() {
        build_args.build_file.clone()
    } else {
        context_dir.join(&build_args.build_file)
    };

    let tag = build_args.tag.as_deref().unwrap_or("latest");
    let mut args: Vec<String> = vec![
        "build".into(),
        format!("--file={}", build_file_path.to_string_lossy()),
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

fn list(list_args: ListSubCmdArgs) -> anyhow::Result<()> {
    let ecr_repository = &list_args.repository;
    let latest_present = ecr_get_manifest(&ecr_repository, &list_args.region, "latest")?.is_some();
    let rollback_present =
        ecr_get_manifest(&ecr_repository, &list_args.region, "rollback")?.is_some();
    let latest_tag = if latest_present {
        ecr_get_commit_sha_tag_from_alias_tag(ecr_repository, "latest", &list_args.region)?
    } else {
        None
    };
    let rollback_tag = if rollback_present {
        ecr_get_commit_sha_tag_from_alias_tag(ecr_repository, "rollback", &list_args.region)?
    } else {
        None
    };
    let last_pushed_tag = ecr_get_last_pushed_commit_sha_tag(ecr_repository, &list_args.region)?;

    eprintln!(
        "📚 Repository: {ecr_repository} (region {})",
        list_args.region
    );
    // current latest
    match (latest_present, &latest_tag) {
        (true, Some(t)) => {
            let url = ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• latest: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• latest:   ✅\n  found but tag unknown"),
        _ => eprintln!("• latest:   ❌"),
    }
    // current rollback
    match (rollback_present, &rollback_tag) {
        (true, Some(t)) => {
            let url = ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• rollback: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• rollback: ✅\n  found but tag unknown"),
        _ => eprintln!("• rollback: ❌"),
    }
    // latest non-alias tag (so not latest or rollback tagged)
    match &last_pushed_tag {
        Some(t) => {
            let url = ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• last pushed: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        None => eprintln!("• last pushed: ❌"),
    }

    Ok(())
}

fn push(push_args: PushSubCmdArgs) -> anyhow::Result<()> {
    ecr_ensure_repo_exists(&push_args.repository, &push_args.region)?;

    // Determine remote tag:
    // 1) if --remote-tag is provided then use it
    // 2) else if --auto-remote-tag then compute next numeric tag
    // 3) otherwise reuse the local tag
    let remote_tag = if let Some(explicit) = &push_args.remote_tag {
        explicit.clone()
    } else if push_args.auto_remote_tag {
        let next = ecr_compute_next_numeric_tag(&push_args.repository, &push_args.region)?;
        eprintln!("➡️  Using computed remote monotonic tag: {}", next);
        next.to_string()
    } else {
        push_args.local_tag.clone()
    };

    let account_id = aws_account_id()?;
    ecr_docker_login(&account_id, &push_args.region)?;

    let registry = format!("{}.dkr.ecr.{}.amazonaws.com", account_id, push_args.region);
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
    docker_cli(
        vec!["push".into(), remote],
        None,
        None,
        "docker push failed",
    )
}

/// promote: N to latest and old latest to rollback
fn promote(promote_args: PromoteSubCmdArgs) -> anyhow::Result<()> {
    let prev_latest = ecr_get_manifest(&promote_args.repository, &promote_args.region, "latest")?;
    let n_manifest = ecr_get_manifest(
        &promote_args.repository,
        &promote_args.region,
        &promote_args.tag,
    )?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "Tag '{}' not found in '{}'",
            promote_args.tag,
            promote_args.repository
        )
    })?;

    ecr_put_manifest(
        &promote_args.repository,
        &promote_args.region,
        "latest",
        &n_manifest,
    )?;

    if let Some(prev) = prev_latest {
        ecr_put_manifest(
            &promote_args.repository,
            &promote_args.region,
            "rollback",
            &prev,
        )?;
    }

    Ok(())
}

/// rollback: promote rollback to latest
fn rollback(rollback_args: RollbackSubCmdArgs) -> anyhow::Result<()> {
    let rb =
        ecr_get_manifest(&rollback_args.repository, &rollback_args.region, "rollback")?.ok_or(
            anyhow::anyhow!("No 'rollback' tag found in '{}'", rollback_args.repository),
        )?;
    ecr_put_manifest(
        &rollback_args.repository,
        &rollback_args.region,
        "latest",
        &rb,
    )
}

/// rollout: rollout latest promoted container
fn rollout(args: RolloutSubCmdArgs) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::{
        io::{self, Write},
        time::{Duration, Instant},
    };

    // Build preferences JSON strictly from flags
    let preferences = serde_json::json!({
        "InstanceWarmup": args.instance_warmup,
        "MinHealthyPercentage": args.min_healthy_percentage,
        "SkipMatching": args.skip_matching,
    })
    .to_string();

    // Kick off the refresh
    let refresh_id = ec2_autoscaling_start_instance_refresh(
        &args.asg,
        &args.region,
        &args.strategy,
        Some(&preferences),
    )
    .context("instance refresh should start")?;

    eprintln!("🚀 Started instance refresh");
    eprintln!("  ASG:     {}", args.asg);
    eprintln!("  Region:  {}", args.region);
    eprintln!("  Refresh: {}", refresh_id);

    // Optional wait for completion with spinner
    if args.wait {
        let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut frame_index = 0;

        let start = Instant::now();
        let timeout = Duration::from_secs(args.wait_timeout_secs);
        let poll = Duration::from_secs(args.wait_poll_secs);

        loop {
            // rotate spinner
            let spinner = spinner_frames[frame_index % spinner_frames.len()];
            frame_index += 1;

            let status_opt =
                ec2_autoscaling_latest_instance_refresh_status(&args.asg, &args.region)
                    .context("instance refresh status should be retrievable")?;

            let (emoji, msg) = match status_opt.as_deref() {
                Some("Pending") => ("⏳", "Pending"),
                Some("InProgress") => ("🚧", "In progress"),
                Some("Successful") => ("✅", "Completed successfully"),
                Some("Failed") => ("❌", "Failed"),
                Some("Cancelled") => ("⚠️", "Cancelled"),
                Some(other) => ("❔", other),
                None => ("🕐", "Waiting..."),
            };

            // Print single-line spinner + status
            print!(
                "\r{spinner}  {emoji}  Refreshing {asg} — Status: {msg:<20}",
                asg = args.asg
            );
            io::stdout().flush().ok();

            // Check terminal states
            match status_opt.as_deref() {
                Some("Successful") => {
                    println!(
                        "\r✅  Rollout completed successfully!{space}",
                        space = " ".repeat(40)
                    );
                    return Ok(());
                }
                Some("Failed") => {
                    println!("\r❌  Rollout failed.{space}", space = " ".repeat(40));
                    anyhow::bail!("rollout finished with status: Failed");
                }
                Some("Cancelled") => {
                    println!("\r⚠️  Rollout cancelled.{space}", space = " ".repeat(40));
                    anyhow::bail!("rollout finished with status: Cancelled");
                }
                _ => {}
            }

            if start.elapsed() >= timeout {
                println!(
                    "\r⏰  Timeout after {}s — rollout still not completed.",
                    args.wait_timeout_secs
                );
                anyhow::bail!("rollout timed out after {} seconds", args.wait_timeout_secs);
            }

            std::thread::sleep(poll);
        }
    }

    Ok(())
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
