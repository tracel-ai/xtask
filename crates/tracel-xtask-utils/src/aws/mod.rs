pub mod asg_instance_picker;
pub mod cli;
pub mod ec2_tag_instance_picker;
pub mod images;
pub mod instance_logs;
pub mod instance_system_log;
pub mod naming;
pub mod regions;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct Ec2Describe {
    #[serde(rename = "Reservations")]
    pub(crate) reservations: Vec<Ec2Reservation>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Ec2Reservation {
    #[serde(rename = "Instances")]
    pub(crate) instances: Vec<Ec2Instance>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Ec2Instance {
    #[serde(rename = "InstanceId")]
    pub instance_id: String,
    #[serde(rename = "Placement")]
    pub placement: Ec2Placement,
    #[serde(rename = "LaunchTime")]
    pub launch_time: String,
    #[serde(rename = "PrivateIpAddress")]
    pub private_ip: Option<String>,
    #[serde(rename = "Tags")]
    pub tags: Option<Vec<Ec2Tag>>,
    #[serde(rename = "State")]
    pub state: InstanceState,
}

#[derive(Debug, Deserialize, Clone)]
pub struct InstanceState {
    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Ec2Placement {
    #[serde(rename = "AvailabilityZone")]
    pub availability_zone: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Ec2Tag {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
}
