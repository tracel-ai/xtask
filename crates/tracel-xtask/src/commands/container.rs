use serde::Deserialize;
use std::io::Write as _;
/// Manage containers.
/// Current implementation uses `docker` and `AWS ECR` as container registry.
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::prelude::anyhow::Context as _;
use crate::prelude::*;
use crate::utils::aws::cli::{
    aws_account_id, ec2_autoscaling_latest_instance_refresh_status,
    ec2_autoscaling_start_instance_refresh, ecr_compute_next_numeric_tag, ecr_docker_login,
    ecr_ensure_repo_exists, ecr_get_commit_sha_tag_from_alias_tag,
    ecr_get_last_pushed_commit_sha_tag, ecr_get_manifest, ecr_put_manifest,
};
use crate::utils::aws::instance_system_log::stream_system_log;
use crate::utils::git::git_repo_root_or_cwd;
use crate::utils::process::{run_process, run_process_capture_stdout};

const SSM_SESSION_DOC: &str = "Xtask-Container-InteractiveShell";

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
    /// When set, always build the container even if the build tag already exists in ECR.
    #[arg(long)]
    pub force: bool,
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerHostSubCmdArgs {
    /// Region of the Auto Scaling Group / container host
    #[arg(long)]
    pub region: String,

    /// Name of the Auto Scaling Group hosting the containers
    #[arg(long, value_name = "ASG_NAME")]
    pub asg: String,

    /// Login user for the SSM interactive shell
    #[arg(long, default_value = "ubuntu")]
    pub user: String,

    /// Show instance system log instead of opening an SSM shell
    #[arg(long)]
    pub system_log: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerListSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// The tag reprensenting the latest tag (defaults to the environment name if omitted)
    #[arg(long)]
    pub latest_tag: Option<String>,
    /// Rollback tag applied by this command (defaults to 'rollback_<environment>' if omitted)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerLogsSubCmdArgs {
    /// AWS region to read logs from
    #[arg(long)]
    pub region: String,
    /// CloudWatch Logs log group name
    #[arg(long, value_name = "LOG_GROUP")]
    pub log_group: String,
    /// Follow stream logs (like 'tail -f')
    #[arg(long, default_value_t = false)]
    pub follow: bool,
    /// Only show logs newer than this duration (AWS CLI syntax like: 10m, 2h, 1d)
    #[arg(long, default_value = "10m")]
    pub since: String,
    /// Optional specific log stream names. Repeatable.
    #[arg(long, value_name = "LOG_STREAM", action = clap::ArgAction::Append)]
    pub log_stream_name: Vec<String>,
    /// If set, pick an instance from the specified ASG and tail a stream named after the instance id.
    #[arg(long, value_name = "ASG_NAME")]
    pub asg: Option<String>,
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
    /// Required container platform (e.g. linux/amd64). If set, the local image must match.
    #[arg(long)]
    pub platform: Option<ContainerPlatform>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ContainerPromoteSubCmdArgs {
    /// Region where the container repository lives
    #[arg(long)]
    pub region: String,
    /// Container repository name
    #[arg(long)]
    pub repository: String,
    /// Build tag to promote for the given environment
    #[arg(long)]
    pub build_tag: String,
    /// Promote tag applied by this command (defaults to the environment name if omitted)
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag applied by this command (defaults to 'rollback_<environment>' if omitted)
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
    /// Promote tag applied by this command (defaults to the environment name if omitted)
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag to promote to promote tag (defaults to 'rollback_<environment>' if omitted)
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

    /// Maximum healthy percentage during the rollout
    #[arg(long, value_name = "PCT", default_value_t = ContainerRolloutSubCmdArgs::default().max_healthy_percentage)]
    pub max_healthy_percentage: u8,

    /// Minimum healthy percentage during the rollout
    #[arg(long, value_name = "PCT", default_value_t = ContainerRolloutSubCmdArgs::default().min_healthy_percentage)]
    pub min_healthy_percentage: u8,

    /// Container promote tag, defaults to 'latest'.
    #[arg(long)]
    pub promote_tag: Option<String>,

    /// Container repository.
    #[arg(long)]
    pub repository: Option<String>,

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
            instance_warmup: 60,
            max_healthy_percentage: 125,
            min_healthy_percentage: 100,
            promote_tag: None,
            repository: None,
            skip_matching: false,
            wait: false,
            wait_timeout_secs: 1800,
            wait_poll_secs: 10,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum ContainerPlatform {
    LinuxAmd64,
    LinuxArm64,
}

impl std::fmt::Display for ContainerPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerPlatform::LinuxAmd64 => write!(f, "linux/amd64"),
            ContainerPlatform::LinuxArm64 => write!(f, "linux/arm64"),
        }
    }
}

/// Wrapper used only for display purposes.
pub struct ManifestDigestDisplay<'a>(pub &'a str);

impl<'a> std::fmt::Display for ManifestDigestDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match manifest_digest_short8(self.0) {
            Ok(Some(d)) => write!(f, "{d}"),
            _ => write!(f, "<unknown>"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciIndex {
    manifests: Vec<OciDescriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OciDescriptor {
    digest: String,
}

/// Extract the first 8 hex chars of the sha256 digest from an OCI manifest JSON.
fn manifest_digest_short8(manifest_json: &str) -> anyhow::Result<Option<String>> {
    let index: OciIndex = match serde_json::from_str(manifest_json) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };

    let digest = index.manifests.first().map(|m| m.digest.as_str());
    let digest = match digest {
        Some(d) => d,
        None => return Ok(None),
    };

    let hex = digest.strip_prefix("sha256:").unwrap_or(digest);
    Ok(Some(hex.chars().take(8).collect()))
}

pub fn handle_command(
    args: ContainerCmdArgs,
    env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        ContainerSubCommand::Build(build_args) => build(build_args),
        ContainerSubCommand::Host(host_args) => host(host_args),
        ContainerSubCommand::List(list_args) => list(list_args, &env),
        ContainerSubCommand::Logs(logs_args) => logs(logs_args),
        ContainerSubCommand::Pull(pull_args) => pull(pull_args),
        ContainerSubCommand::Push(push_args) => push(push_args),
        ContainerSubCommand::Promote(promote_args) => promote(promote_args, &env),
        ContainerSubCommand::Rollback(rollback_args) => rollback(rollback_args, &env),
        ContainerSubCommand::Rollout(rollout_args) => rollout(rollout_args, &env),
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

    // If the image tag already exists in ECR, skip docker build unless forced.
    if ecr_get_manifest(&build_args.image, &build_args.region, tag)?.is_some() {
        if build_args.force {
            eprintln!(
                "⚠️ tag already exists in ECR. Forcing build the docker image because '--force' is set."
            );
        } else {
            eprintln!(
                "✅ Image already present in ECR: {}:{} (manifest {}). Skipping build.",
                build_args.image,
                tag,
                ManifestDigestDisplay(
                    &ecr_get_manifest(&build_args.image, &build_args.region, tag)?
                        .unwrap_or_default()
                ),
            );
            return Ok(());
        }
    }

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

    docker_cli(args, None, None, "docker build should succeed")?;

    let image = build_args.image;
    eprintln!("📦 Built container image: {image}");
    eprintln!("🏷️ Image tag: {tag}");
    eprintln!("🔗 Full name: {image}:{tag}");
    Ok(())
}

fn host(args: ContainerHostSubCmdArgs) -> anyhow::Result<()> {
    let selected =
        crate::utils::aws::asg_instance_picker::pick_asg_instance(&args.region, &args.asg)?;
    if args.system_log {
        eprintln!(
            "📜 Streaming system log for {} ({}, {}) — Ctrl-C to stop",
            selected.instance_id,
            selected.private_ip.as_deref().unwrap_or("no-ip"),
            selected.az
        );
        stream_system_log(&args.region, &selected.instance_id)
    } else {
        aws::cli::ensure_ssm_document(SSM_SESSION_DOC, &args.region, &args.user)?;
        eprintln!(
            "🔌 Connecting to {} ({}, {})",
            selected.instance_id,
            selected.private_ip.as_deref().unwrap_or("no-ip"),
            selected.az
        );

        run_process(
            "aws",
            &[
                "ssm",
                "start-session",
                "--target",
                &selected.instance_id,
                "--region",
                &args.region,
                "--document-name",
                SSM_SESSION_DOC,
            ],
            None,
            None,
            "SSM session to container host should start successfully",
        )
    }
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
            let url = aws::cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• latest: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• latest: ✅\n  found but tag unknown"),
        _ => eprintln!("• latest: ❌"),
    }
    // current rollback
    match (rollback_present, &rollback_tag) {
        (true, Some(t)) => {
            let url = aws::cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• rollback: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        (true, None) => eprintln!("• rollback: ✅\n  found but tag unknown"),
        _ => eprintln!("• rollback: ❌"),
    }
    // latest non-alias tag (so not latest or rollback tagged)
    match &last_pushed_tag {
        Some(t) => {
            let url = aws::cli::ecr_image_url(ecr_repository, t, &list_args.region)?.unwrap();
            eprintln!("• last pushed: ✅\n  🏷 {t}\n  🌐 {url}");
        }
        None => eprintln!("• last pushed: ❌"),
    }

    Ok(())
}

fn logs(mut args: ContainerLogsSubCmdArgs) -> anyhow::Result<()> {
    let mut format = "detailed";
    if let Some(asg) = args.asg.as_deref() {
        let selected =
            crate::utils::aws::asg_instance_picker::pick_asg_instance(&args.region, asg)?;
        eprintln!(
            "🪵 Tailing CloudWatch logs for ASG instance {}\n  IP: {}\n  AZ: {}\n  Log group: {}",
            selected.instance_id,
            selected.private_ip.as_deref().unwrap_or("no-ip"),
            selected.az,
            args.log_group,
        );

        let stream =
            crate::utils::aws::instance_logs::resolve_log_stream_name_containing_instance_id(
                &args.region,
                &args.log_group,
                &selected.instance_id,
            )?;
        eprintln!("  Stream: {stream}");
        args.log_stream_name.push(stream);
        // no need to show the instance ID
        format = "short";
    } else {
        eprintln!(
            "🪵 Tailing CloudWatch logs\n  Log group: {}\n  Region: {}\n  Since: {}\n  Follow: {}",
            args.log_group, args.region, args.since, args.follow,
        );
    }

    let mut cli_args: Vec<String> = vec![
        "logs".into(),
        "tail".into(),
        args.log_group.clone(),
        "--region".into(),
        args.region.clone(),
        "--since".into(),
        args.since.clone(),
        "--format".into(),
        format.into(),
    ];

    if args.follow {
        cli_args.push("--follow".into());
    }

    if !args.log_stream_name.is_empty() {
        cli_args.push("--log-stream-names".into());
        cli_args.extend(args.log_stream_name.clone());
    }

    crate::utils::aws::cli::aws_cli(cli_args, None, None, "aws logs tail should succeed")
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
    let url = aws::cli::ecr_image_url(&args.repository, &args.tag, &args.region)?;
    eprintln!("✅ Pulled image: {full_ref}");
    eprintln!("📥 Pulled image from ECR");
    eprintln!("🗄️ ECR repository: {}", args.repository);
    eprintln!("🏷️ Tag: {}", args.tag);
    if let Some(url) = url {
        eprintln!("🌐 Console URL: {url}");
    }
    Ok(())
}

fn push(push_args: ContainerPushSubCmdArgs) -> anyhow::Result<()> {
    // check for repository existenz
    ecr_ensure_repo_exists(&push_args.repository, &push_args.region)?;
    // check for correct container platform
    if let Some(ref required) = push_args.platform {
        ensure_local_image_platform(
            &push_args.image,
            &push_args.local_tag,
            &required.to_string(),
        )?;
    }
    // check if the container has already been pushed
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
    } else {
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
    }

    let url = aws::cli::ecr_image_url(
        &push_args.repository,
        &push_args.local_tag,
        &push_args.region,
    )?
    .unwrap();
    eprintln!(
        "📤 Pushed image: {}:{}",
        push_args.image, push_args.local_tag
    );
    eprintln!("🗄️ ECR repository: {}", push_args.repository);
    eprintln!(
        "🔗 Remote ref: {}:{}",
        push_args.repository, push_args.local_tag
    );
    eprintln!("🌐 Console URL: {url}");
    Ok(())
}

/// promote: point N to `latest` and move the previous `latest` to `rollback`
fn promote(promote_args: ContainerPromoteSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let promote_tag = promote_tag(promote_args.promote_tag, env);
    eprintln!(
        "Promoting '{}' to '{}'...",
        &promote_args.build_tag, &promote_tag
    );

    // Fetch current 'latest' manifest and the new manifest to promote.
    let current_latest_manifest =
        ecr_get_manifest(&promote_args.repository, &promote_args.region, &promote_tag)
            .context("current '{promote_tag}' manifest should be retrievable")?;
    if let Some(ref current) = current_latest_manifest {
        eprintln!(
            "Found previously promoted image with tag '{promote_tag}': {}",
            ManifestDigestDisplay(current),
        );
    }
    let to_promote_manifest = ecr_get_manifest(
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
    eprintln!(
        "Found new image to promote with tag '{promote_tag}': {}",
        ManifestDigestDisplay(&to_promote_manifest),
    );

    // If 'latest' tag is already the target manifest then do nothing
    if let Some(ref current) = current_latest_manifest {
        if current == &to_promote_manifest {
            eprintln!(
                "ℹ️  Tag '{}' is already promoted as '{promote_tag}' in registry '{}', no changes needed.",
                promote_args.build_tag, promote_args.repository
            );
            return Ok(());
        }
    }

    // If there was a previous 'latest', move it to 'rollback'.
    if let Some(current_manifest) = current_latest_manifest {
        let rollback_tag = rollback_tag(promote_args.rollback_tag, env);
        // this should never happen, report a warning in case the rollback tag is already
        // applied to the current manifest and then do nothing
        let current_rollback_manifest = ecr_get_manifest(
            &promote_args.repository,
            &promote_args.region,
            &rollback_tag,
        )
        .context("current '{rollback_tag}' manifest should be retrievable")?;
        if let Some(rollback_manifest) = current_rollback_manifest
            && rollback_manifest == current_manifest
        {
            eprintln!(
                "⚠️ Tag '{rollback_tag}' is already assigned to manifest '{}', this should not happen and might indicate a bug!",
                ManifestDigestDisplay(&current_manifest),
            );
        } else {
            // Update rollback manifest
            ecr_put_manifest(
                &promote_args.repository,
                &promote_args.region,
                &rollback_tag,
                &current_manifest,
            )
            .context(format!(
                "'{rollback_tag}' should be updated to the previous '{promote_tag}'"
            ))?;
        }
    }

    // At last, update 'latest' to the new manifest.
    ecr_put_manifest(
        &promote_args.repository,
        &promote_args.region,
        &promote_tag,
        &to_promote_manifest,
    )
    .context(format!(
        "'{promote_tag}' should be updated to the target manifest"
    ))?;

    // Report
    eprintln!(
        "✅ Promoted '{}' to '{promote_tag}'.",
        promote_args.build_tag
    );
    let url = aws::cli::ecr_image_url(
        &promote_args.repository,
        &promote_args.build_tag,
        &promote_args.region,
    )?
    .unwrap();
    eprintln!("🗄️ Repository: {}", promote_args.repository);
    eprintln!(
        "🏷️ Tag → (build) {} → (latest) {promote_tag}",
        promote_args.build_tag
    );
    eprintln!("↩️ Previous '{promote_tag}' container (if any) moved to 'rollback_{promote_tag}'");
    eprintln!("🌐 Console URL: {url}");
    Ok(())
}

/// rollback: promote current 'rollback_tag' container to 'promote_tag' and then remove 'rollback_tag'
fn rollback(rollback_args: ContainerRollbackSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let rollback_tag = rollback_tag(rollback_args.rollback_tag, env);
    // Fetch the manifest of the 'rollback' tag
    let current_rb_manifest = ecr_get_manifest(
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
    // If currently promoted container is different of the rollback one,
    // then put the promoted tag on the rollback manifest
    let promote_tag = promote_tag(rollback_args.promote_tag, env);
    if ecr_get_manifest(
        &rollback_args.repository,
        &rollback_args.region,
        &promote_tag,
    )?
    .as_ref()
        != Some(&current_rb_manifest)
    {
        ecr_put_manifest(
            &rollback_args.repository,
            &rollback_args.region,
            &promote_tag,
            &current_rb_manifest,
        )
        .context(format!(
            "'{promote_tag}' should be updated to the '{rollback_tag}' manifest"
        ))?;
        eprintln!(
            "✅ Promoted '{rollback_tag}' manifest '{}' to '{promote_tag}'.",
            ManifestDigestDisplay(&current_rb_manifest),
        );
    } else {
        eprintln!(
            "ℹ️ '{promote_tag}' already points to the '{rollback_tag}' manifest, skipping promotion..."
        );
    }

    // Remove the 'rollback' tag so it no longer aliases this image.
    let filter = format!("imageTag={rollback_tag}");
    aws::cli::aws_ecr_delete_tag_quiet(
        &rollback_args.repository,
        &rollback_args.region,
        &filter,
        &rollback_tag,
    )?;
    eprintln!("🧹 Removed '{rollback_tag}' tag.");
    eprintln!("⏪ Rolled back!");
    eprintln!("🗄️ Repository: {}", rollback_args.repository);
    let promote_commit_sha = aws::cli::ecr_get_commit_sha_tag_from_alias_tag(
        &rollback_args.repository,
        &promote_tag,
        &rollback_args.region,
    )?;
    match promote_commit_sha {
        Some(t) => {
            let url =
                aws::cli::ecr_image_url(&rollback_args.repository, &t, &rollback_args.region)?
                    .unwrap();
            eprintln!("✅ '{promote_tag}' now points to: {t}");
            eprintln!("🌐 Console URL: {url}");
        }
        None => {
            // This would be unusual since all the containers should be tagged with the commit sha
            eprintln!(
                "⚠️ '{promote_tag}' updated, but could not resolve the underlying commit SHA."
            );
        }
    }

    Ok(())
}

/// rollout: rollout latest promoted container for current environment
fn rollout(rollout_args: ContainerRolloutSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    // Build preferences JSON strictly from flags
    let preferences = serde_json::json!({
        "InstanceWarmup": rollout_args.instance_warmup,
        "MaxHealthyPercentage": rollout_args.max_healthy_percentage,
        "MinHealthyPercentage": rollout_args.min_healthy_percentage,
        "SkipMatching": rollout_args.skip_matching,
    })
    .to_string();

    // Kick off the refresh
    let refresh_id = ec2_autoscaling_start_instance_refresh(
        &rollout_args.asg,
        &rollout_args.region,
        &rollout_args.strategy,
        Some(&preferences),
    )
    .context("instance refresh should start")?;

    let console_url = format!(
        "https://{region}.console.aws.amazon.com/ec2/home?region={region}#AutoScalingGroupDetails:id={asg};view=instanceRefresh",
        region = rollout_args.region,
        asg = rollout_args.asg,
    );

    // show the concrete commit SHA tag of the container being rolled out
    let promote_tag = promote_tag(rollout_args.promote_tag, env);
    let container_line = match rollout_args.repository.as_deref() {
        Some(repo) => {
            ecr_get_commit_sha_tag_from_alias_tag(repo, &promote_tag, &rollout_args.region)?
                .map(|commit_tag| format!("  Image:   {repo}:{commit_tag}"))
        }
        None => None,
    };

    eprintln!("🚀 Started instance refresh");
    eprintln!("  ASG:     {}", rollout_args.asg);
    eprintln!("  Region:  {}", rollout_args.region);
    if let Some(line) = container_line {
        eprintln!("{line}");
    }
    eprintln!("  Refresh: {}", refresh_id);
    eprintln!("  Console: {}", console_url);

    if rollout_args.wait {
        let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut frame_index = 0;

        let mut start = Instant::now();
        let timeout = Duration::from_secs(rollout_args.wait_timeout_secs);
        let poll = Duration::from_secs(rollout_args.wait_poll_secs);
        const CLR_EOL: &str = "\x1b[K";

        // Track whether we already triggered a container rollback
        let mut rollback_triggered = false;

        loop {
            let spinner = spinner_frames[frame_index % spinner_frames.len()];
            frame_index += 1;

            let status_opt = ec2_autoscaling_latest_instance_refresh_status(
                &rollout_args.asg,
                &rollout_args.region,
            )
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

            // elapsed time in mm:ss (within current window)
            let elapsed = start.elapsed();
            let elapsed_secs = elapsed.as_secs();
            let min = elapsed_secs / 60;
            let sec = elapsed_secs % 60;

            print!(
                "\r{spinner}  {emoji} ({min:02}:{sec:02}) Refreshing {asg} — Status: {msg:<20}{CLR_EOL}",
                asg = rollout_args.asg,
                msg = msg,
            );
            std::io::stdout().flush().ok();

            match status_opt.as_deref() {
                Some("Successful") => {
                    println!("\r✅ Rollout completed successfully in {min:02}:{sec:02}!{CLR_EOL}");

                    if rollback_triggered {
                        anyhow::bail!(
                            "rollout completed successfully but a container rollback was triggered during the wait window"
                        );
                    }
                    return Ok(());
                }
                Some("Failed") => {
                    println!("\r❌ Rollout failed after {min:02}:{sec:02}.{CLR_EOL}");
                    anyhow::bail!("rollout finished with status: Failed");
                }
                Some("Cancelled") => {
                    println!("\r⚠️ Rollout cancelled after {min:02}:{sec:02}.{CLR_EOL}");
                    anyhow::bail!("rollout finished with status: Cancelled");
                }
                _ => {}
            }

            if elapsed >= timeout {
                if !rollback_triggered {
                    // FIRST TIMEOUT → trigger container rollback and restart the timer
                    println!(
                        "\r⏰ Timeout after {min:02}:{sec:02} (limit: {}s).{CLR_EOL}",
                        rollout_args.wait_timeout_secs
                    );
                    eprintln!(
                        "🛟 Rolling back container state while keeping the current instance refresh..."
                    );

                    let rollback_tag = rollback_tag(None, env);
                    if let Some(ref repo) = rollout_args.repository {
                        let rb_args = ContainerRollbackSubCmdArgs {
                            region: rollout_args.region.clone(),
                            repository: repo.clone(),
                            promote_tag: Some(promote_tag.clone()),
                            rollback_tag: Some(rollback_tag),
                        };

                        rollback(rb_args, env).context("Container rollback should succeed")?;

                        rollback_triggered = true;
                        // restart the timer for a second window
                        start = Instant::now();
                        continue;
                    } else {
                        eprintln!(
                            "⚠️ No container repository was provided to 'rollout', skipping container rollback."
                        );
                        anyhow::bail!(
                            "rollout timed out after {} seconds and no container repository was provided to roll back",
                            rollout_args.wait_timeout_secs
                        );
                    }
                } else {
                    // SECOND TIMEOUT → hard error out
                    println!(
                        "\r⏰ Timeout after container rollback: {min:02}:{sec:02} (extra limit: {}s).{CLR_EOL}",
                        rollout_args.wait_timeout_secs
                    );
                    anyhow::bail!(
                        "rollout still not successful after container rollback and an additional {} seconds",
                        rollout_args.wait_timeout_secs
                    );
                }
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

    docker_cli(cli_args, None, None, "docker run should succeed")?;

    eprintln!("▶️ Running container: {}", args.image);
    if let Some(ref env_file) = args.env_file {
        eprintln!("📄 Using merged env file: {}", env_file.display());
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

fn docker_image_platform(reference: &str) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("docker");
    cmd.arg("inspect")
        .arg("--format={{.Os}}/{{.Architecture}}")
        .arg(reference);

    let out = run_process_capture_stdout(&mut cmd, "docker inspect image platform")?;
    Ok(out.trim().to_string())
}

fn ensure_local_image_platform(
    image: &str,
    tag: &str,
    expected_platform: &str,
) -> anyhow::Result<()> {
    let reference = format!("{image}:{tag}");
    let actual = docker_image_platform(&reference)
        .with_context(|| format!("docker inspect for image '{reference}' should succeed"))?;

    if actual != expected_platform {
        anyhow::bail!(
            "Local image '{reference}' platform should be '{expected}', found '{actual}'",
            expected = expected_platform,
            actual = actual,
        );
    }

    Ok(())
}
