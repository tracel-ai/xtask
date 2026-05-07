use serde::Deserialize;
use std::time::{Duration, Instant};

use crate::{
    aws::{Ec2Describe, Ec2Instance},
    process::{run_process, run_process_capture_stdout},
};

use anyhow::Context as _;

#[derive(Debug, Deserialize)]
struct CreateImageResponse {
    #[serde(rename = "ImageId")]
    image_id: String,
}

#[derive(Debug, Deserialize)]
struct DescribeImagesResponse {
    #[serde(rename = "Images")]
    images: Vec<AmiImage>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AmiImage {
    #[serde(rename = "ImageId")]
    pub image_id: String,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "State")]
    pub state: Option<String>,
    #[serde(rename = "CreationDate")]
    pub creation_date: Option<String>,
    #[serde(rename = "Tags", default)]
    pub tags: Vec<AwsTag>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AwsTag {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
}

pub fn find_single_baker_instance(region: &str, image: &str) -> anyhow::Result<Ec2Instance> {
    let instances = describe_baker_instances(region, image)?;

    match instances.len() {
        0 => anyhow::bail!("No baker instance found for image '{image}'"),
        1 => Ok(instances[0].clone()),
        n => {
            eprintln!("Found {n} baker instances for image '{image}':");
            for instance in &instances {
                eprintln!("• {} ({})", instance.instance_id, instance.state.name);
            }

            anyhow::bail!("Exactly one baker instance should exist for image '{image}', found {n}");
        }
    }
}

pub fn describe_baker_instances(region: &str, image: &str) -> anyhow::Result<Vec<Ec2Instance>> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "describe-instances",
        "--region",
        region,
        "--filters",
        "Name=tag:Baker,Values=true",
        &format!("Name=tag:ImageName,Values={image}"),
        "Name=instance-state-name,Values=pending,running,stopping,stopped",
        "--output",
        "json",
    ]);

    let out = run_process_capture_stdout(
        &mut cmd,
        "aws ec2 describe-instances for image baker should succeed",
    )?;

    let response: Ec2Describe = serde_json::from_str(&out)
        .context("aws ec2 describe-instances response should be valid JSON")?;

    Ok(response
        .reservations
        .into_iter()
        .flat_map(|reservation| reservation.instances)
        .collect())
}

pub fn wait_for_instance_stopped(
    region: &str,
    instance_id: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let poll = Duration::from_secs(10);
    let start = Instant::now();

    loop {
        let state = instance_state(region, instance_id)?;

        eprint!("\r⏳ Waiting for baker instance {instance_id} to stop — state: {state:<12}\x1b[K");

        if state == "stopped" {
            eprintln!("\r✅ Baker instance {instance_id} stopped\x1b[K");
            return Ok(());
        }

        if start.elapsed() >= timeout {
            eprintln!();
            anyhow::bail!(
                "Timed out after {} seconds while waiting for instance '{}' to stop",
                timeout.as_secs(),
                instance_id
            );
        }

        std::thread::sleep(poll);
    }
}

pub fn instance_state(region: &str, instance_id: &str) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "describe-instances",
        "--region",
        region,
        "--instance-ids",
        instance_id,
        "--query",
        "Reservations[0].Instances[0].State.Name",
        "--output",
        "text",
    ]);

    let out = run_process_capture_stdout(
        &mut cmd,
        "aws ec2 describe-instances instance state should succeed",
    )?;

    Ok(out.trim().to_string())
}

pub fn create_image(
    region: &str,
    instance_id: &str,
    ami_name: &str,
    image: &str,
    no_reboot: bool,
) -> anyhow::Result<String> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "create-image",
        "--region",
        region,
        "--instance-id",
        instance_id,
        "--name",
        ami_name,
        "--tag-specifications",
        &format!(
            "ResourceType=image,Tags=[{{Key=ImageName,Value={image}}},{{Key=BakerInstanceId,Value={instance_id}}},{{Key=ManagedBy,Value=xtask-image}}]"
        ),
        "--output",
        "json",
    ]);

    if no_reboot {
        cmd.arg("--no-reboot");
    }

    let out = run_process_capture_stdout(&mut cmd, "aws ec2 create-image should succeed")?;

    let response: CreateImageResponse =
        serde_json::from_str(&out).context("aws ec2 create-image response should be valid JSON")?;

    Ok(response.image_id)
}

pub fn wait_for_image_available(
    region: &str,
    ami_id: &str,
    timeout: Duration,
) -> anyhow::Result<()> {
    let poll = Duration::from_secs(10);
    let start = Instant::now();

    loop {
        let state = image_state(region, ami_id)?.unwrap_or_else(|| "unknown".to_string());

        eprint!("\r⏳ Waiting for AMI {ami_id} to become available — state: {state:<12}\x1b[K");

        if state == "available" {
            eprintln!("\r✅ AMI {ami_id} available\x1b[K");
            return Ok(());
        }

        if state == "failed" {
            eprintln!();
            anyhow::bail!("AMI '{ami_id}' entered failed state");
        }

        if start.elapsed() >= timeout {
            eprintln!();
            anyhow::bail!(
                "Timed out after {} seconds while waiting for AMI '{}' to become available",
                timeout.as_secs(),
                ami_id
            );
        }

        std::thread::sleep(poll);
    }
}

pub fn image_state(region: &str, ami_id: &str) -> anyhow::Result<Option<String>> {
    Ok(get_image_by_id(region, ami_id)?.and_then(|image| image.state))
}

pub fn get_image_by_id(region: &str, ami_id: &str) -> anyhow::Result<Option<AmiImage>> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "describe-images",
        "--region",
        region,
        "--image-ids",
        ami_id,
        "--output",
        "json",
    ]);

    let out = run_process_capture_stdout(&mut cmd, "aws ec2 describe-images by id should succeed")?;

    let response: DescribeImagesResponse = serde_json::from_str(&out)
        .context("aws ec2 describe-images response should be valid JSON")?;

    Ok(response.images.into_iter().next())
}

pub fn find_single_image_by_true_tag(
    region: &str,
    image: &str,
    tag_key: &str,
) -> anyhow::Result<Option<AmiImage>> {
    let images = describe_images_by_true_tag(region, image, tag_key)?;

    match images.len() {
        0 => Ok(None),
        1 => Ok(Some(images[0].clone())),
        n => {
            eprintln!("Found {n} AMIs for image '{image}' with tag '{tag_key}=true':");
            for image in &images {
                eprintln!(
                    "• {} ({})",
                    image.image_id,
                    image.name.as_deref().unwrap_or("unnamed")
                );
            }

            anyhow::bail!(
                "At most one AMI should have tag '{tag_key}=true' for image '{image}', found {n}"
            );
        }
    }
}

pub fn describe_images_by_true_tag(
    region: &str,
    image: &str,
    tag_key: &str,
) -> anyhow::Result<Vec<AmiImage>> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "describe-images",
        "--region",
        region,
        "--owners",
        "self",
        "--filters",
        &format!("Name=tag:ImageName,Values={image}"),
        &format!("Name=tag:{tag_key},Values=true"),
        "--output",
        "json",
    ]);

    let out =
        run_process_capture_stdout(&mut cmd, "aws ec2 describe-images by tag should succeed")?;

    let response: DescribeImagesResponse = serde_json::from_str(&out)
        .context("aws ec2 describe-images response should be valid JSON")?;

    Ok(response.images)
}

pub fn create_true_tag(region: &str, resource_id: &str, key: &str) -> anyhow::Result<()> {
    run_process(
        "aws",
        &[
            "ec2",
            "create-tags",
            "--region",
            region,
            "--resources",
            resource_id,
            "--tags",
            &format!("Key={key},Value=true"),
        ],
        None,
        None,
        "aws ec2 create-tags should succeed",
    )
}

pub fn delete_tag(region: &str, resource_id: &str, key: &str) -> anyhow::Result<()> {
    run_process(
        "aws",
        &[
            "ec2",
            "delete-tags",
            "--region",
            region,
            "--resources",
            resource_id,
            "--tags",
            &format!("Key={key}"),
        ],
        None,
        None,
        "aws ec2 delete-tags should succeed",
    )
}

pub fn ensure_image_matches_name(image: &AmiImage, expected: &str) -> anyhow::Result<()> {
    let actual = image
        .tags
        .iter()
        .find(|tag| tag.key == "ImageName")
        .map(|tag| tag.value.as_str());

    if actual != Some(expected) {
        anyhow::bail!(
            "AMI '{}' should have tag ImageName='{}', found '{}'",
            image.image_id,
            expected,
            actual.unwrap_or("<missing>")
        );
    }

    Ok(())
}

pub fn print_image_summary(image: &AmiImage) {
    eprintln!("  AMI:      {}", image.image_id);
    eprintln!("  Name:     {}", image.name.as_deref().unwrap_or("unnamed"));

    if let Some(state) = &image.state {
        eprintln!("  State:    {state}");
    }

    if let Some(creation_date) = &image.creation_date {
        eprintln!("  Created:  {creation_date}");
    }
}

pub fn find_latest_image_by_name(region: &str, image: &str) -> anyhow::Result<Option<AmiImage>> {
    let mut images = describe_images_by_name(region, image)?;

    images.sort_by(|a, b| {
        a.creation_date
            .as_deref()
            .unwrap_or("")
            .cmp(b.creation_date.as_deref().unwrap_or(""))
    });

    Ok(images.pop())
}

pub fn describe_images_by_name(region: &str, image: &str) -> anyhow::Result<Vec<AmiImage>> {
    let mut cmd = std::process::Command::new("aws");
    cmd.args([
        "ec2",
        "describe-images",
        "--region",
        region,
        "--owners",
        "self",
        "--filters",
        &format!("Name=tag:ImageName,Values={image}"),
        "Name=tag:ManagedBy,Values=xtask-image",
        "--output",
        "json",
    ]);

    let out =
        run_process_capture_stdout(&mut cmd, "aws ec2 describe-images by name should succeed")?;

    let response: DescribeImagesResponse = serde_json::from_str(&out)
        .context("aws ec2 describe-images response should be valid JSON")?;

    Ok(response.images)
}
