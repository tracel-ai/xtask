/// Manage GCP containers.
/// Current implementation uses `docker`, Artifact Registry, and Compute Engine Managed Instance Groups.
use std::io::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use crate::prelude::anyhow::Context as _;
use crate::prelude::*;
use tracel_xtask_utils::{
    gcp::cli::{
        ArtifactRegistryImage, GcpMigRolloutAction, artifact_registry_add_tag,
        artifact_registry_compute_next_numeric_tag, artifact_registry_configure_docker,
        artifact_registry_delete_tag, artifact_registry_ensure_repository_exists,
        artifact_registry_get_digest_from_tag, artifact_registry_get_last_pushed_commit_sha_tag,
        artifact_registry_image_exists, artifact_registry_image_url, artifact_registry_tag_exists,
        mig_console_url, mig_is_stable, mig_start_rolling_action,
    },
    git::git_repo_root_or_cwd,
    process::{run_process, run_process_capture_stdout},
    spinner::{SPINNER_CLR_EOL, Spinner},
};

#[tracel_xtask_macros::declare_command_args(None, GcpContainerSubCommand)]
pub struct GcpContainerCmdArgs {}

impl Default for GcpContainerSubCommand {
    fn default() -> Self {
        GcpContainerSubCommand::Build(GcpContainerBuildSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerBuildSubCmdArgs {
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
    /// Build platform, e.g. linux/amd64.
    #[arg(long)]
    pub platform: Option<String>,
    /// When set, always build the container even if the build tag already exists in Artifact Registry.
    #[arg(long)]
    pub force: bool,
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name. Defaults to --image when omitted.
    #[arg(long)]
    pub remote_image: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerListSubCmdArgs {
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name
    #[arg(long)]
    pub image: String,
    /// The tag representing the latest tag (defaults to the environment name if omitted)
    #[arg(long)]
    pub latest_tag: Option<String>,
    /// Rollback tag applied by this command (defaults to 'rollback_<environment>' if omitted)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerPullSubCmdArgs {
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name
    #[arg(long)]
    pub image: String,
    /// Image tag to pull
    #[arg(long)]
    pub tag: String,
    /// Platform to pull (e.g. linux/amd64), if omitted then docker's default platform is used
    #[arg(long)]
    pub platform: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerPushSubCmdArgs {
    /// Local image name (the one used in the build command)
    #[arg(long)]
    pub image: String,
    /// Local image tag (the one used when building), usually it is the commit SHA
    #[arg(long)]
    pub local_tag: String,
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name. Defaults to --image when omitted.
    #[arg(long)]
    pub remote_image: Option<String>,
    /// Additional explicit remote tag to add (pushed alongside the commit SHA)
    #[arg(long)]
    pub additional_tag: Option<String>,
    /// When set, also add the next monotonic tag alongside the commit SHA
    #[arg(long)]
    pub auto_remote_tag: bool,
    /// Required container platform (e.g. linux/amd64). If set, the local image must match.
    #[arg(long)]
    pub platform: Option<GcpContainerPlatform>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerPromoteSubCmdArgs {
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name
    #[arg(long)]
    pub image: String,
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
pub struct GcpContainerRollbackSubCmdArgs {
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: String,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: String,
    /// Artifact Registry image name
    #[arg(long)]
    pub image: String,
    /// Promote tag applied by this command (defaults to the environment name if omitted)
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag to promote to promote tag (defaults to 'rollback_<environment>' if omitted)
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Clone, PartialEq, Debug)]
pub struct GcpContainerRolloutSubCmdArgs {
    /// GCP project ID
    #[arg(long)]
    pub project: String,
    /// GCP region of the Managed Instance Group
    #[arg(long)]
    pub region: String,
    /// Name of the regional Managed Instance Group to roll
    #[arg(long, value_name = "MIG_NAME")]
    pub mig: String,
    /// Artifact Registry location, e.g. northamerica-northeast1
    #[arg(long)]
    pub location: Option<String>,
    /// Artifact Registry repository name
    #[arg(long)]
    pub repository: Option<String>,
    /// Artifact Registry image name
    #[arg(long)]
    pub image: Option<String>,
    /// Container promote tag, defaults to the environment name.
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// MIG rolling action. Replace is safer when the VM startup/container pull path should re-run.
    #[arg(long, value_enum, default_value_t = GcpContainerRolloutSubCmdArgs::default().action)]
    pub action: GcpMigRolloutAction,
    /// Maximum number/percent of extra instances during the rollout, e.g. 1 or 20%.
    #[arg(long, default_value = "1")]
    pub max_surge: String,
    /// Maximum number/percent of unavailable instances during the rollout, e.g. 0 or 20%.
    #[arg(long, default_value = "0")]
    pub max_unavailable: String,
    /// Wait until the MIG becomes stable
    #[arg(long, default_value_t = GcpContainerRolloutSubCmdArgs::default().wait)]
    pub wait: bool,
    /// Max seconds to wait when --wait is set
    #[arg(long, default_value_t = GcpContainerRolloutSubCmdArgs::default().wait_timeout_secs)]
    pub wait_timeout_secs: u64,
    /// Poll interval seconds when --wait is set
    #[arg(long, default_value_t = GcpContainerRolloutSubCmdArgs::default().wait_poll_secs)]
    pub wait_poll_secs: u64,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct GcpContainerRunSubCmdArgs {
    /// Fully qualified image reference
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

impl Default for GcpContainerRolloutSubCmdArgs {
    fn default() -> Self {
        Self {
            project: String::new(),
            region: String::new(),
            mig: String::new(),
            location: None,
            repository: None,
            image: None,
            promote_tag: None,
            action: GcpMigRolloutAction::Replace,
            max_surge: "1".to_string(),
            max_unavailable: "0".to_string(),
            wait: false,
            wait_timeout_secs: 1800,
            wait_poll_secs: 10,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum GcpContainerPlatform {
    LinuxAmd64,
    LinuxArm64,
}

impl std::fmt::Display for GcpContainerPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GcpContainerPlatform::LinuxAmd64 => write!(f, "linux/amd64"),
            GcpContainerPlatform::LinuxArm64 => write!(f, "linux/arm64"),
        }
    }
}

pub fn handle_command(
    args: GcpContainerCmdArgs,
    env: Environment,
    _ctx: Context,
) -> anyhow::Result<()> {
    match args.get_command() {
        GcpContainerSubCommand::Build(build_args) => build(build_args),
        GcpContainerSubCommand::List(list_args) => list(list_args, &env),
        GcpContainerSubCommand::Pull(pull_args) => pull(pull_args),
        GcpContainerSubCommand::Push(push_args) => push(push_args),
        GcpContainerSubCommand::Promote(promote_args) => promote(promote_args, &env),
        GcpContainerSubCommand::Rollback(rollback_args) => rollback(rollback_args, &env),
        GcpContainerSubCommand::Rollout(rollout_args) => rollout(rollout_args, &env),
        GcpContainerSubCommand::Run(run_args) => run(run_args),
    }
}

fn promote_tag(tag: Option<String>, env: &Environment) -> String {
    tag.unwrap_or(env.to_string())
}

fn rollback_tag(tag: Option<String>, env: &Environment) -> String {
    tag.unwrap_or(format!("rollback_{env}"))
}

fn remote_image_name(local_image: &str, remote_image: Option<String>) -> String {
    remote_image.unwrap_or_else(|| local_image.to_string())
}

fn artifact_image(
    project: impl Into<String>,
    location: impl Into<String>,
    repository: impl Into<String>,
    image: impl Into<String>,
) -> ArtifactRegistryImage {
    ArtifactRegistryImage::new(project, location, repository, image)
}

fn build(build_args: GcpContainerBuildSubCmdArgs) -> anyhow::Result<()> {
    let context_dir = build_args.context_dir.unwrap_or(git_repo_root_or_cwd()?);
    let build_file_path = if build_args.build_file.is_absolute() {
        build_args.build_file.clone()
    } else {
        context_dir.join(&build_args.build_file)
    };
    let tag = build_args.build_tag.as_deref().unwrap_or("latest");
    let remote_image = remote_image_name(&build_args.image, build_args.remote_image);

    let artifact_image = artifact_image(
        build_args.project.clone(),
        build_args.location.clone(),
        build_args.repository.clone(),
        remote_image,
    );

    if artifact_registry_image_exists(&artifact_image, tag)? {
        if build_args.force {
            eprintln!(
                "⚠️ tag already exists in Artifact Registry. Forcing docker build because '--force' is set."
            );
        } else {
            eprintln!(
                "✅ Image already present in Artifact Registry: {}. Skipping build.",
                artifact_image.tagged_ref(tag),
            );
            return Ok(());
        }
    }

    let mut args: Vec<String> = vec![
        "build".into(),
        format!("--file={}", build_file_path.to_string_lossy()),
        format!("--tag={}:{}", build_args.image, tag),
    ];
    if let Some(platform) = build_args.platform {
        args.push(format!("--platform={platform}"));
    }
    for kv in build_args.build_args {
        args.push(format!("--build-arg={kv}"));
    }
    args.push(context_dir.to_string_lossy().into());

    docker_cli(args, None, None, "docker build should succeed")?;

    eprintln!("📦 Built container image: {}", build_args.image);
    eprintln!("🏷️ Image tag: {tag}");
    eprintln!("🔗 Full local name: {}:{tag}", build_args.image);
    Ok(())
}

fn list(list_args: GcpContainerListSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let image = artifact_image(
        list_args.project,
        list_args.location,
        list_args.repository,
        list_args.image,
    );

    let latest_tag = promote_tag(list_args.latest_tag, env);
    let rollback_tag = rollback_tag(list_args.rollback_tag, env);

    let latest_digest = artifact_registry_get_digest_from_tag(&image, &latest_tag)?;
    let rollback_digest = artifact_registry_get_digest_from_tag(&image, &rollback_tag)?;
    let last_pushed_tag = artifact_registry_get_last_pushed_commit_sha_tag(&image)?;

    eprintln!("📚 Artifact Registry image: {}", image.image_ref());

    match latest_digest {
        Some(digest) => {
            let url = artifact_registry_image_url(&image, &latest_tag)?;
            eprintln!("• latest: ✅\n  🏷 {latest_tag}\n  🔎 {digest}\n  🌐 {url}");
        }
        None => eprintln!("• latest: ❌"),
    }

    match rollback_digest {
        Some(digest) => {
            let url = artifact_registry_image_url(&image, &rollback_tag)?;
            eprintln!("• rollback: ✅\n  🏷 {rollback_tag}\n  🔎 {digest}\n  🌐 {url}");
        }
        None => eprintln!("• rollback: ❌"),
    }

    match last_pushed_tag {
        Some(tag) => {
            let url = artifact_registry_image_url(&image, &tag)?;
            eprintln!("• last pushed: ✅\n  🏷 {tag}\n  🌐 {url}");
        }
        None => eprintln!("• last pushed: ❌"),
    }

    Ok(())
}

fn pull(args: GcpContainerPullSubCmdArgs) -> anyhow::Result<()> {
    let image = artifact_image(args.project, args.location, args.repository, args.image);

    eprintln!(
        "📥 Pulling image from Artifact Registry\n Image: {}\n Tag:   {}",
        image.image_ref(),
        args.tag
    );

    artifact_registry_configure_docker(&image.location)?;

    let full_ref = image.tagged_ref(&args.tag);
    let mut docker_args: Vec<String> = vec!["pull".into()];

    if let Some(ref platform) = args.platform {
        docker_args.push("--platform".into());
        docker_args.push(platform.clone());
    }

    docker_args.push(full_ref.clone());

    docker_cli(docker_args, None, None, "docker pull should succeed")?;

    eprintln!("✅ Pulled image: {full_ref}");
    eprintln!("🌐 Console URL: {}", image.console_url(Some(&args.tag)));

    Ok(())
}

fn push(push_args: GcpContainerPushSubCmdArgs) -> anyhow::Result<()> {
    let remote_image = remote_image_name(&push_args.image, push_args.remote_image);

    let image = artifact_image(
        push_args.project,
        push_args.location,
        push_args.repository,
        remote_image,
    );

    artifact_registry_ensure_repository_exists(&image.project, &image.location, &image.repository)?;

    if let Some(ref required) = push_args.platform {
        ensure_local_image_platform(
            &push_args.image,
            &push_args.local_tag,
            &required.to_string(),
        )?;
    }

    if artifact_registry_image_exists(&image, &push_args.local_tag)? {
        eprintln!(
            "ℹ️ Image with commit tag '{}' already exists in Artifact Registry, skipping push...",
            push_args.local_tag
        );

        if let Some(explicit) = &push_args.additional_tag {
            eprintln!(
                "🏷️ Adding explicit alias tag '{}' to existing image",
                explicit
            );
            artifact_registry_add_tag(&image, &push_args.local_tag, explicit)?;
            eprintln!("✅ Added alias tag '{}'", explicit);
        }

        eprintln!("🎉 Push completed");
    } else {
        artifact_registry_configure_docker(&image.location)?;

        let primary_remote = image.tagged_ref(&push_args.local_tag);

        eprintln!(
            "➡️ Preparing to push primary tag (commit): {}",
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
            "docker tag primary Artifact Registry image should succeed",
        )?;

        docker_cli(
            vec!["push".into(), primary_remote.clone()],
            None,
            None,
            "docker push primary Artifact Registry image should succeed",
        )?;

        let mut extra_tags: Vec<String> = Vec::new();

        if push_args.auto_remote_tag {
            let next = artifact_registry_compute_next_numeric_tag(&image)?;
            eprintln!("🔢 Auto monotonic tag computed: {}", next);
            extra_tags.push(next.to_string());
        }

        if let Some(explicit) = &push_args.additional_tag {
            eprintln!("🏷️ Adding explicit extra tag: {}", explicit);
            extra_tags.push(explicit.clone());
        }

        for tag in &extra_tags {
            artifact_registry_add_tag(&image, &push_args.local_tag, tag)?;
            eprintln!("✅ Added extra tag: {}", tag);
        }

        eprintln!("🎉 Push completed");
    }

    let url = artifact_registry_image_url(&image, &push_args.local_tag)?;

    eprintln!(
        "📤 Pushed image: {}:{}",
        push_args.image, push_args.local_tag
    );
    eprintln!("🗄️ Artifact Registry image: {}", image.image_ref());
    eprintln!("🌐 Console URL: {url}");

    Ok(())
}

fn promote(promote_args: GcpContainerPromoteSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let image = artifact_image(
        promote_args.project,
        promote_args.location,
        promote_args.repository,
        promote_args.image,
    );

    let promote_tag = promote_tag(promote_args.promote_tag, env);
    let rollback_tag = rollback_tag(promote_args.rollback_tag, env);

    eprintln!(
        "Promoting '{}' to '{}'...",
        promote_args.build_tag, promote_tag
    );

    if !artifact_registry_tag_exists(&image, &promote_args.build_tag)? {
        anyhow::bail!(
            "Tag '{}' not found in '{}'",
            promote_args.build_tag,
            image.image_ref()
        );
    }

    let current_promote_digest = artifact_registry_get_digest_from_tag(&image, &promote_tag)?;
    let to_promote_digest = artifact_registry_get_digest_from_tag(&image, &promote_args.build_tag)?
        .with_context(|| {
            format!(
                "tag '{}' should resolve to an Artifact Registry version",
                promote_args.build_tag
            )
        })?;

    if current_promote_digest.as_deref() == Some(to_promote_digest.as_str()) {
        eprintln!(
            "ℹ️ Tag '{}' is already promoted as '{promote_tag}' in '{}', no changes needed.",
            promote_args.build_tag,
            image.image_ref(),
        );
        return Ok(());
    }

    if current_promote_digest.is_some() {
        artifact_registry_add_tag(&image, &promote_tag, &rollback_tag).with_context(|| {
            format!("'{rollback_tag}' should point to previous '{promote_tag}'")
        })?;
    }

    artifact_registry_add_tag(&image, &promote_args.build_tag, &promote_tag)
        .with_context(|| format!("'{promote_tag}' should point to target build tag"))?;

    eprintln!(
        "✅ Promoted '{}' to '{promote_tag}'.",
        promote_args.build_tag
    );
    eprintln!("🗄️ Artifact Registry image: {}", image.image_ref());
    eprintln!(
        "🏷️ Tag → (build) {} → (promoted) {promote_tag}",
        promote_args.build_tag
    );
    eprintln!("↩️ Previous '{promote_tag}' container, if any, moved to '{rollback_tag}'");
    eprintln!(
        "🌐 Console URL: {}",
        image.console_url(Some(&promote_args.build_tag))
    );

    Ok(())
}

fn rollback(
    rollback_args: GcpContainerRollbackSubCmdArgs,
    env: &Environment,
) -> anyhow::Result<()> {
    let image = artifact_image(
        rollback_args.project,
        rollback_args.location,
        rollback_args.repository,
        rollback_args.image,
    );

    let promote_tag = promote_tag(rollback_args.promote_tag, env);
    let rollback_tag = rollback_tag(rollback_args.rollback_tag, env);

    if !artifact_registry_tag_exists(&image, &rollback_tag)? {
        anyhow::bail!("No '{rollback_tag}' tag found in '{}'", image.image_ref());
    }

    let rollback_digest = artifact_registry_get_digest_from_tag(&image, &rollback_tag)?
        .with_context(|| {
            format!("'{rollback_tag}' should resolve to an Artifact Registry version")
        })?;

    let promote_digest = artifact_registry_get_digest_from_tag(&image, &promote_tag)?;

    if promote_digest.as_deref() != Some(rollback_digest.as_str()) {
        artifact_registry_add_tag(&image, &rollback_tag, &promote_tag)
            .with_context(|| format!("'{promote_tag}' should be updated to '{rollback_tag}'"))?;

        eprintln!("✅ Promoted '{rollback_tag}' to '{promote_tag}'.");
    } else {
        eprintln!(
            "ℹ️ '{promote_tag}' already points to the '{rollback_tag}' image, skipping promotion..."
        );
    }

    artifact_registry_delete_tag(&image, &rollback_tag)
        .with_context(|| format!("'{rollback_tag}' should be removed after rollback"))?;

    eprintln!("🧹 Removed '{rollback_tag}' tag.");
    eprintln!("⏪ Rolled back!");
    eprintln!("🗄️ Artifact Registry image: {}", image.image_ref());

    match artifact_registry_get_digest_from_tag(&image, &promote_tag)? {
        Some(digest) => {
            eprintln!("✅ '{promote_tag}' now points to: {digest}");
            eprintln!("🌐 Console URL: {}", image.console_url(Some(&promote_tag)));
        }
        None => {
            eprintln!("⚠️ '{promote_tag}' updated, but could not resolve the underlying digest.");
        }
    }

    Ok(())
}

fn rollout(rollout_args: GcpContainerRolloutSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    mig_start_rolling_action(
        &rollout_args.project,
        &rollout_args.region,
        &rollout_args.mig,
        rollout_args.action,
        &rollout_args.max_surge,
        &rollout_args.max_unavailable,
    )
    .context("GCP MIG rolling action should start")?;

    let console_url = mig_console_url(
        &rollout_args.project,
        &rollout_args.region,
        &rollout_args.mig,
    );

    let promote_tag = promote_tag(rollout_args.promote_tag.clone(), env);

    let container_line = match (
        rollout_args.location.as_deref(),
        rollout_args.repository.as_deref(),
        rollout_args.image.as_deref(),
    ) {
        (Some(location), Some(repository), Some(image_name)) => {
            let image = artifact_image(
                rollout_args.project.clone(),
                location.to_string(),
                repository.to_string(),
                image_name.to_string(),
            );

            artifact_registry_get_digest_from_tag(&image, &promote_tag)?
                .map(|digest| format!("  Image:   {}:{promote_tag} ({digest})", image.image_ref()))
        }
        _ => None,
    };

    eprintln!("🚀 Started GCP MIG rolling action");
    eprintln!("  MIG:     {}", rollout_args.mig);
    eprintln!("  Region:  {}", rollout_args.region);
    eprintln!("  Action:  {}", rollout_args.action);
    if let Some(line) = container_line {
        eprintln!("{line}");
    }
    eprintln!("  Console: {}", console_url);

    if rollout_args.wait {
        let mut spinner = Spinner::new();

        let timeout = Duration::from_secs(rollout_args.wait_timeout_secs);
        let poll = Duration::from_secs(rollout_args.wait_poll_secs);

        loop {
            let frame = spinner.next_frame();

            let stable = mig_is_stable(
                &rollout_args.project,
                &rollout_args.region,
                &rollout_args.mig,
            )
            .context("GCP MIG status should be retrievable")?;

            let (emoji, msg) = match stable {
                Some(true) => ("✅", "Stable"),
                Some(false) => ("🚧", "Rolling"),
                None => ("🕐", "Waiting..."),
            };

            let elapsed = spinner.elapsed();
            let (min, sec) = spinner.elapsed_mm_ss();

            print!(
                "\r{frame}  {emoji} ({min:02}:{sec:02}) Rolling {mig} — Status: {msg:<20}{SPINNER_CLR_EOL}",
                mig = rollout_args.mig,
                msg = msg,
            );
            std::io::stdout().flush().ok();

            if stable == Some(true) {
                println!(
                    "\r✅ GCP MIG rollout completed successfully in {min:02}:{sec:02}!{SPINNER_CLR_EOL}"
                );
                return Ok(());
            }

            if elapsed >= timeout {
                println!(
                    "\r⏰ Timeout after {min:02}:{sec:02} (limit: {}s).{SPINNER_CLR_EOL}",
                    rollout_args.wait_timeout_secs
                );
                anyhow::bail!(
                    "GCP MIG rollout timed out after {} seconds",
                    rollout_args.wait_timeout_secs
                );
            }

            std::thread::sleep(poll);
        }
    }

    Ok(())
}

fn run(args: GcpContainerRunSubCmdArgs) -> anyhow::Result<()> {
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

    cli_args.extend(args.extra_arg.clone());
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
