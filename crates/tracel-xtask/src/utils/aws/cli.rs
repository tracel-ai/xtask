use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;

use crate::{prelude::run_process, utils::process::run_process_capture_stdout};

/// Run a process, discarding stdout and inheriting stderr.
/// Fail on non-zero exit.
fn run_process_quiet(cmd: &mut Command, error_msg: &str) -> anyhow::Result<()> {
    // Discard stdout to avoid noise in our CLI output.
    cmd.stdout(Stdio::null());

    let status = cmd.status().with_context(|| {
        format!(
            "{error_msg} (failed to spawn '{}')",
            cmd.get_program().to_string_lossy()
        )
    })?;

    if !status.success() {
        anyhow::bail!("{error_msg} (exit status {status})");
    }

    Ok(())
}

/// Run `aws` cli with passed arguments.
/// Uses the generic `run_process`, but injects AWS env vars to disable pager/auto-prompt.
pub fn aws_cli(
    args: Vec<String>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let mut merged_envs: HashMap<&str, &str> = envs.unwrap_or_default();
    merged_envs.insert("AWS_PAGER", "");
    merged_envs.insert("AWS_CLI_AUTO_PROMPT", "off");

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_process("aws", &arg_refs, Some(merged_envs), path, error_msg)
}

/// Run `aws` cli with passed arguments, discarding stdout but keeping stderr.
/// Useful for commands where you only care about success/failure, not output.
pub fn aws_cli_quiet(
    args: Vec<String>,
    envs: Option<HashMap<&str, &str>>,
    path: Option<&Path>,
    error_msg: &str,
) -> anyhow::Result<()> {
    let mut cmd = Command::new("aws");

    if let Some(p) = path {
        cmd.current_dir(p);
    }
    if let Some(e) = envs {
        cmd.envs(e);
    }

    // Always disable AWS pager and auto-prompt for our wrappers.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    for a in &args {
        cmd.arg(a);
    }

    run_process_quiet(&mut cmd, error_msg)
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

    // Always disable AWS pager and auto-prompt for our wrappers.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

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
    let mut cmd = Command::new("aws");

    // Always disable AWS pager and auto-prompt for our wrappers.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    cmd.args(&args);

    let out = cmd.output().with_context(|| label.to_string())?;
    if !out.status.success() {
        return Ok(None);
    }
    let s = String::from_utf8(out.stdout).context("utf8 stdout")?;
    Ok(Some(s))
}

/// Return the setup account Id of the local aws cli.
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

// EC2 -----------------------------------------------------------------------

pub fn ec2_describe_instances_json(
    region: &str,
    instance_ids: &[String],
) -> anyhow::Result<String> {
    anyhow::ensure!(
        !instance_ids.is_empty(),
        "ec2 describe-instances should be called with at least one instance id"
    );

    let mut args: Vec<String> = vec![
        "ec2".into(),
        "describe-instances".into(),
        "--region".into(),
        region.into(),
        "--output".into(),
        "json".into(),
        "--instance-ids".into(),
    ];
    args.extend(instance_ids.iter().cloned());

    aws_cli_capture_stdout(args, "aws ec2 describe-instances", None, None)
        .map(|s| s.trim_end().to_string())
}

pub fn ec2_instance_get_console_output_json(
    region: &str,
    instance_id: &str,
) -> anyhow::Result<String> {
    aws_cli_capture_stdout(
        vec![
            "ec2".into(),
            "get-console-output".into(),
            "--instance-id".into(),
            instance_id.into(),
            "--region".into(),
            region.into(),
            "--latest".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws ec2 get-console-output should succeed",
        None,
        None,
    )
    .map(|s| s.trim_end().to_string())
}

/// Start an Auto Scaling Group instance refresh and return its refresh ID.
/// If you pass `None` for `preferences_json`, AWS will use the ASG defaults.
/// Example preferences (as JSON string):
/// r#"{"Strategy":"Rolling","InstanceWarmup":120,"MinHealthyPercentage":90}"#
///
/// Note: this only *starts* the refresh; it does not wait for completion.
pub fn ec2_autoscaling_start_instance_refresh(
    asg_name: &str,
    region: &str,
    strategy: &str,
    preferences_json: Option<&str>,
) -> anyhow::Result<String> {
    let mut args = vec![
        "autoscaling".into(),
        "start-instance-refresh".into(),
        "--auto-scaling-group-name".into(),
        asg_name.into(),
        "--strategy".into(),
        strategy.into(),
        "--region".into(),
        region.into(),
        "--query".into(),
        "InstanceRefreshId".into(),
        "--output".into(),
        "text".into(),
    ];

    if let Some(prefs) = preferences_json {
        // AWS CLI expects a JSON payload as a single argument
        args.push("--preferences".into());
        args.push(prefs.into());
    }

    aws_cli_capture_stdout(args, "aws autoscaling start-instance-refresh", None, None)
        .map(|s| s.trim().to_string())
}

/// Get the latest instance refresh status for an ASG (if any).
/// Possible values include: Pending, InProgress, Successful, Failed, Cancelling, Cancelled.
pub fn ec2_autoscaling_latest_instance_refresh_status(
    asg_name: &str,
    region: &str,
) -> anyhow::Result<Option<String>> {
    let out = aws_cli_try_capture_stdout(
        vec![
            "autoscaling".into(),
            "describe-instance-refreshes".into(),
            "--auto-scaling-group-name".into(),
            asg_name.into(),
            "--region".into(),
            region.into(),
            "--query".into(),
            "sort_by(InstanceRefreshes,&StartTime)[-1].Status".into(),
            "--output".into(),
            "text".into(),
        ],
        "aws autoscaling describe-instance-refreshes",
    )?;

    Ok(out
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "None"))
}

pub fn ec2_autoscaling_rollback_instance_refresh(asg: &str, region: &str) -> anyhow::Result<()> {
    use crate::prelude::anyhow::Context as _;
    use std::process::Command;

    let output = Command::new("aws")
        .args([
            "autoscaling",
            "rollback-instance-refresh",
            "--auto-scaling-group-name",
            asg,
            "--region",
            region,
        ])
        .output()
        .with_context(|| {
            format!(
                "Rollback of instance refresh for Auto Scaling Group '{}' in region '{}' should succeed",
                asg, region
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Rollback of instance refresh for Auto Scaling Group '{}' in region '{}' should succeed, \
             but AWS CLI exited with:\n{}",
            asg,
            region,
            stderr
        );
    }

    Ok(())
}

pub fn ec2_autoscaling_describe_groups_json(
    region: &str,
    asg_name: &str,
) -> anyhow::Result<String> {
    aws_cli_capture_stdout(
        vec![
            "autoscaling".into(),
            "describe-auto-scaling-groups".into(),
            "--auto-scaling-group-names".into(),
            asg_name.into(),
            "--region".into(),
            region.into(),
            "--output".into(),
            "json".into(),
        ],
        "aws autoscaling describe-auto-scaling-groups",
        None,
        None,
    )
    .map(|s| s.trim_end().to_string())
}

pub fn ec2_elbv2_describe_target_health_json(
    region: &str,
    target_group_arn: &str,
) -> anyhow::Result<String> {
    aws_cli_capture_stdout(
        vec![
            "elbv2".into(),
            "describe-target-health".into(),
            "--target-group-arn".into(),
            target_group_arn.into(),
            "--region".into(),
            region.into(),
            "--output".into(),
            "json".into(),
        ],
        "aws elbv2 describe-target-health",
        None,
        None,
    )
    .map(|s| s.trim_end().to_string())
}

// ECR -----------------------------------------------------------------------

pub fn ecr_ensure_repo_exists(repository: &str, region: &str) -> anyhow::Result<()> {
    // We do not care about stdout for these calls; only success/failure.
    if aws_cli_quiet(
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
        "aws ecr describe-repositories should succeed",
    )
    .is_ok()
    {
        // repository found
        return Ok(());
    }
    // create the repository
    aws_cli_quiet(
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
        "aws ecr create-repository should succeed",
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
    aws_cli_quiet(
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
        "aws ecr put-image should succeed",
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

/// Get the commit sha for an alias tag (i.e. 'latest' or 'rollback')
pub fn ecr_get_commit_sha_tag_from_alias_tag(
    repository: &str,
    alias_tag: &str,
    region: &str,
) -> anyhow::Result<Option<String>> {
    // Describe the image by alias tag to get its image details with all tags
    let json = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "describe-images".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
            "--image-ids".into(),
            format!("imageTag={alias_tag}"),
            "--query".into(),
            "imageDetails[0].imageTags".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws ecr describe-images",
        None,
        None,
    )?;

    let v: serde_json::Value =
        serde_json::from_str(&json).context("parsing describe-images output")?;
    let tags = v.as_array().cloned().unwrap_or_default();

    // Return the first non-alias tag that looks like a commit sha
    let is_hexish = |s: &str| {
        let len = s.len();
        (7..=40).contains(&len) && s.chars().all(|c| c.is_ascii_hexdigit())
    };
    let mut candidates: Vec<String> = tags
        .into_iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .filter(|s| s != "latest" && s != "rollback" && is_hexish(s))
        .collect();
    candidates.sort_by_key(|s| std::cmp::Reverse(s.len()));
    Ok(candidates.into_iter().next())
}

/// Get the commit sha tag for the most recently pushed image in the repo.
pub fn ecr_get_last_pushed_commit_sha_tag(
    repository: &str,
    region: &str,
) -> anyhow::Result<Option<String>> {
    // Get tags for the most recent image by pushed time
    let json = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "describe-images".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
            "--query".into(),
            "max_by(imageDetails, & imagePushedAt).imageTags".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws ecr describe-images (last pushed)",
        None,
        None,
    )?;

    let v: serde_json::Value = serde_json::from_str(&json).context("parsing last-pushed tags")?;
    let tags = v.as_array().cloned().unwrap_or_default();

    // Return a non-alias tag that looks like a commit sha
    let is_hexish = |s: &str| {
        let len = s.len();
        (7..=40).contains(&len) && s.chars().all(|c| c.is_ascii_hexdigit())
    };
    let mut candidates: Vec<String> = tags
        .into_iter()
        .filter_map(|t| t.as_str().map(|s| s.to_string()))
        .filter(|s| s != "latest" && s != "rollback" && is_hexish(s))
        .collect();
    candidates.sort_by_key(|s| std::cmp::Reverse(s.len()));
    Ok(candidates.into_iter().next())
}

/// Fetch the latest numerical tag and return it incremented by 1
pub fn ecr_compute_next_numeric_tag(repository: &str, region: &str) -> anyhow::Result<u64> {
    let json = aws_cli_capture_stdout(
        vec![
            "ecr".into(),
            "describe-images".into(),
            "--repository-name".into(),
            repository.into(),
            "--region".into(),
            region.into(),
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

/// Quietly delete an ECR tag (batch-delete-image), discarding stdout but keeping stderr
/// and failing on non-zero exit.
pub fn aws_ecr_delete_tag_quiet(
    repository: &str,
    region: &str,
    image_id: &str,     // e.g. "imageTag=rollback_stag"
    rollback_tag: &str, // for error messages
) -> anyhow::Result<()> {
    let mut cmd = Command::new("aws");
    cmd.arg("ecr")
        .arg("batch-delete-image")
        .arg("--repository-name")
        .arg(repository)
        .arg("--image-ids")
        .arg(image_id)
        .arg("--region")
        .arg(region);

    // Disable AWS pager and auto-prompt so we never get an interactive UI.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    // Discard stdout to avoid JSON noise, keep stderr inherited.
    cmd.stdout(Stdio::null());

    let status = cmd.status().with_context(|| {
        format!(
            "removing '{rollback_tag}' tag should succeed (failed to spawn aws ecr batch-delete-image)"
        )
    })?;

    if !status.success() {
        anyhow::bail!("removing '{rollback_tag}' tag should succeed (exit status {status})");
    }

    Ok(())
}

// Secrets Manager ------------------------------------------------------------

/// Fetch the SecretString for a given secret.
/// `secret_id` can be a name or an ARN.
/// `out_format` can be either "text" or "json"
pub fn secretsmanager_get_secret_string(
    secret_id: &str,
    region: &str,
    out_format: &str,
) -> anyhow::Result<String> {
    let out = aws_cli_capture_stdout(
        vec![
            "secretsmanager".into(),
            "get-secret-value".into(),
            "--secret-id".into(),
            secret_id.into(),
            "--region".into(),
            region.into(),
            "--query".into(),
            "SecretString".into(),
            "--output".into(),
            out_format.into(),
        ],
        "aws secretsmanager get-secret-value",
        None,
        None,
    )?;

    Ok(out.trim_end().to_string())
}

/// Put a new SecretString value for the given secret.
/// This creates a new version of the secret.
pub fn secretsmanager_put_secret_string(
    secret_id: &str,
    region: &str,
    secret_string: &str,
) -> anyhow::Result<()> {
    // we avoid using `aws_cli` here to prevent logging the secret value in the process command line.
    let mut cmd = Command::new("aws");
    cmd.arg("secretsmanager")
        .arg("put-secret-value")
        .arg("--secret-id")
        .arg(secret_id)
        .arg("--region")
        .arg(region)
        .arg("--secret-string")
        .arg(secret_string);

    // Disable AWS pager and auto-prompt.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    let status = cmd
        .status()
        .with_context(|| "aws secretsmanager put-secret-value should succeed".to_string())?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "aws secretsmanager put-secret-value should succeed (exit status {status})"
        ));
    }

    Ok(())
}

/// Create an empty Secrets Manager secret (metadata only, no initial version).
pub fn secretsmanager_create_empty_secret(
    name: &str,
    region: &str,
    description: Option<&str>,
) -> anyhow::Result<()> {
    let mut args = vec![
        "secretsmanager".into(),
        "create-secret".into(),
        "--name".into(),
        name.into(),
        "--region".into(),
        region.into(),
    ];

    if let Some(desc) = description {
        args.push("--description".into());
        args.push(desc.into());
    }

    aws_cli_quiet(args, None, None, "aws secretsmanager create-secret failed")
}

/// List all versions (including deprecated ones) for a given secret as raw JSON.
/// `secret_id` can be a name or an ARN.
pub fn secretsmanager_list_secret_versions_json(
    secret_id: &str,
    region: &str,
) -> anyhow::Result<String> {
    let out = aws_cli_capture_stdout(
        vec![
            "secretsmanager".into(),
            "list-secret-version-ids".into(),
            "--secret-id".into(),
            secret_id.into(),
            "--region".into(),
            region.into(),
            "--include-deprecated".into(),
            "--output".into(),
            "json".into(),
        ],
        "aws secretsmanager list-secret-version-ids",
        None,
        None,
    )?;

    Ok(out.trim_end().to_string())
}

/// Describe a Secrets Manager secret as raw JSON.
pub fn secretsmanager_describe_secret(secret_id: &str, region: &str) -> anyhow::Result<String> {
    let out = aws_cli_capture_stdout(
        vec![
            "secretsmanager".into(),
            "describe-secret".into(),
            "--secret-id".into(),
            secret_id.into(),
            "--region".into(),
            region.into(),
            "--output".into(),
            "json".into(),
        ],
        "aws secretsmanager describe-secret",
        None,
        None,
    )?;

    Ok(out.trim_end().to_string())
}

/// Return Ok(true) if the secret exists, Ok(false) if it does not.
pub fn secretsmanager_secret_exists(secret_id: &str, region: &str) -> anyhow::Result<bool> {
    let mut cmd = Command::new("aws");
    cmd.arg("secretsmanager")
        .arg("describe-secret")
        .arg("--secret-id")
        .arg(secret_id)
        .arg("--region")
        .arg(region);

    // Disable AWS pager and auto-prompt.
    cmd.env("AWS_PAGER", "");
    cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    let output = cmd.output().with_context(|| {
        format!(
            "Invoking 'aws secretsmanager describe-secret' for '{}' in region '{}' should succeed",
            secret_id, region
        )
    })?;

    if output.status.success() {
        // Secret exists
        return Ok(true);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("ResourceNotFoundException") {
        // Secret does not exist
        return Ok(false);
    }

    // other error
    Err(anyhow::anyhow!(
        "aws secretsmanager describe-secret for '{}' in region '{}' should succeed (exit status: {}, stderr: {})",
        secret_id,
        region,
        output.status,
        stderr.trim(),
    ))
}

// Systems Manager -----------------------------------------------------------

/// document to be able to login as a specific user in an SSM session
pub fn ensure_ssm_document(doc_name: &str, region: &str, login_user: &str) -> anyhow::Result<()> {
    let document_json = format!(
        r#"{{
        "schemaVersion": "1.0",
        "description": "Xtask interactive shell",
        "sessionType": "Standard_Stream",
        "inputs": {{
            "runAsEnabled": true,
            "runAsDefaultUser": "{user}",
            "shellProfile": {{
                "linux": "cd ~; exec bash -l"
            }}
        }}
    }}"#,
        user = login_user,
    );

    // Check if document exists
    let mut check_cmd = std::process::Command::new("aws");
    check_cmd.args([
        "ssm",
        "describe-document",
        "--name",
        doc_name,
        "--region",
        region,
    ]);
    check_cmd.env("AWS_PAGER", "");
    check_cmd.env("AWS_CLI_AUTO_PROMPT", "off");

    let check = check_cmd.output().context("describe-document should run")?;

    if !check.status.success() {
        // Create doc
        eprintln!("📄 Creating SSM document '{doc_name}'...");
        let mut create_cmd = std::process::Command::new("aws");
        create_cmd
            .args([
                "ssm",
                "create-document",
                "--name",
                doc_name,
                "--content",
                &document_json,
                "--document-type",
                "Session",
                "--region",
                region,
            ])
            .env("AWS_PAGER", "")
            .env("AWS_CLI_AUTO_PROMPT", "off");

        let create = create_cmd.output().context("create-document should run")?;

        if !create.status.success() {
            let stderr = String::from_utf8_lossy(&create.stderr);
            // In case of race
            if !stderr.contains("AlreadyExistsException") {
                anyhow::bail!("create-document failed:\n{stderr}");
            }
        }
    } else {
        // Update doc to ensure latest content
        eprintln!("📄 Updating SSM document '{doc_name}' to latest content...");
        let mut update_cmd = std::process::Command::new("aws");
        update_cmd
            .args([
                "ssm",
                "update-document",
                "--name",
                doc_name,
                "--content",
                &document_json,
                "--document-version",
                "$LATEST",
                "--region",
                region,
            ])
            .env("AWS_PAGER", "")
            .env("AWS_CLI_AUTO_PROMPT", "off");

        let update = update_cmd.output().context("update-document should run")?;

        if !update.status.success() {
            let stderr = String::from_utf8_lossy(&update.stderr);

            // If content is identical, treat as success
            if stderr.contains("DuplicateDocumentContent") {
                eprintln!("ℹ️ SSM document '{doc_name}' is already up to date.");
                return Ok(());
            }
            anyhow::bail!("update-document failed:\n{stderr}");
        }
    }

    Ok(())
}
