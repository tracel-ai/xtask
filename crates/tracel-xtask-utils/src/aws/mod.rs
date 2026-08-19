#[cfg(feature = "aws-asg-instance-picker")]
pub mod asg_instance_picker;
#[cfg(feature = "aws-cli")]
pub mod cli;
#[cfg(feature = "aws-ec2-tag-instance-picker")]
pub mod ec2_tag_instance_picker;
#[cfg(feature = "aws-images")]
pub mod images;
#[cfg(feature = "aws-instance-logs")]
pub mod instance_logs;
#[cfg(feature = "aws-instance-system-log")]
pub mod instance_system_log;
#[cfg(feature = "aws-naming")]
pub mod naming;
#[cfg(feature = "aws-regions")]
pub mod regions;

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
use serde::Deserialize;

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
#[derive(Debug, Deserialize)]
pub(crate) struct Ec2Describe {
    #[serde(rename = "Reservations")]
    pub(crate) reservations: Vec<Ec2Reservation>,
}

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
#[derive(Debug, Deserialize)]
pub(crate) struct Ec2Reservation {
    #[serde(rename = "Instances")]
    pub(crate) instances: Vec<Ec2Instance>,
}

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
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

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
#[derive(Debug, Deserialize, Clone)]
pub struct InstanceState {
    #[serde(rename = "Name")]
    pub name: String,
}

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
#[derive(Debug, Deserialize, Clone)]
pub struct Ec2Placement {
    #[serde(rename = "AvailabilityZone")]
    pub availability_zone: String,
}

#[cfg(any(feature = "aws-ec2-tag-instance-picker", feature = "aws-images"))]
#[derive(Debug, Deserialize, Clone)]
pub struct Ec2Tag {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: String,
}
