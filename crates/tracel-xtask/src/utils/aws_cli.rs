use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;

use super::process::{run_process, run_process_capture_stdout};

/// Run `aws` cli with passed arguments.
pub fn aws_cli(
    args: Vec<String>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_process("aws", &arg_refs, envs, path, error_msg)
}

/// Run `aws` cli and capture stdout.
/// Fail on non-zero exit.
pub fn aws_cli_capture_stdout(
    args: Vec<String>,
    label: &str,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
) -> anyhow::Result<String> {
    let mut cmd = Command::new("aws");
    if let Some(p) = path {
        cmd.current_dir(p);
    }
    if let Some(e) = envs {
        cmd.envs(e);
    }
    for a in &args {
        cmd.arg(a);
    }
    run_process_capture_stdout(&mut cmd, label)
}

/// Run `aws` cli and capture stdout.
/// Return Ok(None) on non-zero exit.
/// Useful for commands where “not found” is a non-zero exit you want to treat as absence.
pub fn aws_cli_try_capture_stdout(
    args: Vec<String>,
    label: &str,
) -> anyhow::Result<Option<String>> {
    let out = Command::new("aws")
        .args(&args)
        .output()
        .with_context(|| label.to_string())?;
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8(out.stdout).context("utf8 stdout")?;
    Ok(Some(s))
}

// High level helpers --------------------------------------------------------

pub fn aws_account_id() -> anyhow::Result<String> {
    aws_cli_capture_stdout(
        vec![
            "sts".into(),
            "get-caller-identity".into(),
            "--query".into(),
            "Account".into(),
            "--output".into(),
            "text".into(),
        ],
        "aws sts get-caller-identity",
        None,
        None,
    )
    .map(|s| s.trim().to_string())
}

// ECR -----------------------------------------------------------------------

pub fn ecr_ensure_repo_exists(repository: &str, region: &str) -> anyhow::Result<()> {
    if aws_cli(
        vec![
            "ecr".into(),
            "describe-repositories".into(),
            "--repository-names".into(),
            repository.into(),
            "--region".into(),
            region.into(),
        ],
        None,
        None,
        "aws ecr describe-repositories failed",
    )
    .is_ok()
    {
        // repository found
        return Ok(());
    }
    // create the repository
    aws_cli(
        vec![
            "ecr".into(),
            "create-repository".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
        ],
        None,
        None,
        "aws ecr create-repository failed",
    )
}

pub fn ecr_docker_login(account_id: &str, region: &str) -> anyhow::Result<()> {
    let registry = format!("{account_id}.dkr.ecr.{region}.amazonaws.com");
    // get login password
    let pass = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "get-login-password".into(),
            "--region".into(),
            region.into(),
        ],
        "aws ecr get-login-password",
        None,
        None,
    )?;
    // docker login
    let mut proc = Command::new("docker")
        .arg("login")
        .args(["--username", "AWS"])
        .arg("--password-stdin")
        .arg(&registry)
        .stdin(Stdio::piped())
        .spawn()
        .context("spawning docker login")?;
    proc.stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("no stdin for docker login"))?
        .write_all(pass.trim_end().as_bytes())?;
    let status = proc.wait().context("waiting on docker login")?;
    if !status.success() {
        return Err(anyhow::anyhow!("docker login failed: {status}"));
    }
    Ok(())
}

pub fn ecr_get_manifest(
    repository: &str,
    region: &str,
    tag: &str,
) -> anyhow::Result<Option<String>> {
    let args = vec![
        "ecr".into(),
        "batch-get-image".into(),
        "--repository-name".into(),
        repository.into(),
        "--region".into(),
        region.into(),
        "--image-ids".into(),
        format!("imageTag={tag}"),
        "--accepted-media-types".into(),
        "application/vnd.docker.distribution.manifest.v2+json".into(),
        "--query".into(),
        "images[0].imageManifest".into(),
        "--output".into(),
        "text".into(),
    ];
    match aws_cli_try_capture_stdout(args, "aws ecr batch-get-image")? {
        Some(s) => {
            let s = s.trim().to_string();
            if s.is_empty() || s == "None" {
                Ok(None)
            } else {
                Ok(Some(s))
            }
        }
        None => Ok(None),
    }
}

pub fn ecr_put_manifest(
    repository: &str,
    region: &str,
    tag: &str,
    manifest: &str,
) -> anyhow::Result<()> {
    aws_cli(
        vec![
            "ecr".into(),
            "put-image".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
            "--image-tag".into(),
            tag.into(),
            "--image-manifest".into(),
            manifest.into(),
        ],
        None,
        None,
        "aws ecr put-image failed",
    )
}

/// Query the digest for a given repository, region and tag
pub fn ecr_image_digest(
    repository: &str,
    tag: &str,
    region: &str,
) -> anyhow::Result<Option<String>> {
    let json = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "describe-images".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
            "--image-ids".into(),
            format!("imageTag={tag}"),
            "--query".into(),
            "imageDetails[0].imageDigest".into(),
            "--output".into(),
            "text".into(),
        ],
        "aws ecr describe-images for digest",
        None,
        None,
    )?;

    let digest = json.trim();
    if digest.is_empty() || digest.eq_ignore_ascii_case("None") {
        Ok(None)
    } else {
        Ok(Some(digest.to_string()))
    }
}

/// Generate the AWS Console URL that leads directly to the image details page
/// for the given repository and tag.
/// If the digest cannot be retrieved, return None.
pub fn ecr_image_url(repository: &str, tag: &str, region: &str) -> anyhow::Result<Option<String>> {
    use crate::utils::aws_cli::{aws_account_id, ecr_image_digest};
    let account_id = aws_account_id()?;
    if let Some(digest) = ecr_image_digest(repository, tag, region)? {
        Ok(Some(format!(
            "https://{region}.console.aws.amazon.com/ecr/repositories/private/{account_id}/{repository}/_/image/{digest}/details?region={region}",
            region = region,
            account_id = account_id,
            repository = repository,
            digest = digest,
        )))
    } else {
        Ok(None)
    }
}
