use anyhow::Context as _;
use inquire::Select;
use owo_colors::OwoColorize as _;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime};

use crate::utils::aws;

#[derive(Debug, Deserialize)]
struct AsgDescribe {
    #[serde(rename = "AutoScalingGroups")]
    auto_scaling_groups: Vec<AsgGroup>,
}

#[derive(Debug, Deserialize)]
struct AsgGroup {
    #[serde(rename = "Instances")]
    instances: Vec<AsgInstance>,

    #[serde(rename = "TargetGroupARNs")]
    target_group_arns: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AsgInstance {
    #[serde(rename = "InstanceId")]
    instance_id: String,

    #[serde(rename = "LifecycleState")]
    lifecycle_state: String,
}

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
    // RFC3339 format
    #[serde(rename = "LaunchTime")]
    launch_time: String,
    #[serde(rename = "PrivateIpAddress")]
    private_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Ec2Placement {
    #[serde(rename = "AvailabilityZone")]
    availability_zone: String,
}

#[derive(Debug, Deserialize)]
struct TgHealthDescribe {
    #[serde(rename = "TargetHealthDescriptions")]
    target_health_descriptions: Vec<TgHealthDesc>,
}

#[derive(Debug, Deserialize)]
struct TgHealthDesc {
    #[serde(rename = "Target")]
    target: TgTarget,
    #[serde(rename = "TargetHealth")]
    target_health: TgTargetHealth,
}

#[derive(Debug, Deserialize)]
struct TgTarget {
    #[serde(rename = "Id")]
    id: String,
}

#[derive(Debug, Deserialize)]
struct TgTargetHealth {
    #[serde(rename = "State")]
    state: String,
}

#[derive(Clone, Debug)]
pub struct SelectedAsgInstance {
    pub instance_id: String,
    pub tg_health: String,
    pub age: Duration,
    pub az: String,
    pub private_ip: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct PickerFormat {
    gap: &'static str,
}

impl Default for PickerFormat {
    fn default() -> Self {
        Self { gap: "   " }
    }
}

#[derive(Clone, Debug)]
struct InstanceChoice {
    instance: SelectedAsgInstance,
    format: PickerFormat,
}

impl fmt::Display for InstanceChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use owo_colors::OwoColorize;
        let x = &self.instance;
        let gap = self.format.gap;
        let age = shorten_duration(x.age);
        let (h_emoji, h_text) = health_badge(&x.tg_health);
        let ip = x.private_ip.as_deref().unwrap_or("no-ip");

        write!(
            f,
            "{}{gap}🌐 {}{gap}🖥️️ {}{gap}{h_emoji} {h_text}{gap}⏱️ {}",
            x.instance_id.yellow(),
            x.az,
            ip,
            age,
            gap = gap,
        )
    }
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

fn health_badge(health: &str) -> (&'static str, String) {
    // ELBv2 states include: healthy, unhealthy, initial, draining, unused, etc.
    match health {
        "healthy" => ("✅", health.green().to_string()),
        "unhealthy" => ("❌", health.red().to_string()),
        "draining" => ("🟠", health.yellow().to_string()),
        "initial" => ("🟡", health.yellow().to_string()),
        "unused" => ("⚪", health.dimmed().to_string()),
        "unknown" | "n/a" => ("❔", health.dimmed().to_string()),
        other => ("❔", other.dimmed().to_string()),
    }
}

fn parse_rfc3339_to_system_time(s: &str) -> anyhow::Result<SystemTime> {
    let dt = time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .with_context(|| format!("LaunchTime should be valid RFC3339, got '{s}'"))?;
    let unix = dt.unix_timestamp();
    let nanos = dt.nanosecond();
    // LaunchTime should never be before epoch, but keep it robust.
    let base = if unix >= 0 {
        SystemTime::UNIX_EPOCH + Duration::from_secs(unix as u64)
    } else {
        SystemTime::UNIX_EPOCH - Duration::from_secs((-unix) as u64)
    };

    Ok(base + Duration::from_nanos(nanos as u64))
}

/// Pick an instance from an ASG with a interactive prompt.
pub fn pick_asg_instance(region: &str, asg: &str) -> anyhow::Result<SelectedAsgInstance> {
    eprintln!("🔍 Fetching instances info...");
    // retrieve JSON info about the ASG
    let asg_json = aws::cli::ec2_autoscaling_describe_groups_json(region, asg)?;
    let asg_desc: AsgDescribe =
        serde_json::from_str(&asg_json).context("Parsing ASG describe JSON should succeed")?;
    let group = asg_desc
        .auto_scaling_groups
        .first()
        .with_context(|| format!("Auto Scaling Group '{asg}' should exist"))?;
    let instance_ids: Vec<String> = group
        .instances
        .iter()
        .filter(|i| i.lifecycle_state == "InService")
        .map(|i| i.instance_id.clone())
        .collect();
    if instance_ids.is_empty() {
        anyhow::bail!(
            "Auto Scaling Group '{asg}' should have at least one Instance but none were found!"
        );
    }
    let tg_arn = group
        .target_group_arns
        .as_ref()
        .and_then(|v| v.first())
        .cloned()
        .with_context(|| {
            format!("Auto Scaling Group '{asg}' should have at least one Target Group ARN")
        })?;

    // retrieve instances health
    let tg_json = aws::cli::ec2_elbv2_describe_target_health_json(region, &tg_arn)?;
    let tg_desc: TgHealthDescribe =
        serde_json::from_str(&tg_json).context("Parsing target health JSON should succeed")?;
    let mut health_by_id: HashMap<String, String> = HashMap::new();
    for d in tg_desc.target_health_descriptions {
        health_by_id.insert(d.target.id, d.target_health.state);
    }

    // retrieve launch time and AZ for each instance
    let ec2_json = aws::cli::ec2_describe_instances_json(region, &instance_ids)?;
    let ec2_desc: Ec2Describe =
        serde_json::from_str(&ec2_json).context("Parsing EC2 describe JSON should succeed")?;
    let now = SystemTime::now();

    // create choices
    let mut choices: Vec<InstanceChoice> = Vec::new();
    for r in ec2_desc.reservations {
        for i in r.instances {
            let launch = parse_rfc3339_to_system_time(&i.launch_time)
                .context("Parsing EC2 LaunchTime should succeed")?;
            let age = now.duration_since(launch).unwrap_or(Duration::from_secs(0));
            let fmt = PickerFormat::default();
            choices.push(InstanceChoice {
                instance: SelectedAsgInstance {
                    instance_id: i.instance_id.clone(),
                    tg_health: health_by_id
                        .get(&i.instance_id)
                        .cloned()
                        .unwrap_or_else(|| "n/a".to_string()),
                    age,
                    az: i.placement.availability_zone.clone(),
                    private_ip: i.private_ip.clone(),
                },
                format: fmt,
            });
        }
    }
    // we put the newest instances first
    choices.sort_by_key(|c| c.instance.age);

    // prompt user to select an instance and return it
    let selected = Select::new("Select Instance:", choices)
        .with_page_size(12)
        .with_vim_mode(true)
        .with_help_message("↑/↓ (or j/k), type to filter, Enter to connect, Esc to cancel")
        .prompt()
        .context("Selecting an instance should succeed")?;

    Ok(selected.instance)
}
