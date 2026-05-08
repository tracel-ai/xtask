/// Manage AWS virtual machine images.
/// Current implementation uses Terraform-managed baker EC2 instances and AWS AMIs.
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::prelude::anyhow::Context as _;
use crate::prelude::*;
use tracel_xtask_utils::{
    aws::{
        images::{
            create_image, create_true_tag, delete_tag, ensure_image_matches_name,
            find_baker_instance, find_single_image_by_true_tag, get_image_by_id,
            print_image_summary, wait_for_image_available, wait_for_instance_stopped,
        },
        instance_system_log::stream_system_log,
    },
    git::git_repo_root_or_cwd,
    process::{run_process, run_process_capture_stdout},
};

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
    #[arg(long, default_value_t = 3600)]
    pub stop_timeout_secs: u64,
    /// Timeout while waiting for AMIs to become available.
    #[arg(long, default_value_t = 1800)]
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

pub fn handle_command(args: ImageCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    match args.get_command() {
        ImageSubCommand::Build(build_args) => build(build_args),
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

fn build(args: ImageBuildSubCmdArgs) -> anyhow::Result<()> {
    if args.images.is_empty() {
        anyhow::bail!("at least one '--image <IMAGE_NAME>' should be provided");
    }

    let tf_root = normalize_path(args.tf_root)?;

    if !args.skip_apply {
        eprintln!("🏗️ Applying image baker Terraform state");
        terraform_apply(&tf_root, &args.tf_arg)
            .context("image baker Terraform state should apply successfully")?;
    }

    let mut created_images = Vec::new();
    let tags = parse_tags(&args.tags)?;

    for image in &args.images {
        eprintln!("👩‍🍳 Looking for baker instance for image '{image}'");

        let instance = find_baker_instance(&args.region, image).with_context(|| {
            format!("baker instance for image '{image}' should be discoverable")
        })?;

        eprintln!(
            "👩‍🍳 Found baker instance for image '{image}'\n  Instance: {}\n  State:    {}\n  IP:       {}\n  AZ:       {}",
            instance.instance_id,
            instance.state.name,
            instance.private_ip.as_deref().unwrap_or("no-ip"),
            instance.placement.availability_zone,
        );

        wait_for_instance_stopped(
            &args.region,
            &instance.instance_id,
            Duration::from_secs(args.stop_timeout_secs),
        )
        .with_context(|| {
            format!(
                "baker instance '{}' for image '{}' should stop",
                instance.instance_id, image
            )
        })?;

        let ami_name = build_ami_name(image)?;
        let ami_id = create_image(
            &args.region,
            &instance.instance_id,
            &ami_name,
            image,
            args.no_reboot,
            &tags,
        )
        .with_context(|| {
            format!(
                "AMI for image '{image}' should be created from instance '{}'",
                instance.instance_id
            )
        })?;

        wait_for_image_available(
            &args.region,
            &ami_id,
            Duration::from_secs(args.ami_timeout_secs),
        )
        .with_context(|| format!("AMI '{ami_id}' for image '{image}' should become available"))?;

        eprintln!("✅ Built AMI for image '{image}'");
        eprintln!("  AMI:  {ami_id}");
        eprintln!("  Name: {ami_name}");

        created_images.push((image.clone(), ami_id, ami_name));
    }

    eprintln!("🎉 Image build completed");
    for (image, ami_id, ami_name) in created_images {
        eprintln!("• {image}: {ami_id} ({ami_name})");
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
    eprintln!("  ImageName={}", args.image);

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

fn build_ami_name(image: &str) -> anyhow::Result<String> {
    let git_sha = git_short_sha().unwrap_or_else(|_| "unknown".to_string());
    let timestamp = utc_timestamp_compact()?;

    Ok(format!("tracel-{image}-{timestamp}-{git_sha}"))
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
