/// Manage containers.
/// Current implementation uses `docker` and `AWS ECR` as container registry.
use std::path::PathBuf;

use crate::prelude::anyhow::Context as _;
use crate::prelude::*;
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
        ContainerSubCommand::Build(ContainerBuildSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerBuildSubCmdArgs {
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
    pub build_tag: Option<String>,
    /// Build arguments
    #[arg(long)]
    pub build_args: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerListSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// The tag reprensenting the latest tag (defaults to the environment name if ommited)
    #[arg(long)]
    pub latest_tag: Option<String>,
    /// Rollback tag applied by this command (defaults to 'rollback_<environment>' if ommited)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerPullSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// Image tag to pull
    #[arg(long)]
    pub tag: String,
    /// Platform to pull (e.g. linux/amd64), if omitted then docker's default platform is used
    #[arg(long)]
    pub platform: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerPushSubCmdArgs {
    /// Local image name (the one used in the build command)
    #[arg(long)]
    pub image: String,
    /// Local image tag (the one used when building), usually it is the commit SHA
    #[arg(long)]
    pub local_tag: String,
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name to push into
    #[arg(long)]
    pub repository: String,
    /// Additional explicit remote tag to add (pushed alongside the commit SHA)
    #[arg(long)]
    pub additional_tag: Option<String>,
    /// When set, also add the next monotonic tag alongside the commit SHA
    #[arg(long)]
    pub auto_remote_tag: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerPromoteSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// Build tag to promote for the given environmentd
    #[arg(long)]
    pub build_tag: String,
    /// Promote tag applied by this command (defaults to the environment name if ommited)
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag applied by this command (defaults to 'rollback_<environment>' if ommited)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerRollbackSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// Promote tag applied by this command (defaults to the environment name if ommited)
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag to promote to promote tag (defaults to 'rollback_<environment>' if ommited)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Clone, PartialEq, Debug)]
pub struct ContainerRolloutSubCmdArgs {
    /// Region of the Auto Scaling Group
    #[arg(long)]
    pub region: String,

    /// Name of the Auto Scaling Group to refresh
    #[arg(long, value_name = "ASG_NAME")]
    pub asg: String,

    /// Strategy for instance refresh (Rolling is the standard choice for zero-downtime rollouts)
    #[arg(long, value_name = "Rolling", default_value_t = ContainerRolloutSubCmdArgs::default().strategy)]
    pub strategy: String,

    /// Seconds for instance warmup
    #[arg(long, value_name = "SECS", default_value_t = ContainerRolloutSubCmdArgs::default().instance_warmup)]
    pub instance_warmup: u64,

    /// Minimum healthy percentage during the rollout
    #[arg(long, value_name = "PCT", default_value_t = ContainerRolloutSubCmdArgs::default().min_healthy_percentage)]
    pub min_healthy_percentage: u8,

    /// If set, skip replacing instances that already match the launch template/config
    #[arg(long, default_value_t = ContainerRolloutSubCmdArgs::default().skip_matching)]
    pub skip_matching: bool,

    /// Wait until the refresh completes
    #[arg(long, default_value_t = ContainerRolloutSubCmdArgs::default().wait)]
    pub wait: bool,

    /// Max seconds to wait when --wait is set
    #[arg(long, default_value_t = ContainerRolloutSubCmdArgs::default().wait_timeout_secs)]
    pub wait_timeout_secs: u64,

    /// Poll interval seconds when --wait is set
    #[arg(long, default_value_t = ContainerRolloutSubCmdArgs::default().wait_poll_secs)]
    pub wait_poll_secs: u64,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerRunSubCmdArgs {
    /// Fully qualified image reference (e.g. 123.dkr.ecr.us-east-1.amazonaws.com/bc-backend:latest)
    #[arg(long)]
    pub image: String,

    /// Container name
    #[arg(long)]
    pub name: Option<String>,

    /// Optional env-file to pass to docker
    #[arg(long)]
    pub env_file: Option<std::path::PathBuf>,

    /// When set, use `--network host`
    #[arg(long)]
    pub host_network: bool,

    /// Extra docker run args, passed as-is after the standard flags
    #[arg(long)]
    pub extra_arg: Vec<String>,
}

impl Default for ContainerRolloutSubCmdArgs {
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

pub fn handle_command(
    args: ContainerCmdArgs,
    env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        ContainerSubCommand::Build(build_args) => build(build_args),
        ContainerSubCommand::List(list_args) => list(list_args, &env),
        ContainerSubCommand::Pull(pull_args) => pull(pull_args),
        ContainerSubCommand::Push(push_args) => push(push_args),
        ContainerSubCommand::Promote(promote_args) => promote(promote_args, &env),
        ContainerSubCommand::Rollback(rollback_args) => rollback(rollback_args, &env),
        ContainerSubCommand::Rollout(rollout_args) => rollout(rollout_args),
        ContainerSubCommand::Run(run_args) => run(run_args),
    }
}

fn promote_tag(tag: Option<String>, env: &Environment) -> String {
    tag.unwrap_or(env.to_string())
}

fn rollback_tag(tag: Option<String>, env: &Environment) -> String {
    tag.unwrap_or(format!("rollback_{env}"))
}

fn build(build_args: ContainerBuildSubCmdArgs) -> anyhow::Result<()> {
    let context_dir = build_args.context_dir.unwrap_or(git_repo_root_or_cwd()?);
    let build_file_path = if build_args.build_file.is_absolute() {
        build_args.build_file.clone()
    } else {
        context_dir.join(&build_args.build_file)
    };

    let tag = build_args.build_tag.as_deref().unwrap_or("latest");
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

fn list(list_args: ContainerListSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let ecr_repository = &list_args.repository;
    let latest_tag = promote_tag(list_args.latest_tag, env);
    let rollback_tag = rollback_tag(list_args.rollback_tag, env);
    let latest_present =
        ecr_get_manifest(ecr_repository, &list_args.region, &latest_tag)?.is_some();
    let rollback_present =
        ecr_get_manifest(ecr_repository, &list_args.region, &rollback_tag)?.is_some();
    let latest_commit_tag = if latest_present {
        ecr_get_commit_sha_tag_from_alias_tag(ecr_repository, &latest_tag, &list_args.region)?
    } else {
        None
    };
    let rollback_tag = if rollback_present {
        ecr_get_commit_sha_tag_from_alias_tag(ecr_repository, &rollback_tag, &list_args.region)?
    } else {
        None
    };
    let last_pushed_tag = ecr_get_last_pushed_commit_sha_tag(ecr_repository, &list_args.region)?;

    eprintln!(
        "📚 Repository: {ecr_repository} (region {})",
        list_args.region
    );
    // current latest
    match (latest_present, &latest_commit_tag) {
        (true, Some(t)) => {
            let url = aws_cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• latest: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• latest: ✅\n  found but tag unknown"),
        _ => eprintln!("• latest: ❌"),
    }
    // current rollback
    match (rollback_present, &rollback_tag) {
        (true, Some(t)) => {
            let url = aws_cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• rollback: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• rollback: ✅\n  found but tag unknown"),
        _ => eprintln!("• rollback: ❌"),
    }
    // latest non-alias tag (so not latest or rollback tagged)
    match &last_pushed_tag {
        Some(t) => {
            let url = aws_cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• last pushed: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        None => eprintln!("• last pushed: ❌"),
    }

    Ok(())
}

fn pull(args: ContainerPullSubCmdArgs) -> anyhow::Result<()> {
    let account_id = aws_account_id()?;
    eprintln!(
        "📥 Pulling image from ECR\n Account: {account_id}\n Region:  {}\n Repo:    {}\n Tag:     {}",
        args.region, args.repository, args.tag
    );
    ecr_docker_login(&account_id, &args.region)?;
    // Build docker args
    let full_ref = format!(
        "{account}.dkr.ecr.{region}.amazonaws.com/{repo}:{tag}",
        account = account_id,
        region = args.region,
        repo = args.repository,
        tag = args.tag,
    );
    let mut docker_args: Vec<String> = vec!["pull".into()];
    if let Some(ref platform) = args.platform {
        docker_args.push("--platform".into());
        docker_args.push(platform.clone());
    }
    docker_args.push(full_ref.clone());
    // pull image
    docker_cli(docker_args, None, None, "docker pull should succeed")?;
    eprintln!("✅ Pulled image: {full_ref}");
    Ok(())
}

fn push(push_args: ContainerPushSubCmdArgs) -> anyhow::Result<()> {
    ecr_ensure_repo_exists(&push_args.repository, &push_args.region)?;
    // check if the container as already been pushed
    if let Some(existing_manifest) = ecr_get_manifest(
        &push_args.repository,
        &push_args.region,
        &push_args.local_tag,
    )? {
        eprintln!(
            "ℹ️ Image with commit tag '{}' already exists in ECR, skipping push...",
            push_args.local_tag
        );

        // If an explicit extra tag is requested, alias it to the same manifest without re-pushing.
        if let Some(explicit) = &push_args.additional_tag {
            eprintln!(
                "🏷️  Adding explicit alias tag '{}' to existing image",
                explicit
            );
            ecr_put_manifest(
                &push_args.repository,
                &push_args.region,
                explicit,
                &existing_manifest,
            )?;
            eprintln!("✅ Added alias tag '{}'", explicit);
        }
        eprintln!("🎉 Push completed");
        return Ok(());
    }

    // login
    let account_id = aws_account_id()?;
    ecr_docker_login(&account_id, &push_args.region)?;

    // push image with primary tag (commit sha)
    let registry = format!("{}.dkr.ecr.{}.amazonaws.com", account_id, push_args.region);
    let repo_full = format!("{}/{}", registry, push_args.repository);
    let primary_remote = format!("{repo_full}:{}", push_args.local_tag);
    eprintln!(
        "➡️  Preparing to push primary tag (commit): {}",
        push_args.local_tag
    );
    docker_cli(
        vec![
            "tag".into(),
            format!("{}:{}", push_args.image, push_args.local_tag),
            primary_remote.clone(),
        ],
        None,
        None,
        "docker tag (primary) should succeed",
    )?;
    docker_cli(
        vec!["push".into(), primary_remote.clone()],
        None,
        None,
        "docker push (primary) should succeed",
    )?;

    // Collect any additional tags we should add in addition to the commit sha
    let mut extra_tags: Vec<String> = Vec::new();
    if push_args.auto_remote_tag {
        let next = ecr_compute_next_numeric_tag(&push_args.repository, &push_args.region)?;
        eprintln!("🔢 Auto monotonic tag computed: {}", next);
        extra_tags.push(next.to_string());
    }
    if let Some(explicit) = &push_args.additional_tag {
        eprintln!("🏷️  Adding explicit extra tag: {}", explicit);
        extra_tags.push(explicit.clone());
    }

    // Push additional tags
    for tag in &extra_tags {
        let remote = format!("{repo_full}:{tag}");
        docker_cli(
            vec![
                "tag".into(),
                format!("{}:{}", push_args.image, push_args.local_tag),
                remote.clone(),
            ],
            None,
            None,
            "docker tag should succeed",
        )?;
        docker_cli(
            vec!["push".into(), remote.clone()],
            None,
            None,
            "docker push should succeed",
        )?;
        eprintln!("✅ Added extra tag: {}", tag);
    }
    eprintln!("🎉 Push completed");

    Ok(())
}

/// promote: point N to `latest` and move the previous `latest` to `rollback`
fn promote(promote_args: ContainerPromoteSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let promote_tag = promote_tag(promote_args.promote_tag, env);
    // Fetch current 'latest' and the target tag's manifest.
    let prev_latest_manifest =
        ecr_get_manifest(&promote_args.repository, &promote_args.region, &promote_tag)
            .context("current '{promote_tag}' manifest should be retrievable")?;
    let n_manifest = ecr_get_manifest(
        &promote_args.repository,
        &promote_args.region,
        &promote_args.build_tag,
    )?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "Tag '{}' not found in '{}'",
            promote_args.build_tag,
            promote_args.repository
        )
    })?;
    // If 'latest' tag is already the target manifest then do nothing
    if let Some(ref prev) = prev_latest_manifest {
        if prev == &n_manifest {
            eprintln!(
                "ℹ️  Tag '{}' is already promoted as '{promote_tag}' in '{}', no changes needed.",
                promote_args.build_tag, promote_args.repository
            );
            return Ok(());
        }
    }
    // Update 'latest' to the new manifest.
    ecr_put_manifest(
        &promote_args.repository,
        &promote_args.region,
        &promote_tag,
        &n_manifest,
    )
    .context("'{promote_tag}' should be updated to the target manifest")?;
    // If there was a previous 'latest', move it to 'rollback'.
    if let Some(prev) = prev_latest_manifest {
        let rollback_tag = rollback_tag(promote_args.rollback_tag, env);
        // Only write rollback if it's different from the new 'latest'.
        ecr_put_manifest(
            &promote_args.repository,
            &promote_args.region,
            &rollback_tag,
            &prev,
        )
        .context("'{rollback_tag}' should be updated to the previous '{promote_tag}'")?;
    }

    eprintln!(
        "✅ Promoted '{}' to '{promote_tag}' in repository '{}'.",
        promote_args.build_tag, promote_args.repository
    );
    Ok(())
}

/// rollback: promote current 'rollback_tag' container to 'promote_tag' and then remove 'rollback_tag'
fn rollback(rollback_args: ContainerRollbackSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let rollback_tag = rollback_tag(rollback_args.rollback_tag, env);
    // Fetch the manifest of the 'rollback' tag
    let rb = ecr_get_manifest(
        &rollback_args.repository,
        &rollback_args.region,
        &rollback_tag,
    )?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "No '{rollback_tag}' tag found in '{}'",
            rollback_args.repository
        )
    })?;
    // If promoted container is different, update it; if it's already the same, skip write.
    let promote_tag = promote_tag(rollback_args.promote_tag, env);
    if ecr_get_manifest(
        &rollback_args.repository,
        &rollback_args.region,
        &promote_tag,
    )?
    .as_ref()
        != Some(&rb)
    {
        ecr_put_manifest(
            &rollback_args.repository,
            &rollback_args.region,
            &promote_tag,
            &rb,
        )
        .context("'{promote_tag}' should be updated to the '{rollback_tag}' manifest")?;
        eprintln!("✅ Promoted '{rollback_tag}' to '{promote_tag}'.");
    } else {
        eprintln!("ℹ️ '{promote_tag}' already points to the '{rollback_tag}' manifest, skipping promotion...");
    }
    // Remove the 'rollback' tag so it no longer aliases this image.
    let filter = format!("imageTag={rollback_tag}");
    let aws_args: Vec<&str> = vec![
        "ecr",
        "batch-delete-image",
        "--repository-name",
        &rollback_args.repository,
        "--image-ids",
        &filter,
        "--region",
        &rollback_args.region,
    ];
    run_process(
        "aws",
        &aws_args,
        None,
        None,
        "removing '{rollback_tag}' tag should succeed",
    )
    .context("failed to remove '{rollback_tag}' tag")?;
    eprintln!("🧹 Removed '{rollback_tag}' tag.");
    Ok(())
}

/// rollout: rollout latest promoted container
fn rollout(args: ContainerRolloutSubCmdArgs) -> anyhow::Result<()> {
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
                        "\r✅ Rollout completed successfully!{space}",
                        space = " ".repeat(40)
                    );
                    return Ok(());
                }
                Some("Failed") => {
                    println!("\r❌ Rollout failed.{space}", space = " ".repeat(40));
                    anyhow::bail!("rollout finished with status: Failed");
                }
                Some("Cancelled") => {
                    println!("\r⚠️ Rollout cancelled.{space}", space = " ".repeat(40));
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

fn run(args: ContainerRunSubCmdArgs) -> anyhow::Result<()> {
    let mut cli_args: Vec<String> = vec!["run".into(), "--rm".into()];

    if let Some(ref name) = args.name {
        cli_args.push("--name".into());
        cli_args.push(name.clone());
    }

    if let Some(ref env_file) = args.env_file {
        cli_args.push("--env-file".into());
        cli_args.push(env_file.to_string_lossy().into_owned());
    }

    if args.host_network {
        cli_args.push("--network".into());
        cli_args.push("host".into());
    }

    // Extra args come before the image
    cli_args.extend(args.extra_arg.clone());

    // Finally, the image (repo:tag, commit tag, whatever)
    cli_args.push(args.image.clone());

    docker_cli(cli_args, None, None, "docker run should succeed")
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
