use anyhow::Context as _;
use inquire::Select;
use owo_colors::OwoColorize as _;
use serde::Deserialize;
use std::fmt;
use std::time::{Duration, SystemTime};

use crate::process::run_process_capture_stdout;

#[derive(Debug, Deserialize)]
struct Ec2Describe {
    #[serde(rename = "Reservations")]
    reservations: Vec<Ec2Reservation>,
}

#[derive(Debug, Deserialize)]
struct Ec2Reservation {
    #[serde(rename = "Instances")]
    instances: Vec<Ec2Instance>,
}

#[derive(Debug, Deserialize)]
struct Ec2Instance {
    #[serde(rename = "InstanceId")]
    instance_id: String,
    #[serde(rename = "Placement")]
    placement: Ec2Placement,
    #[serde(rename = "LaunchTime")]
    launch_time: String,
    #[serde(rename = "PrivateIpAddress")]
    private_ip: Option<String>,
    #[serde(rename = "Tags")]
    tags: Option<Vec<Ec2Tag>>,
}

#[derive(Debug, Deserialize)]
struct Ec2Placement {
    #[serde(rename = "AvailabilityZone")]
    availability_zone: String,
}

#[derive(Debug, Deserialize)]
struct Ec2Tag {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Value")]
    value: String,
}

#[derive(Clone, Debug)]
pub struct SelectedEc2Instance {
    pub instance_id: String,
    pub name: Option<String>,
    pub age: Duration,
    pub az: String,
    pub private_ip: Option<String>,
    pub tags: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug)]
struct PickerFormat {
    gap: &'static str,
    name_w: usize,
    az_w: usize,
    ip_w: usize,
    age_w: usize,
}

impl Default for PickerFormat {
    fn default() -> Self {
        Self {
            gap: "   ",
            name_w: 0,
            az_w: 0,
            ip_w: 0,
            age_w: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct InstanceChoice {
    instance: SelectedEc2Instance,
    format: PickerFormat,
}

impl fmt::Display for InstanceChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let x = &self.instance;
        let fmt_cfg = self.format;
        let gap = fmt_cfg.gap;

        let name = x.name.as_deref().unwrap_or(&x.instance_id);
        let ip = x.private_ip.as_deref().unwrap_or("no-ip");
        let age = shorten_duration(x.age);

        write!(
            f,
            "{:<name_w$}{gap}🌐 {:<az_w$}{gap}🖥️ {:<ip_w$}{gap}⏱️ {:>age_w$}",
            name.yellow(),
            x.az,
            ip,
            age,
            name_w = fmt_cfg.name_w,
            az_w = fmt_cfg.az_w,
            ip_w = fmt_cfg.ip_w,
            age_w = fmt_cfg.age_w,
            gap = gap,
        )
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum InstanceSort {
    #[default]
    Name,
    AgeAsc,
    AgeDesc,
}

fn shorten_duration(d: Duration) -> String {
    let s = d.as_secs();

    let days = s / 86_400;
    let hours = (s % 86_400) / 3600;
    let mins = (s % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

fn parse_rfc3339_to_system_time(s: &str) -> anyhow::Result<SystemTime> {
    let dt = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .with_context(|| format!("LaunchTime should be valid RFC3339, got '{s}'"))?;
    let unix = dt.unix_timestamp();
    let nanos = dt.nanosecond();

    let base = if unix >= 0 {
        SystemTime::UNIX_EPOCH + Duration::from_secs(unix as u64)
    } else {
        SystemTime::UNIX_EPOCH - Duration::from_secs((-unix) as u64)
    };

    Ok(base + Duration::from_nanos(nanos as u64))
}

fn find_tag(tags: &[Ec2Tag], key: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.key == key)
        .map(|tag| tag.value.clone())
}

/// Pick a running EC2 instance interactively from a set of AWS EC2 tag filters.
pub fn pick_ec2_instance_by_filters(
    region: &str,
    filters: &[String],
    sort: InstanceSort,
) -> anyhow::Result<SelectedEc2Instance> {
    eprintln!("🔍 Fetching instances info...");

    let mut cmd = std::process::Command::new("aws");
    cmd.arg("ec2")
        .arg("describe-instances")
        .arg("--region")
        .arg(region)
        .arg("--filters")
        .arg("Name=instance-state-name,Values=running");

    for filter in filters {
        cmd.arg(filter);
    }

    cmd.arg("--output").arg("json");

    let ec2_json = run_process_capture_stdout(
        &mut cmd,
        "aws ec2 describe-instances should succeed for tag-based picker",
    )?;

    let ec2_desc: Ec2Describe =
        serde_json::from_str(&ec2_json).context("Parsing EC2 describe JSON should succeed")?;

    let now = SystemTime::now();
    let mut instances = vec![];

    for reservation in ec2_desc.reservations {
        for instance in reservation.instances {
            let launch = parse_rfc3339_to_system_time(&instance.launch_time)
                .context("Parsing EC2 LaunchTime should succeed")?;

            let age = now.duration_since(launch).unwrap_or(Duration::from_secs(0));
            let tags = instance.tags.unwrap_or_default();

            let normalized_tags = tags
                .iter()
                .map(|tag| (tag.key.clone(), tag.value.clone()))
                .collect::<Vec<_>>();

            instances.push(SelectedEc2Instance {
                instance_id: instance.instance_id.clone(),
                name: find_tag(&tags, "Name"),
                age,
                az: instance.placement.availability_zone.clone(),
                private_ip: instance.private_ip.clone(),
                tags: normalized_tags,
            });
        }
    }

    if instances.is_empty() {
        anyhow::bail!("At least one running EC2 instance should match the provided filters.");
    }

    // sort choices
    match sort {
        InstanceSort::Name => {
            instances.sort_by(|a, b| {
                let a_name = a.name.as_deref().unwrap_or(&a.instance_id);
                let b_name = b.name.as_deref().unwrap_or(&b.instance_id);
                a_name.cmp(b_name)
            });
        }
        InstanceSort::AgeAsc => {
            instances.sort_by_key(|i| i.age);
        }
        InstanceSort::AgeDesc => {
            instances.sort_by_key(|i| std::cmp::Reverse(i.age));
        }
    }

    let mut format = PickerFormat::default();

    for instance in &instances {
        let name = instance.name.as_deref().unwrap_or(&instance.instance_id);
        let ip = instance.private_ip.as_deref().unwrap_or("no-ip");
        let age = shorten_duration(instance.age);

        format.name_w = format.name_w.max(name.len());
        format.az_w = format.az_w.max(instance.az.len());
        format.ip_w = format.ip_w.max(ip.len());
        format.age_w = format.age_w.max(age.len());
    }

    let choices = instances
        .into_iter()
        .map(|instance| InstanceChoice { instance, format })
        .collect::<Vec<_>>();

    let selected = Select::new("Select Instance:", choices)
        .with_page_size(12)
        .with_vim_mode(true)
        .with_help_message("↑/↓ (or j/k), type to filter, Enter to select, Esc to cancel")
        .prompt()
        .context("Selecting an EC2 instance should succeed")?;

    Ok(selected.instance)
}

/// Convenience helper for exact tag matches.
pub fn pick_ec2_instance_by_tags(
    region: &str,
    tags: &[(&str, &str)],
    sort: InstanceSort,
) -> anyhow::Result<SelectedEc2Instance> {
    let filters = tags
        .iter()
        .map(|(key, value)| format!("Name=tag:{key},Values={value}"))
        .collect::<Vec<_>>();

    pick_ec2_instance_by_filters(region, &filters, sort)
}
