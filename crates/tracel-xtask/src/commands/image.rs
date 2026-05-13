use std::collections::BTreeMap;
use std::io::Write as _;
/// Manage AWS virtual machine images.
/// Current implementation uses Terraform-managed baker EC2 instances and AWS AMIs.
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::prelude::anyhow::Context as _;
use crate::prelude::*;
use tracel_xtask_utils::spinner::Spinner;
use tracel_xtask_utils::{
    aws::{
        images::{
            create_image, create_true_tag, delete_tag, ensure_image_matches_name,
            find_baker_instance, find_single_image_by_true_tag, get_image_by_id,
            print_image_summary,
        },
        instance_system_log::stream_system_log,
    },
    git::git_repo_root_or_cwd,
    process::{run_process, run_process_capture_stdout},
};

const POLLING_INTERVAL_SECS: u64 = 5;
const SSM_SESSION_DOC: &str = "Xtask-Image-InteractiveShell";

#[tracel_xtask_macros::declare_command_args(None, ImageSubCommand)]
pub struct ImageCmdArgs {}

impl Default for ImageSubCommand {
    fn default() -> Self {
        ImageSubCommand::List(ImageListSubCmdArgs::default())
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImageBuildSubCmdArgs {
    /// Region where the baker instances and AMIs live.
    #[arg(long)]
    pub region: String,
    /// Terraform state/root responsible for creating baker instances.
    #[arg(long, value_name = "PATH")]
    pub tf_root: PathBuf,
    /// Logical image names to bake. Repeatable.
    #[arg(long = "image", value_name = "IMAGE_NAME", action = clap::ArgAction::Append)]
    pub images: Vec<String>,
    /// Additional AMI tags to apply at creation time. Repeatable, format: KEY=VALUE.
    #[arg(long = "tag", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
    pub tags: Vec<String>,
    /// Timeout while waiting for baker instances to stop.
    #[arg(long, default_value_t = 600)]
    pub stop_timeout_secs: u64,
    /// Timeout while waiting for AMIs to become available.
    #[arg(long, default_value_t = 600)]
    pub ami_timeout_secs: u64,
    /// If set, skip the Terraform apply before looking for baker instances.
    #[arg(long)]
    pub skip_apply: bool,
    /// If set, create the AMI without rebooting the source instance.
    #[arg(long)]
    pub no_reboot: bool,
    /// Extra Terraform apply args.
    #[arg(long, value_name = "ARG", action = clap::ArgAction::Append)]
    pub tf_arg: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImagePromoteSubCmdArgs {
    /// Region where the AMI lives.
    #[arg(long)]
    pub region: String,
    /// Logical image name.
    #[arg(long)]
    pub image: String,
    /// AMI id to promote.
    #[arg(long, value_name = "AMI_ID")]
    pub ami_id: String,
    /// Promote tag key applied by this command. Defaults to `latest_<environment>`.
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag key applied by this command. Defaults to `rollback_<environment>`.
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImageRollbackSubCmdArgs {
    /// Region where the AMI lives.
    #[arg(long)]
    pub region: String,
    /// Logical image name.
    #[arg(long)]
    pub image: String,
    /// Promote tag key applied by this command. Defaults to `latest_<environment>`.
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag key to promote. Defaults to `rollback_<environment>`.
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImageRolloutSubCmdArgs {
    /// Terraform state/root responsible for consuming the promoted AMI.
    #[arg(long, value_name = "PATH")]
    pub tf_root: PathBuf,
    /// Extra Terraform apply args.
    #[arg(long, value_name = "ARG", action = clap::ArgAction::Append)]
    pub tf_arg: Vec<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImageListSubCmdArgs {
    /// Region where the AMIs live.
    #[arg(long)]
    pub region: String,
    /// Logical image name.
    #[arg(long)]
    pub image: String,
    /// Promote tag key. Defaults to `latest_<environment>`.
    #[arg(long)]
    pub promote_tag: Option<String>,
    /// Rollback tag key. Defaults to `rollback_<environment>`.
    #[arg(long)]
    pub rollback_tag: Option<String>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ImageHostSubCmdArgs {
    /// Region of the baker instance.
    #[arg(long)]
    pub region: String,
    /// Logical image name of the baker instance.
    #[arg(long)]
    pub image: String,
    /// Login user for the SSM interactive shell.
    #[arg(long, default_value = "ubuntu")]
    pub user: String,
    /// Show instance system log instead of opening an SSM shell.
    #[arg(long)]
    pub system_log: bool,
}

#[derive(Debug)]
struct BuiltImage {
    image: String,
    ami_id: String,
    ami_name: String,
}

#[derive(Debug, Clone)]
enum ImageBuildState {
    DiscoveringBaker,
    Baking {
        instance_id: String,
        state: String,
    },
    CreatingImage {
        instance_id: String,
        ami_name: String,
        state: Option<String>,
    },
    Complete {
        ami_id: String,
        ami_name: String,
    },
    Failed {
        error: String,
    },
}

impl ImageBuildState {
    fn is_completed(&self) -> bool {
        matches!(
            self,
            ImageBuildState::Complete { .. } | ImageBuildState::Failed { .. }
        )
    }

    fn emoji(&self) -> &'static str {
        match self {
            ImageBuildState::DiscoveringBaker => "🔎",
            ImageBuildState::Baking { .. } => "🔥",
            ImageBuildState::CreatingImage { .. } => "📸",
            ImageBuildState::Complete { .. } => "✅",
            ImageBuildState::Failed { .. } => "❌",
        }
    }

    fn phase(&self) -> &'static str {
        match self {
            ImageBuildState::DiscoveringBaker => "discovering",
            ImageBuildState::Baking { .. } => "baking",
            ImageBuildState::CreatingImage { .. } => "creating image",
            ImageBuildState::Complete { .. } => "complete",
            ImageBuildState::Failed { .. } => "failed",
        }
    }

    fn details(&self) -> String {
        match self {
            ImageBuildState::DiscoveringBaker => "looking for baker instance".to_owned(),
            ImageBuildState::Baking { instance_id, state } => {
                format!("{instance_id} state={state}")
            }
            ImageBuildState::CreatingImage {
                instance_id,
                ami_name,
                state,
            } => match state {
                Some(state) => format!("{ami_name} from {instance_id} state={state}"),
                None => format!("{ami_name} from {instance_id}"),
            },
            ImageBuildState::Complete { ami_id, ami_name } => {
                format!("{ami_id} ({ami_name})")
            }
            ImageBuildState::Failed { error } => error.clone(),
        }
    }
}

#[derive(Debug)]
enum ImageBuildEvent {
    State {
        image: String,
        state: ImageBuildState,
    },
}

struct LiveImageBuildTable {
    statuses: BTreeMap<String, ImageBuildState>,
    lines_rendered: usize,
    started_at: Instant,
    spinner: Spinner,
    stop_timeout: Duration,
    ami_timeout: Duration,
}

impl LiveImageBuildTable {
    fn new(images: &[String], stop_timeout: Duration, ami_timeout: Duration) -> Self {
        let statuses = images
            .iter()
            .map(|image| (image.clone(), ImageBuildState::DiscoveringBaker))
            .collect();

        Self {
            statuses,
            lines_rendered: 0,
            started_at: Instant::now(),
            spinner: Spinner::new(),
            stop_timeout,
            ami_timeout,
        }
    }

    fn update(&mut self, image: String, state: ImageBuildState) {
        self.statuses.insert(image, state);
    }

    fn render(&mut self) {
        if self.lines_rendered > 0 {
            // Move cursor back to the first line of the previous render.
            print!("\x1b[{}A", self.lines_rendered);
        }

        let elapsed = self.started_at.elapsed().as_secs();
        let min = elapsed / 60;
        let sec = elapsed % 60;
        let spinner = self.spinner.next_frame();

        let mut lines = Vec::new();
        lines.push(
            "───────────────────────────────────────────────────────────────────────────"
                .to_owned(),
        );
        lines.push(format!(
            "👷‍♂️ Image build status ({min:02}:{sec:02}) [stop timeout: {}, AMI timeout: {}]",
            format_duration(&self.stop_timeout),
            format_duration(&self.ami_timeout),
        ));
        lines.push(format!(
            "{:<4} {:<24} {:<18} {}",
            "", "Image", "Phase", "Details"
        ));
        lines.push(
            "──── ──────────────────────── ────────────────── ─────────────────────────".to_owned(),
        );

        for (image, state) in &self.statuses {
            let marker = if state.is_completed() {
                state.emoji()
            } else {
                &format!("{}{spinner}", state.emoji())
            };

            lines.push(format!(
                "{:<4} {:<24} {:<18} {}",
                marker,
                truncate(image, 24),
                state.phase(),
                truncate(&state.details(), 80),
            ));
        }

        for line in &lines {
            // Clear each line before writing so shorter updates do not leave garbage.
            println!("\r\x1b[2K{line}");
        }

        self.lines_rendered = lines.len();
        std::io::stdout().flush().ok();
    }

    fn finish(&mut self) {
        self.render();
        println!();
        self.lines_rendered = 0;
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

pub fn handle_command(args: ImageCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    match args.get_command() {
        ImageSubCommand::Build(build_args) => build(build_args, &env.into_explicit()),
        ImageSubCommand::Promote(promote_args) => promote(promote_args, &env.into_explicit()),
        ImageSubCommand::Rollback(rollback_args) => rollback(rollback_args, &env.into_explicit()),
        ImageSubCommand::Rollout(rollout_args) => rollout(rollout_args),
        ImageSubCommand::List(list_args) => list(list_args, &env.into_explicit()),
        ImageSubCommand::Host(host_args) => host(host_args),
    }
}

fn promote_tag(tag: Option<String>, env: &Environment<ExplicitIndex>) -> String {
    tag.unwrap_or(format!("latest_{env}"))
}

fn rollback_tag(tag: Option<String>, env: &Environment<ExplicitIndex>) -> String {
    tag.unwrap_or(format!("rollback_{env}"))
}

fn build(args: ImageBuildSubCmdArgs, env: &Environment<ExplicitIndex>) -> anyhow::Result<()> {
    if args.images.is_empty() {
        anyhow::bail!("at least one '--image <IMAGE_NAME>' should be provided");
    }

    let tf_root = normalize_path(args.tf_root)?;

    if !args.skip_apply {
        eprintln!("🏗️ Applying image baker Terraform state");
        terraform_apply(&tf_root, &args.tf_arg)
            .context("image baker Terraform state should apply successfully")?;
    }

    let mut tags = parse_tags(&args.tags)?;
    tags.push(("env".to_owned(), env.long().to_owned()));
    let region = args.region.clone();
    let stop_timeout = Duration::from_secs(args.stop_timeout_secs);
    let ami_timeout = Duration::from_secs(args.ami_timeout_secs);
    let no_reboot = args.no_reboot;

    let (event_tx, event_rx) = mpsc::channel::<ImageBuildEvent>();

    let built_images = std::thread::scope(|scope| {
        let mut handles = Vec::new();

        for image in args.images.clone() {
            let region = region.clone();
            let tags = tags.clone();
            let event_tx = event_tx.clone();
            let env = env.clone();

            handles.push(scope.spawn(move || {
                build_image_lifecycle(
                    &region,
                    &image,
                    &env,
                    stop_timeout,
                    ami_timeout,
                    no_reboot,
                    &tags,
                    &event_tx,
                )
            }));
        }

        drop(event_tx);

        let printer = scope.spawn(move || {
            let mut table = LiveImageBuildTable::new(&args.images, stop_timeout, ami_timeout);
            table.render();

            loop {
                match event_rx.recv_timeout(Duration::from_millis(120)) {
                    Ok(ImageBuildEvent::State { image, state }) => {
                        table.update(image, state);
                        table.render();
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        table.render();
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }

            table.finish();
        });

        let mut built_images = Vec::new();
        let mut errors = Vec::new();

        for handle in handles {
            match handle.join().map_err(thread_panic_error)? {
                Ok(built) => built_images.push(built),
                Err(err) => errors.push(err),
            }
        }

        printer.join().map_err(thread_panic_error)?;

        if !errors.is_empty() {
            for err in &errors {
                eprintln!("❌ {err:#}");
            }

            anyhow::bail!("{} image build worker(s) failed", errors.len());
        }

        anyhow::Ok(built_images)
    })?;

    eprintln!("🎉 Image build completed");
    for built in built_images {
        eprintln!("• {}: {} ({})", built.image, built.ami_id, built.ami_name);
    }

    Ok(())
}

fn promote(args: ImagePromoteSubCmdArgs, env: &Environment<ExplicitIndex>) -> anyhow::Result<()> {
    let promote_tag = promote_tag(args.promote_tag, env);
    let rollback_tag = rollback_tag(args.rollback_tag, env);

    let target = get_image_by_id(&args.region, &args.ami_id)?
        .ok_or_else(|| anyhow::anyhow!("AMI '{}' should exist", args.ami_id))?;

    ensure_image_matches_name(&target, &args.image)?;

    let current_latest = find_single_image_by_true_tag(&args.region, &args.image, &promote_tag)?;
    let current_rollback = find_single_image_by_true_tag(&args.region, &args.image, &rollback_tag)?;

    if current_latest.as_ref().map(|image| image.image_id.as_str()) == Some(args.ami_id.as_str()) {
        eprintln!(
            "ℹ️ AMI '{}' is already promoted with tag '{}=true', no changes needed.",
            args.ami_id, promote_tag
        );
        return Ok(());
    }

    eprintln!("🚀 Promoting AMI");
    eprintln!("  Image:       {}", args.image);
    eprintln!("  Target AMI:  {}", args.ami_id);
    eprintln!("  Promote tag: {promote_tag}=true");
    eprintln!("  Rollback tag: {rollback_tag}=true");

    if let Some(current_rollback) = current_rollback {
        eprintln!(
            "🧹 Removing previous rollback tag from {}",
            current_rollback.image_id
        );
        delete_tag(&args.region, &current_rollback.image_id, &rollback_tag)
            .context("previous rollback tag should be removed")?;
    }

    if let Some(current_latest) = current_latest {
        eprintln!(
            "↩️ Moving previous promoted AMI {} to rollback",
            current_latest.image_id
        );

        delete_tag(&args.region, &current_latest.image_id, &promote_tag)
            .context("previous promoted tag should be removed")?;

        create_true_tag(&args.region, &current_latest.image_id, &rollback_tag)
            .context("rollback tag should be applied to previous promoted AMI")?;
    }

    delete_tag(&args.region, &args.ami_id, &rollback_tag)
        .context("target AMI rollback tag should be removed before promotion")?;

    create_true_tag(&args.region, &args.ami_id, &promote_tag)
        .context("promote tag should be applied to target AMI")?;

    eprintln!("✅ Promoted AMI '{}'", args.ami_id);
    eprintln!("  {}=true", promote_tag);
    eprintln!("  Image={}", args.image);

    Ok(())
}

fn rollback(args: ImageRollbackSubCmdArgs, env: &Environment<ExplicitIndex>) -> anyhow::Result<()> {
    let promote_tag = promote_tag(args.promote_tag, env);
    let rollback_tag = rollback_tag(args.rollback_tag, env);

    let current_latest = find_single_image_by_true_tag(&args.region, &args.image, &promote_tag)?;
    let current_rollback = find_single_image_by_true_tag(&args.region, &args.image, &rollback_tag)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No rollback AMI found for image '{}' with tag '{}=true'",
                args.image,
                rollback_tag
            )
        })?;

    eprintln!("⏪ Rolling back AMI");
    eprintln!("  Image:        {}", args.image);
    eprintln!("  Rollback AMI: {}", current_rollback.image_id);
    eprintln!("  Promote tag:  {promote_tag}=true");
    eprintln!("  Rollback tag: {rollback_tag}=true");

    if let Some(current_latest) = current_latest {
        if current_latest.image_id != current_rollback.image_id {
            delete_tag(&args.region, &current_latest.image_id, &promote_tag)
                .context("current promoted tag should be removed")?;
        }
    }

    create_true_tag(&args.region, &current_rollback.image_id, &promote_tag)
        .context("promote tag should be applied to rollback AMI")?;

    delete_tag(&args.region, &current_rollback.image_id, &rollback_tag)
        .context("rollback tag should be removed after rollback")?;

    eprintln!("✅ Rolled back image '{}'", args.image);
    eprintln!("  {}=true → {}", promote_tag, current_rollback.image_id);
    eprintln!("  Removed {}=true", rollback_tag);

    Ok(())
}

fn rollout(args: ImageRolloutSubCmdArgs) -> anyhow::Result<()> {
    let tf_root = normalize_path(args.tf_root)?;

    eprintln!("🚀 Applying Terraform application state");
    eprintln!("  Root: {}", tf_root.display());

    terraform_apply(&tf_root, &args.tf_arg)
        .context("image rollout Terraform state should apply successfully")?;

    eprintln!("✅ Rollout Terraform apply completed");

    Ok(())
}

fn list(args: ImageListSubCmdArgs, env: &Environment<ExplicitIndex>) -> anyhow::Result<()> {
    let promote_tag = promote_tag(args.promote_tag, env);
    let rollback_tag = rollback_tag(args.rollback_tag, env);

    let latest = find_single_image_by_true_tag(&args.region, &args.image, &promote_tag)?;
    let rollback = find_single_image_by_true_tag(&args.region, &args.image, &rollback_tag)?;

    eprintln!("📚 Image family: {}", args.image);
    eprintln!("  Region: {}", args.region);

    match latest {
        Some(image) => {
            eprintln!("• latest: ✅");
            print_image_summary(&image);
        }
        None => eprintln!("• latest: ❌"),
    }

    match rollback {
        Some(image) => {
            eprintln!("• rollback: ✅");
            print_image_summary(&image);
        }
        None => eprintln!("• rollback: ❌"),
    }

    Ok(())
}

fn host(args: ImageHostSubCmdArgs) -> anyhow::Result<()> {
    let selected = find_baker_instance(&args.region, &args.image)?;

    if args.system_log {
        eprintln!(
            "📜 Streaming system log for {} ({}, {}) — Ctrl-C to stop",
            selected.instance_id,
            selected.private_ip.as_deref().unwrap_or("no-ip"),
            selected.placement.availability_zone,
        );

        stream_system_log(&args.region, &selected.instance_id)
    } else {
        aws::cli::ensure_ssm_document(SSM_SESSION_DOC, &args.region, &args.user)?;

        eprintln!(
            "🔌 Connecting to {} ({}, {})",
            selected.instance_id,
            selected.private_ip.as_deref().unwrap_or("no-ip"),
            selected.placement.availability_zone,
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
            "SSM session to image baker should start successfully",
        )
    }
}

fn terraform_apply(tf_root: &Path, extra_args: &[String]) -> anyhow::Result<()> {
    let mut args: Vec<String> = vec!["apply".into(), "-auto-approve".into()];
    args.extend(extra_args.iter().cloned());

    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    run_process(
        "terraform",
        &arg_refs,
        None,
        Some(tf_root),
        "terraform apply should succeed",
    )
}

fn normalize_path(path: PathBuf) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(git_repo_root_or_cwd()?.join(path))
    }
}

fn build_ami_name(env: &Environment<ExplicitIndex>, image: &str) -> anyhow::Result<String> {
    let git_sha = git_short_sha().unwrap_or_else(|_| "unknown".to_string());
    let timestamp = utc_timestamp_compact()?;

    Ok(format!(
        "{}-tracel-{image}-{timestamp}-{git_sha}",
        env.short()
    ))
}

fn git_short_sha() -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("git");
    cmd.args(["rev-parse", "--short=12", "HEAD"]);

    let out = run_process_capture_stdout(&mut cmd, "git rev-parse should succeed")?;
    Ok(out.trim().to_string())
}

fn utc_timestamp_compact() -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("date");
    cmd.args(["-u", "+%Y%m%d%H%M%S"]);

    let out = run_process_capture_stdout(&mut cmd, "date should be executable")?;
    Ok(out.trim().to_string())
}

fn parse_tags(tags: &[String]) -> anyhow::Result<Vec<(String, String)>> {
    tags.iter()
        .map(|tag| {
            let (key, value) = tag.split_once('=').ok_or_else(|| {
                anyhow::anyhow!("AMI tag should use KEY=VALUE format, got '{tag}'")
            })?;

            let key = key.trim();
            let value = value.trim();

            if key.is_empty() {
                anyhow::bail!("AMI tag key should not be empty in '{tag}'");
            }

            if value.is_empty() {
                anyhow::bail!("AMI tag value should not be empty in '{tag}'");
            }

            Ok((key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn build_image_lifecycle(
    region: &str,
    image: &str,
    env: &Environment<ExplicitIndex>,
    stop_timeout: Duration,
    ami_timeout: Duration,
    no_reboot: bool,
    tags: &[(String, String)],
    event_tx: &mpsc::Sender<ImageBuildEvent>,
) -> anyhow::Result<BuiltImage> {
    send_image_build_state(event_tx, image, ImageBuildState::DiscoveringBaker);

    let result = (|| {
        let instance = find_baker_instance(region, image).with_context(|| {
            format!("baker instance for image '{image}' should be discoverable")
        })?;

        wait_for_instance_stopped_with_status(
            region,
            image,
            &instance.instance_id,
            stop_timeout,
            event_tx,
        )
        .with_context(|| {
            format!(
                "baker instance '{}' for image '{}' should stop",
                instance.instance_id, image
            )
        })?;

        let ami_name = build_ami_name(env, image)?;

        send_image_build_state(
            event_tx,
            image,
            ImageBuildState::CreatingImage {
                instance_id: instance.instance_id.clone(),
                ami_name: ami_name.clone(),
                state: None,
            },
        );

        let ami_id = create_image(
            region,
            &instance.instance_id,
            &ami_name,
            image,
            no_reboot,
            tags,
        )
        .with_context(|| {
            format!(
                "AMI for image '{image}' should be created from instance '{}'",
                instance.instance_id
            )
        })?;

        wait_for_image_available_with_status(
            region,
            image,
            &instance.instance_id,
            &ami_id,
            &ami_name,
            ami_timeout,
            event_tx,
        )
        .with_context(|| format!("AMI '{ami_id}' for image '{image}' should become available"))?;

        let built = BuiltImage {
            image: image.to_owned(),
            ami_id,
            ami_name,
        };

        send_image_build_state(
            event_tx,
            image,
            ImageBuildState::Complete {
                ami_id: built.ami_id.clone(),
                ami_name: built.ami_name.clone(),
            },
        );

        Ok(built)
    })();

    if let Err(err) = &result {
        send_image_build_state(
            event_tx,
            image,
            ImageBuildState::Failed {
                error: format!("{err:#}"),
            },
        );
    }

    result
}

fn wait_for_instance_stopped_with_status(
    region: &str,
    image: &str,
    instance_id: &str,
    timeout: Duration,
    event_tx: &mpsc::Sender<ImageBuildEvent>,
) -> anyhow::Result<()> {
    let poll = Duration::from_secs(POLLING_INTERVAL_SECS);
    let start = Instant::now();

    loop {
        let state = tracel_xtask_utils::aws::images::instance_state(region, instance_id)?;

        send_image_build_state(
            event_tx,
            image,
            ImageBuildState::Baking {
                instance_id: instance_id.to_owned(),
                state: state.clone(),
            },
        );

        if state == "stopped" {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            anyhow::bail!(
                "Timed out after {} seconds while waiting for instance '{}' to stop",
                timeout.as_secs(),
                instance_id
            );
        }

        std::thread::sleep(poll);
    }
}

fn wait_for_image_available_with_status(
    region: &str,
    image: &str,
    instance_id: &str,
    ami_id: &str,
    ami_name: &str,
    timeout: Duration,
    event_tx: &mpsc::Sender<ImageBuildEvent>,
) -> anyhow::Result<()> {
    let poll = Duration::from_secs(10);
    let start = Instant::now();

    loop {
        let state = tracel_xtask_utils::aws::images::image_state(region, ami_id)?
            .unwrap_or_else(|| "unknown".to_owned());

        send_image_build_state(
            event_tx,
            image,
            ImageBuildState::CreatingImage {
                instance_id: instance_id.to_owned(),
                ami_name: ami_name.to_owned(),
                state: Some(state.clone()),
            },
        );

        if state == "available" {
            return Ok(());
        }

        if state == "failed" {
            anyhow::bail!("AMI '{ami_id}' entered failed state");
        }

        if start.elapsed() >= timeout {
            anyhow::bail!(
                "Timed out after {} seconds while waiting for AMI '{}' to become available",
                timeout.as_secs(),
                ami_id
            );
        }

        std::thread::sleep(poll);
    }
}

fn send_image_build_state(
    event_tx: &mpsc::Sender<ImageBuildEvent>,
    image: &str,
    state: ImageBuildState,
) {
    let _ = event_tx.send(ImageBuildEvent::State {
        image: image.to_owned(),
        state,
    });
}

fn thread_panic_error(payload: Box<dyn std::any::Any + Send + 'static>) -> anyhow::Error {
    if let Some(message) = payload.downcast_ref::<&str>() {
        anyhow::anyhow!("image build worker thread panicked: {message}")
    } else if let Some(message) = payload.downcast_ref::<String>() {
        anyhow::anyhow!("image build worker thread panicked: {message}")
    } else {
        anyhow::anyhow!("image build worker thread panicked with non-string payload")
    }
}
