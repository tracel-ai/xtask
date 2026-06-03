use std::{
    ffi::OsString,
    io::Write as _,
    path::{Path, PathBuf},
    process::Stdio,
};

use anyhow::Context as _;
use serde::Deserialize;

use crate::{group_info, process::run_process_capture_stdout};

fn gcloud_program() -> OsString {
    if let Some(program) = std::env::var_os("GCLOUD") {
        if !program.is_empty() {
            return program;
        }
    }

    if let Some(program) = default_sdk_gcloud_path() {
        return program.into_os_string();
    }

    "gcloud".into()
}

fn default_sdk_gcloud_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join("google-cloud-sdk/bin/gcloud");

    path.exists().then_some(path)
}

pub fn gcloud_cli(
    args: Vec<String>,
    envs: Option<std::collections::HashMap<&str, &str>>,
    path: Option<&Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let joined_args = args.join(" ");
    group_info!("Command line: gcloud {}", &joined_args);

    let mut command = std::process::Command::new(gcloud_program());

    if let Some(path) = path {
        command.current_dir(path);
    }

    if let Some(envs) = envs {
        command.envs(&envs);
    }

    let status = command
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to execute gcloud {}: {}", joined_args, e))?;

    if !status.success() {
        anyhow::bail!("{error_msg} ({status})");
    }

    Ok(())
}

pub fn gcloud_capture_stdout(args: Vec<String>, error_msg: &str) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new(gcloud_program());
    cmd.args(args);
    run_process_capture_stdout(&mut cmd, error_msg)
}

fn gcloud_output_quiet(args: &[&str], context: &str) -> anyhow::Result<std::process::Output> {
    std::process::Command::new(gcloud_program())
        .args(args)
        .arg("--quiet")
        .output()
        .with_context(|| context.to_string())
}

fn gcloud_missing_resource(stderr: &[u8]) -> bool {
    let stderr = String::from_utf8_lossy(stderr);

    stderr.contains("Image not found")
        || stderr.contains("Tag not found")
        || stderr.contains("NOT_FOUND")
        || stderr.contains("not found")
}

fn gcloud_stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

// Artifact Registry ---------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ArtifactRegistryImage {
    pub project: String,
    pub location: String,
    pub repository: String,
    pub image: String,
}

impl ArtifactRegistryImage {
    pub fn new(
        project: impl Into<String>,
        location: impl Into<String>,
        repository: impl Into<String>,
        image: impl Into<String>,
    ) -> Self {
        Self {
            project: project.into(),
            location: location.into(),
            repository: repository.into(),
            image: image.into(),
        }
    }

    pub fn registry_host(&self) -> String {
        format!("{}-docker.pkg.dev", self.location)
    }

    pub fn image_ref(&self) -> String {
        format!(
            "{}/{}/{}/{}",
            self.registry_host(),
            self.project,
            self.repository,
            self.image,
        )
    }

    pub fn tagged_ref(&self, tag: &str) -> String {
        format!("{}:{tag}", self.image_ref())
    }

    pub fn console_url(&self, tag: Option<&str>) -> String {
        // This URL shape is stable enough for a helpful output link. The CLI operations
        // remain the source of truth.
        let mut url = format!(
            "https://console.cloud.google.com/artifacts/docker/{project}/{location}/{repository}/{image}",
            project = self.project,
            location = self.location,
            repository = self.repository,
            image = self.image,
        );

        if let Some(tag) = tag {
            url.push_str(&format!("?project={}&tag={}", self.project, tag));
        } else {
            url.push_str(&format!("?project={}", self.project));
        }

        url
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactRegistryDockerTag {
    name: String,
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactRegistryDockerImage {
    tags: Option<Vec<String>>,
}

pub fn artifact_registry_configure_docker(location: &str) -> anyhow::Result<()> {
    let host = format!("{location}-docker.pkg.dev");

    gcloud_cli(
        vec![
            "auth".into(),
            "configure-docker".into(),
            host,
            "--quiet".into(),
        ],
        None,
        None,
        "gcloud auth configure-docker should succeed",
    )
}

pub fn artifact_registry_ensure_repository_exists(
    project: &str,
    location: &str,
    repository: &str,
) -> anyhow::Result<()> {
    gcloud_cli(
        vec![
            "artifacts".into(),
            "repositories".into(),
            "describe".into(),
            repository.into(),
            "--project".into(),
            project.into(),
            "--location".into(),
            location.into(),
            "--format".into(),
            "value(name)".into(),
        ],
        None,
        None,
        "Artifact Registry repository should exist",
    )
}

pub fn artifact_registry_image_exists(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<bool> {
    let tagged_ref = image.tagged_ref(tag);

    let output = gcloud_output_quiet(
        &[
            "artifacts",
            "docker",
            "images",
            "describe",
            tagged_ref.as_str(),
        ],
        "gcloud Artifact Registry image describe should start",
    )?;

    if output.status.success() {
        return Ok(true);
    }

    if gcloud_missing_resource(&output.stderr) {
        return Ok(false);
    }

    anyhow::bail!(
        "gcloud Artifact Registry image describe failed for '{}':\n{}",
        tagged_ref,
        gcloud_stderr(&output),
    )
}

pub fn artifact_registry_tag_exists(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<bool> {
    artifact_registry_get_digest_from_tag(image, tag).map(|digest| digest.is_some())
}

pub fn artifact_registry_get_tag_version(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<Option<String>> {
    let stdout = gcloud_capture_stdout(
        vec![
            "artifacts".into(),
            "docker".into(),
            "tags".into(),
            "list".into(),
            image.image_ref(),
            "--project".into(),
            image.project.clone(),
            "--filter".into(),
            format!("tag:{tag}"),
            "--format".into(),
            "json".into(),
        ],
        "Artifact Registry docker tags list should succeed",
    )?;

    let tags: Vec<ArtifactRegistryDockerTag> =
        serde_json::from_str(&stdout).context("Artifact Registry tags JSON should parse")?;

    let version = tags
        .into_iter()
        .find(|t| t.name.ends_with(&format!("/tags/{tag}")) || t.name.ends_with(&format!(":{tag}")))
        .and_then(|t| t.version);

    Ok(version)
}

pub fn artifact_registry_get_digest_from_tag(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<Option<String>> {
    let tagged_ref = image.tagged_ref(tag);

    let output = gcloud_output_quiet(
        &[
            "artifacts",
            "docker",
            "images",
            "describe",
            tagged_ref.as_str(),
            "--format=value(image_summary.digest)",
        ],
        "gcloud Artifact Registry image digest describe should start",
    )?;

    if output.status.success() {
        let digest = String::from_utf8_lossy(&output.stdout).trim().to_owned();

        if digest.is_empty() {
            return Ok(None);
        }

        return Ok(Some(digest));
    }

    if gcloud_missing_resource(&output.stderr) {
        return Ok(None);
    }

    anyhow::bail!(
        "gcloud Artifact Registry image digest describe failed for '{}':\n{}",
        tagged_ref,
        gcloud_stderr(&output),
    )
}

pub fn artifact_registry_add_tag(
    image: &ArtifactRegistryImage,
    source_tag: &str,
    target_tag: &str,
) -> anyhow::Result<()> {
    let source = image.tagged_ref(source_tag);
    let target = image.tagged_ref(target_tag);

    gcloud_cli(
        vec![
            "artifacts".into(),
            "docker".into(),
            "tags".into(),
            "add".into(),
            source,
            target,
            "--project".into(),
            image.project.clone(),
            "--quiet".into(),
        ],
        None,
        None,
        "Artifact Registry docker tag add should succeed",
    )
}

pub fn artifact_registry_delete_tag(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<()> {
    let target = image.tagged_ref(tag);

    gcloud_cli(
        vec![
            "artifacts".into(),
            "docker".into(),
            "tags".into(),
            "delete".into(),
            target,
            "--project".into(),
            image.project.clone(),
            "--quiet".into(),
        ],
        None,
        None,
        "Artifact Registry docker tag delete should succeed",
    )
}

pub fn artifact_registry_list_images_with_tags(
    image: &ArtifactRegistryImage,
) -> anyhow::Result<Vec<String>> {
    let stdout = gcloud_capture_stdout(
        vec![
            "artifacts".into(),
            "docker".into(),
            "images".into(),
            "list".into(),
            image.image_ref(),
            "--include-tags".into(),
            "--project".into(),
            image.project.clone(),
            "--format".into(),
            "json".into(),
        ],
        "Artifact Registry docker images list should succeed",
    )?;

    let images: Vec<ArtifactRegistryDockerImage> =
        serde_json::from_str(&stdout).context("Artifact Registry images JSON should parse")?;

    let mut tags = Vec::new();

    for item in images {
        if let Some(item_tags) = item.tags {
            tags.extend(item_tags);
        }
    }

    tags.sort();
    tags.dedup();

    Ok(tags)
}

pub fn artifact_registry_get_last_pushed_commit_sha_tag(
    image: &ArtifactRegistryImage,
) -> anyhow::Result<Option<String>> {
    let tags = artifact_registry_list_images_with_tags(image)?;

    Ok(tags
        .into_iter()
        .filter(|tag| is_probable_git_sha(tag))
        .max())
}

pub fn artifact_registry_compute_next_numeric_tag(
    image: &ArtifactRegistryImage,
) -> anyhow::Result<u64> {
    let tags = artifact_registry_list_images_with_tags(image)?;

    let max = tags
        .iter()
        .filter_map(|tag| tag.parse::<u64>().ok())
        .max()
        .unwrap_or(0);

    Ok(max + 1)
}

fn is_probable_git_sha(tag: &str) -> bool {
    let len = tag.len();

    (7..=40).contains(&len) && tag.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn artifact_registry_image_url(
    image: &ArtifactRegistryImage,
    tag: &str,
) -> anyhow::Result<String> {
    Ok(image.console_url(Some(tag)))
}

// Managed Instance Group ----------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GcpMigRolloutAction {
    Replace,
    Restart,
}

impl std::fmt::Display for GcpMigRolloutAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Replace => write!(f, "replace"),
            Self::Restart => write!(f, "restart"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedInstanceGroupDescription {
    status: Option<ManagedInstanceGroupStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedInstanceGroupStatus {
    is_stable: Option<bool>,
}

pub fn mig_start_rolling_action(
    project: &str,
    region: &str,
    mig: &str,
    action: GcpMigRolloutAction,
    max_surge: &str,
    max_unavailable: &str,
) -> anyhow::Result<()> {
    let action = action.to_string();

    gcloud_cli(
        vec![
            "compute".into(),
            "instance-groups".into(),
            "managed".into(),
            "rolling-action".into(),
            action,
            mig.into(),
            "--project".into(),
            project.into(),
            "--region".into(),
            region.into(),
            format!("--max-surge={max_surge}"),
            format!("--max-unavailable={max_unavailable}"),
            "--quiet".into(),
        ],
        None,
        None,
        "GCP MIG rolling action should start",
    )
}

pub fn mig_is_stable(project: &str, region: &str, mig: &str) -> anyhow::Result<Option<bool>> {
    let stdout = gcloud_capture_stdout(
        vec![
            "compute".into(),
            "instance-groups".into(),
            "managed".into(),
            "describe".into(),
            mig.into(),
            "--project".into(),
            project.into(),
            "--region".into(),
            region.into(),
            "--format".into(),
            "json".into(),
        ],
        "GCP MIG describe should succeed",
    )?;

    let description: ManagedInstanceGroupDescription =
        serde_json::from_str(&stdout).context("GCP MIG describe JSON should parse")?;

    Ok(description.status.and_then(|s| s.is_stable))
}

pub fn mig_wait_until_stable(project: &str, region: &str, mig: &str) -> anyhow::Result<()> {
    gcloud_cli(
        vec![
            "compute".into(),
            "instance-groups".into(),
            "managed".into(),
            "wait-until".into(),
            mig.into(),
            "--project".into(),
            project.into(),
            "--region".into(),
            region.into(),
            "--stable".into(),
        ],
        None,
        None,
        "GCP MIG should become stable",
    )
}

pub fn mig_console_url(project: &str, region: &str, mig: &str) -> String {
    format!(
        "https://console.cloud.google.com/compute/instanceGroups/details/{region}/{mig}?project={project}"
    )
}

// Secret Manager ------------------------------------------------------------

fn push_secret_location_args(args: &mut Vec<String>, location: Option<&str>) {
    if let Some(location) = location {
        args.push("--location".into());
        args.push(location.into());
    }
}

/// Create a Secret Manager secret metadata resource.
///
/// If `replication_locations` is provided, user-managed replication is used.
/// Otherwise automatic replication is used.
pub fn secret_manager_create_secret(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
    replication_locations: Option<&str>,
) -> anyhow::Result<()> {
    let mut args = vec![
        "secrets".into(),
        "create".into(),
        secret_id.into(),
        "--project".into(),
        project.into(),
        "--quiet".into(),
    ];

    push_secret_location_args(&mut args, location);

    if let Some(replication_locations) = replication_locations {
        args.push("--replication-policy".into());
        args.push("user-managed".into());
        args.push("--locations".into());
        args.push(replication_locations.into());
    } else if location.is_none() {
        args.push("--replication-policy".into());
        args.push("automatic".into());
    }

    gcloud_cli(args, None, None, "gcloud secrets create should succeed")
}

/// Fetch the latest Secret Manager version payload as a string.
pub fn secret_manager_get_secret_string(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
) -> anyhow::Result<String> {
    let mut args = vec![
        "secrets".into(),
        "versions".into(),
        "access".into(),
        "latest".into(),
        "--secret".into(),
        secret_id.into(),
        "--project".into(),
        project.into(),
    ];

    push_secret_location_args(&mut args, location);

    let out = gcloud_capture_stdout(args, "gcloud secrets versions access should succeed")?;

    Ok(out.trim_end().to_string())
}

/// Add a new Secret Manager version for the given secret.
/// The secret value is sent through stdin using `--data-file=-`, so it is not
/// exposed as a command-line argument.
pub fn secret_manager_put_secret_string(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
    secret_string: &str,
) -> anyhow::Result<()> {
    let mut cmd = std::process::Command::new(gcloud_program());

    cmd.arg("secrets")
        .arg("versions")
        .arg("add")
        .arg(secret_id)
        .arg("--data-file=-")
        .arg("--project")
        .arg(project)
        .arg("--quiet")
        .stdin(Stdio::piped());

    if let Some(location) = location {
        cmd.arg("--location").arg(location);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| "gcloud secrets versions add should start".to_string())?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("gcloud secrets versions add stdin should be available"))?
        .write_all(secret_string.as_bytes())
        .with_context(|| "writing secret payload to gcloud stdin should succeed")?;

    let status = child
        .wait()
        .with_context(|| "gcloud secrets versions add should finish".to_string())?;

    if !status.success() {
        anyhow::bail!("gcloud secrets versions add should succeed (exit status {status})");
    }

    Ok(())
}

/// List all versions for a given secret as raw JSON.
pub fn secret_manager_list_secret_versions_json(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
) -> anyhow::Result<String> {
    let mut args = vec![
        "secrets".into(),
        "versions".into(),
        "list".into(),
        secret_id.into(),
        "--project".into(),
        project.into(),
        "--format".into(),
        "json".into(),
    ];

    push_secret_location_args(&mut args, location);

    let out = gcloud_capture_stdout(args, "gcloud secrets versions list should succeed")?;

    Ok(out.trim_end().to_string())
}

/// Describe a Secret Manager secret as raw JSON.
pub fn secret_manager_describe_secret_json(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
) -> anyhow::Result<String> {
    let mut args = vec![
        "secrets".into(),
        "describe".into(),
        secret_id.into(),
        "--project".into(),
        project.into(),
        "--format".into(),
        "json".into(),
    ];

    push_secret_location_args(&mut args, location);

    let out = gcloud_capture_stdout(args, "gcloud secrets describe should succeed")?;

    Ok(out.trim_end().to_string())
}

/// Return Ok(true) if the secret exists, Ok(false) if it does not.
pub fn secret_manager_secret_exists(
    secret_id: &str,
    project: &str,
    location: Option<&str>,
) -> anyhow::Result<bool> {
    let mut cmd = std::process::Command::new(gcloud_program());

    cmd.arg("secrets")
        .arg("describe")
        .arg(secret_id)
        .arg("--project")
        .arg(project)
        .arg("--quiet");

    if let Some(location) = location {
        cmd.arg("--location").arg(location);
    }

    let output = cmd.output().with_context(|| {
        format!(
            "Invoking 'gcloud secrets describe' for '{}' in project '{}' should succeed",
            secret_id, project
        )
    })?;

    if output.status.success() {
        return Ok(true);
    }

    if gcloud_missing_resource(&output.stderr) {
        return Ok(false);
    }

    anyhow::bail!(
        "gcloud secrets describe for '{}' in project '{}' should succeed (exit status: {}, stderr: {})",
        secret_id,
        project,
        output.status,
        gcloud_stderr(&output),
    )
}
