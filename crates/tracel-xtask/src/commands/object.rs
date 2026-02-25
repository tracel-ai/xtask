use std::path::PathBuf;

use anyhow::Context as _;

use crate::prelude::*;
use crate::utils::aws::cli::{
    aws_cli_quiet, ecr_get_commit_sha_tag_from_alias_tag, s3_console_url, s3_copy_object, s3_cp_file_to_s3, s3_head_object, s3_put_object_tags, s3_url
};
use crate::utils::git::git_repo_root_or_cwd;
use crate::utils::process::run_process;

#[tracel_xtask_macros::declare_command_args(None, ObjectSubCommand)]
pub struct ObjectCmdArgs {}

impl Default for ObjectSubCommand {
    fn default() -> Self {
        ObjectSubCommand::Build(ObjectBuildSubCmdArgs::default())
    }
}

pub fn handle_command(args: ObjectCmdArgs, env: Environment, _ctx: Context) -> anyhow::Result<()> {
    match args.get_command() {
        ObjectSubCommand::Build(a) => build(a),
        ObjectSubCommand::List(a) => list(a, &env),
        ObjectSubCommand::Promote(a) => promote(a, &env),
        ObjectSubCommand::Push(a) => push(a),
        ObjectSubCommand::Rollout(a) => rollout(a, &env),
        ObjectSubCommand::Rollback(a) => rollback(a, &env),
    }
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectBuildSubCmdArgs {
    /// Cargo package/crate name (workspace member).
    #[arg(long)]
    pub crate_name: String,

    /// Optional bin target name (if crate has multiple bins).
    #[arg(long)]
    pub bin: Option<String>,

    /// Build profile (default: release).
    #[arg(long, default_value = "release")]
    pub profile: String,

    /// Workspace root (defaults to git repo root / cwd).
    #[arg(long)]
    pub workspace_dir: Option<PathBuf>,

    /// Output directory where we will copy the built artifact for convenience (optional).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectListSubCmdArgs {
    #[arg(long)]
    pub bucket: String,
    #[arg(long, default_value = "objects")]
    pub prefix: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub region: String,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectPromoteSubCmdArgs {
    /// S3 bucket name.
    #[arg(long)]
    pub bucket: String,

    /// S3 key prefix.
    #[arg(long, default_value = "objects")]
    pub prefix: String,

    /// Object logical name.
    #[arg(long)]
    pub name: String,

    /// Immutable build id to promote (usually commit SHA).
    #[arg(long)]
    pub build_id: String,

    /// AWS region.
    #[arg(long)]
    pub region: String,

    /// Container repository to bind to (ECR repo name).
    #[arg(long)]
    pub container_repository: String,

    /// Container "latest" tag name (defaults to env name if omitted).
    #[arg(long)]
    pub container_latest_tag: Option<String>,

    /// Optional: explicitly override the container commit tag instead of resolving from ECR alias.
    /// If not set, we resolve the commit tag behind `container_latest_tag`.
    #[arg(long)]
    pub container_commit_tag: Option<String>,

    /// Add env tag to object tags.
    #[arg(long, default_value_t = true)]
    pub tag_environment: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectPushSubCmdArgs {
    /// Local file to upload (typically the built binary).
    #[arg(long)]
    pub file: PathBuf,

    /// S3 bucket name.
    #[arg(long)]
    pub bucket: String,

    /// S3 key prefix (e.g. "burn-central/objects").
    #[arg(long, default_value = "objects")]
    pub prefix: String,

    /// Object logical name (e.g. "compute-provider" or "bc-backend-migrator").
    #[arg(long)]
    pub name: String,

    /// Immutable build id (usually commit SHA).
    #[arg(long)]
    pub build_id: String,

    /// AWS region (used for console URLs / consistent behavior).
    #[arg(long)]
    pub region: String,

    /// If set, overwrite the same key if it exists.
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectRolloutSubCmdArgs {
    /// S3 bucket name.
    #[arg(long)]
    pub bucket: String,

    /// S3 key prefix.
    #[arg(long, default_value = "objects")]
    pub prefix: String,

    /// Object logical name (base name without suffix).
    #[arg(long)]
    pub name: String,

    /// Immutable build id to roll out.
    #[arg(long)]
    pub build_id: String,

    /// AWS region.
    #[arg(long)]
    pub region: String,

    /// Container repository to bind to (ECR repo name).
    #[arg(long)]
    pub container_repository: String,

    /// Container "latest" tag name (defaults to env name if omitted).
    #[arg(long)]
    pub container_latest_tag: Option<String>,

    /// Optional: explicitly override the container commit tag instead of resolving from ECR alias.
    #[arg(long)]
    pub container_commit_tag: Option<String>,

    /// If set, always copy even if `.latest` already equals the source ETag (extra HEADs needed).
    #[arg(long, default_value_t = false)]
    pub force: bool,
}

#[derive(clap::Args, Default, Clone, PartialEq)]
pub struct ObjectRollbackSubCmdArgs {
    /// S3 bucket name.
    #[arg(long)]
    pub bucket: String,

    /// S3 key prefix.
    #[arg(long, default_value = "objects")]
    pub prefix: String,

    /// Object logical name (base name without suffix).
    #[arg(long)]
    pub name: String,

    /// AWS region.
    #[arg(long)]
    pub region: String,

    /// If set, also update `.rollback` to the previous `.latest` after the rollback.
    /// This makes rollback "swap" latest/rollback (handy for flip-flopping).
    #[arg(long, default_value_t = false)]
    pub swap: bool,

    /// Container repository to bind to (ECR repo name).
    /// Optional: if set, we will (re)apply binding tags on `.latest`.
    #[arg(long)]
    pub container_repository: Option<String>,

    /// Container "latest" tag name (defaults to env name if omitted).
    #[arg(long)]
    pub container_latest_tag: Option<String>,

    /// Optional: explicitly override the container commit tag instead of resolving from ECR alias.
    #[arg(long)]
    pub container_commit_tag: Option<String>,
}

fn build(args: ObjectBuildSubCmdArgs) -> anyhow::Result<()> {
    let workspace = args.workspace_dir.unwrap_or(git_repo_root_or_cwd()?);

    let mut cargo_args: Vec<String> = vec!["build".into()];
    if args.profile == "release" {
        cargo_args.push("--release".into());
    } else {
        cargo_args.push(format!("--profile={}", args.profile));
    }
    cargo_args.push("-p".into());
    cargo_args.push(args.crate_name.clone());
    if let Some(bin) = &args.bin {
        cargo_args.push("--bin".into());
        cargo_args.push(bin.clone());
    }

    let arg_refs: Vec<&str> = cargo_args.iter().map(|s| s.as_str()).collect();
    run_process(
        "cargo",
        &arg_refs,
        None,
        Some(&workspace),
        "cargo build should succeed",
    )?;

    // Infer output path (linux/mac). If you also target windows, you can add `.exe` handling here.
    let bin_name = args.bin.clone().unwrap_or_else(|| args.crate_name.clone());
    let target_dir = workspace.join("target").join(&args.profile);
    let built_path = target_dir.join(&bin_name);

    anyhow::ensure!(
        built_path.exists(),
        "built artifact should exist at '{}'",
        built_path.display()
    );

    if let Some(out_dir) = args.out_dir {
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("out_dir '{}' should be creatable", out_dir.display()))?;
        let dst = out_dir.join(&bin_name);
        std::fs::copy(&built_path, &dst).with_context(|| {
            format!(
                "built artifact should be copyable from '{}' to '{}'",
                built_path.display(),
                dst.display()
            )
        })?;
        eprintln!("📦 Built: {}", dst.display());
    } else {
        eprintln!("📦 Built: {}", built_path.display());
    }

    Ok(())
}

fn list(args: ObjectListSubCmdArgs, _env: &Environment) -> anyhow::Result<()> {
    let latest_key = latest_object_key(&args.prefix, &args.name);
    let rollback_key = rollback_object_key(&args.prefix, &args.name);

    eprintln!("📚 Object set: {}/{}", args.bucket, args.prefix);
    eprintln!("• latest:   {}", s3_url(&args.bucket, &latest_key));
    match s3_head_object(&args.bucket, &latest_key, &args.region)? {
        Some(etag) => eprintln!("  ✅ present (etag {etag})"),
        None => eprintln!("  ❌ absent"),
    }

    eprintln!("• rollback: {}", s3_url(&args.bucket, &rollback_key));
    match s3_head_object(&args.bucket, &rollback_key, &args.region)? {
        Some(etag) => eprintln!("  ✅ present (etag {etag})"),
        None => eprintln!("  ❌ absent"),
    }

    Ok(())
}

fn promote(args: ObjectPromoteSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let key = build_object_key(&args.prefix, &args.name, &args.build_id);
    anyhow::ensure!(
        s3_head_object(&args.bucket, &key, &args.region)?.is_some(),
        "object to promote should exist: {}",
        s3_url(&args.bucket, &key)
    );

    let latest_tag = container_latest_tag(args.container_latest_tag, env);

    let commit_tag = match args.container_commit_tag {
        Some(t) => t,
        None => {
            ecr_get_commit_sha_tag_from_alias_tag(&args.container_repository, &latest_tag, &args.region)?
                .ok_or_else(|| anyhow::anyhow!(
                    "container commit tag should be resolvable from ECR for repo '{}' and alias tag '{}'",
                    args.container_repository,
                    latest_tag
                ))?
        }
    };

    // Apply binding tags to the immutable build object.
    let mut tags = vec![
        ("tracel:container_repo", args.container_repository.as_str()),
        ("tracel:container_latest_tag", latest_tag.as_str()),
        ("tracel:container_commit_tag", commit_tag.as_str()),
    ];
    let env_s;
    if args.tag_environment {
        env_s = env.to_string();
        tags.push(("tracel:environment", env_s.as_str()));
    }

    s3_put_object_tags(&args.bucket, &key, &args.region, &tags)?;
    eprintln!("🏷️  Promoted (tagged) object:");
    eprintln!("• Object:   {}", s3_url(&args.bucket, &key));
    eprintln!(
        "• Bound to: {}:{} (alias '{}')",
        args.container_repository, commit_tag, latest_tag
    );
    eprintln!(
        "🌐 Console: {}",
        s3_console_url(&args.region, &args.bucket, &key)
    );
    Ok(())
}

fn push(args: ObjectPushSubCmdArgs) -> anyhow::Result<()> {
    anyhow::ensure!(
        args.file.exists(),
        "file should exist: {}",
        args.file.display()
    );

    let key = build_object_key(&args.prefix, &args.name, &args.build_id);
    let dst = s3_url(&args.bucket, &key);

    // Optional: existence check (skip if present unless --force)
    if !args.force {
        if s3_head_object(&args.bucket, &key, &args.region)?.is_some() {
            eprintln!("✅ Already present in S3: {dst} (skipping; use --force to overwrite)");
            eprintln!(
                "🌐 Console: {}",
                s3_console_url(&args.region, &args.bucket, &key)
            );
            return Ok(());
        }
    }

    s3_cp_file_to_s3(&args.file, &args.bucket, &key, &args.region)?;
    eprintln!("📤 Uploaded: {}", args.file.display());
    eprintln!("🗄️  S3: {dst}");
    eprintln!(
        "🌐 Console: {}",
        s3_console_url(&args.region, &args.bucket, &key)
    );
    Ok(())
}

fn rollout(args: ObjectRolloutSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let src_key = build_object_key(&args.prefix, &args.name, &args.build_id);
    anyhow::ensure!(
        s3_head_object(&args.bucket, &src_key, &args.region)?.is_some(),
        "source build object should exist: {}",
        s3_url(&args.bucket, &src_key)
    );

    let latest_key = latest_object_key(&args.prefix, &args.name);
    let rollback_key = rollback_object_key(&args.prefix, &args.name);

    // If .latest exists, back it up to .rollback
    if s3_head_object(&args.bucket, &latest_key, &args.region)?.is_some() {
        s3_copy_object(
            &args.bucket,
            &latest_key,
            &args.bucket,
            &rollback_key,
            &args.region,
        )?;
        eprintln!("↩️  Backed up .latest → .rollback");
        eprintln!("• {}", s3_url(&args.bucket, &rollback_key));
    }

    // Replace .latest with source
    s3_copy_object(
        &args.bucket,
        &src_key,
        &args.bucket,
        &latest_key,
        &args.region,
    )?;
    eprintln!("🚀 Rolled out .latest");
    eprintln!("• {}", s3_url(&args.bucket, &latest_key));

    // Apply the same binding tags on the permalink object too (helps ops quickly inspect “what is live”)
    let latest_tag = container_latest_tag(args.container_latest_tag, env);
    let commit_tag = match args.container_commit_tag {
        Some(t) => t,
        None => {
            ecr_get_commit_sha_tag_from_alias_tag(&args.container_repository, &latest_tag, &args.region)?
                .ok_or_else(|| anyhow::anyhow!(
                    "container commit tag should be resolvable from ECR for repo '{}' and alias tag '{}'",
                    args.container_repository,
                    latest_tag
                ))?
        }
    };

    let env_s;
    let mut tags = vec![
        ("tracel:container_repo", args.container_repository.as_str()),
        ("tracel:container_latest_tag", latest_tag.as_str()),
        ("tracel:container_commit_tag", commit_tag.as_str()),
        ("tracel:source_build_id", args.build_id.as_str()),
    ];
    env_s = env.to_string();
    tags.push(("tracel:environment", env_s.as_str()));

    s3_put_object_tags(&args.bucket, &latest_key, &args.region, &tags)?;
    eprintln!("🏷️  Tagged .latest with container binding");
    eprintln!(
        "🌐 Console: {}",
        s3_console_url(&args.region, &args.bucket, &latest_key)
    );
    Ok(())
}

fn rollback(args: ObjectRollbackSubCmdArgs, env: &Environment) -> anyhow::Result<()> {
    let latest_key = latest_object_key(&args.prefix, &args.name);
    let rollback_key = rollback_object_key(&args.prefix, &args.name);

    anyhow::ensure!(
        s3_head_object(&args.bucket, &rollback_key, &args.region)?.is_some(),
        "rollback object should exist: {}",
        s3_url(&args.bucket, &rollback_key)
    );

    // If swap is enabled and .latest exists, capture it first.
    let latest_etag = s3_head_object(&args.bucket, &latest_key, &args.region)?;

    if args.swap {
        if latest_etag.is_some() {
            // latest -> temp (we reuse .rollback as the destination after copying rollback->latest)
            // We'll do: latest -> temp_key, rollback -> latest, temp -> rollback
            let temp_key = format!("{}/{}.rollback.swap.tmp", args.prefix, args.name);

            s3_copy_object(
                &args.bucket,
                &latest_key,
                &args.bucket,
                &temp_key,
                &args.region,
            )?;

            s3_copy_object(
                &args.bucket,
                &rollback_key,
                &args.bucket,
                &latest_key,
                &args.region,
            )?;

            s3_copy_object(
                &args.bucket,
                &temp_key,
                &args.bucket,
                &rollback_key,
                &args.region,
            )?;

            // best-effort cleanup of temp
            let _ = aws_cli_quiet(
                vec![
                    "s3api".into(),
                    "delete-object".into(),
                    "--bucket".into(),
                    args.bucket.clone(),
                    "--key".into(),
                    temp_key.clone(),
                    "--region".into(),
                    args.region.clone(),
                ],
                None,
                None,
                "aws s3api delete-object should succeed",
            );

            eprintln!("⏪ Rolled back (swap enabled):");
        } else {
            // no latest: just restore rollback -> latest
            s3_copy_object(
                &args.bucket,
                &rollback_key,
                &args.bucket,
                &latest_key,
                &args.region,
            )?;
            eprintln!("⏪ Rolled back (.latest was absent):");
        }
    } else {
        // Normal rollback: rollback -> latest (and keep rollback intact)
        s3_copy_object(
            &args.bucket,
            &rollback_key,
            &args.bucket,
            &latest_key,
            &args.region,
        )?;
        eprintln!("⏪ Rolled back:");
    }

    eprintln!("• latest:   {}", s3_url(&args.bucket, &latest_key));
    eprintln!("• rollback: {}", s3_url(&args.bucket, &rollback_key));
    eprintln!(
        "🌐 Console: {}",
        s3_console_url(&args.region, &args.bucket, &latest_key)
    );

    // Optional: (re)apply container binding tags to `.latest` after rollback.
    // This keeps ops visibility consistent (what is live / what is it bound to).
    if let Some(repo) = args.container_repository.as_deref() {
        let latest_tag = container_latest_tag(args.container_latest_tag.clone(), env);

        let commit_tag = match args.container_commit_tag.clone() {
            Some(t) => t,
            None => {
                ecr_get_commit_sha_tag_from_alias_tag(repo, &latest_tag, &args.region)?
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "container commit tag should be resolvable from ECR for repo '{}' and alias tag '{}'",
                            repo,
                            latest_tag
                        )
                    })?
            }
        };

        let env_s = env.to_string();
        let tags = vec![
            ("tracel:container_repo", repo),
            ("tracel:container_latest_tag", latest_tag.as_str()),
            ("tracel:container_commit_tag", commit_tag.as_str()),
            ("tracel:environment", env_s.as_str()),
            ("tracel:source", "rollback"),
        ];

        s3_put_object_tags(&args.bucket, &latest_key, &args.region, &tags)?;
        eprintln!("🏷️  Updated .latest binding tags after rollback");
    }

    Ok(())
}

fn container_latest_tag(tag: Option<String>, env: &Environment) -> String {
    tag.unwrap_or(env.to_string())
}

fn build_object_key(prefix: &str, name: &str, build_id: &str) -> String {
    // immutable build object
    format!("{prefix}/{name}/{build_id}/{name}")
}

fn latest_object_key(prefix: &str, name: &str) -> String {
    // permalink
    format!("{prefix}/{name}/{name}.latest")
}

fn rollback_object_key(prefix: &str, name: &str) -> String {
    // permalink rollback
    format!("{prefix}/{name}/{name}.rollback")
}
