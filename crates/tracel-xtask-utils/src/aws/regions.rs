use std::fmt::{self, Display};

use strum::EnumIter;

/// Canonical AWS commercial regions.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum Region {
    // Africa
    AfSouth1, // af-south-1

    // Asia Pacific
    ApEast1,      // ap-east-1
    ApSouth1,     // ap-south-1
    ApSouth2,     // ap-south-2
    ApSoutheast1, // ap-southeast-1
    ApSoutheast2, // ap-southeast-2
    ApSoutheast3, // ap-southeast-3
    ApSoutheast4, // ap-southeast-4
    ApNortheast1, // ap-northeast-1
    ApNortheast2, // ap-northeast-2
    ApNortheast3, // ap-northeast-3

    // Canada
    CaCentral1, // ca-central-1

    // Europe
    EuCentral1, // eu-central-1
    EuCentral2, // eu-central-2
    EuWest1,    // eu-west-1
    EuWest2,    // eu-west-2
    EuWest3,    // eu-west-3
    EuNorth1,   // eu-north-1
    EuSouth1,   // eu-south-1
    EuSouth2,   // eu-south-2
    IlCentral1, // il-central-1

    // Middle East
    MeSouth1,   // me-south-1
    MeCentral1, // me-central-1

    // South America
    SaEast1, // sa-east-1

    // US
    UsEast1, // us-east-1
    UsEast2, // us-east-2
    UsWest1, // us-west-1
    UsWest2, // us-west-2
}

impl Region {
    pub fn cidr_shard(self) -> u8 {
        use Region::*;
        match self {
            UsEast1 => 0,
            UsEast2 => 1,
            UsWest1 => 2,
            UsWest2 => 3,
            CaCentral1 => 4,
            SaEast1 => 5,
            EuWest1 => 6,
            EuWest2 => 7,
            EuWest3 => 8,
            EuCentral1 => 9,
            EuCentral2 => 10,
            EuNorth1 => 11,
            EuSouth1 => 12,
            EuSouth2 => 13,
            MeCentral1 => 14,
            MeSouth1 => 15,
            AfSouth1 => 16,
            IlCentral1 => 17,
            ApSouth1 => 18,
            ApSouth2 => 19,
            ApSoutheast1 => 20,
            ApSoutheast2 => 21,
            ApSoutheast3 => 22,
            ApNortheast1 => 23,
            ApNortheast2 => 24,
            ApNortheast3 => 25,
            ApSoutheast4 => 26,
            ApEast1 => 27,
            // max 31 (5-bit)
        }
    }

    pub fn long(self) -> &'static str {
        use Region::*;
        match self {
            AfSouth1 => "af-south-1",
            ApEast1 => "ap-east-1",
            ApSouth1 => "ap-south-1",
            ApSouth2 => "ap-south-2",
            ApSoutheast1 => "ap-southeast-1",
            ApSoutheast2 => "ap-southeast-2",
            ApSoutheast3 => "ap-southeast-3",
            ApSoutheast4 => "ap-southeast-4",
            ApNortheast1 => "ap-northeast-1",
            ApNortheast2 => "ap-northeast-2",
            ApNortheast3 => "ap-northeast-3",
            CaCentral1 => "ca-central-1",
            EuCentral1 => "eu-central-1",
            EuCentral2 => "eu-central-2",
            EuWest1 => "eu-west-1",
            EuWest2 => "eu-west-2",
            EuWest3 => "eu-west-3",
            EuNorth1 => "eu-north-1",
            EuSouth1 => "eu-south-1",
            EuSouth2 => "eu-south-2",
            IlCentral1 => "il-central-1",
            MeSouth1 => "me-south-1",
            MeCentral1 => "me-central-1",
            SaEast1 => "sa-east-1",
            UsEast1 => "us-east-1",
            UsEast2 => "us-east-2",
            UsWest1 => "us-west-1",
            UsWest2 => "us-west-2",
        }
    }

    /// Short, compact code for naming.
    pub fn short(self) -> &'static str {
        use Region::*;
        match self {
            AfSouth1 => "afs1",
            ApEast1 => "ape1",
            ApSouth1 => "aps1",
            ApSouth2 => "aps2",
            ApSoutheast1 => "apse1",
            ApSoutheast2 => "apse2",
            ApSoutheast3 => "apse3",
            ApSoutheast4 => "apse4",
            ApNortheast1 => "apn1",
            ApNortheast2 => "apn2",
            ApNortheast3 => "apn3",
            CaCentral1 => "cac1",
            EuCentral1 => "euc1",
            EuCentral2 => "euc2",
            EuWest1 => "euw1",
            EuWest2 => "euw2",
            EuWest3 => "euw3",
            EuNorth1 => "eun1",
            EuSouth1 => "eus1",
            EuSouth2 => "eus2",
            IlCentral1 => "ilc1",
            MeSouth1 => "mes1",
            MeCentral1 => "mec1",
            SaEast1 => "sae1",
            UsEast1 => "use1",
            UsEast2 => "use2",
            UsWest1 => "usw1",
            UsWest2 => "usw2",
        }
    }

    pub fn availability_zones(self) -> &'static [&'static str] {
        use Region::*;
        match self {
            // Africa
            AfSouth1 => &["af-south-1a", "af-south-1b", "af-south-1c"],

            // Asia Pacific
            ApEast1 => &["ap-east-1a", "ap-east-1b", "ap-east-1c"],
            ApSouth1 => &["ap-south-1a", "ap-south-1b", "ap-south-1c"],
            ApSouth2 => &["ap-south-2a", "ap-south-2b", "ap-south-2c"],
            ApSoutheast1 => &["ap-southeast-1a", "ap-southeast-1b", "ap-southeast-1c"],
            ApSoutheast2 => &["ap-southeast-2a", "ap-southeast-2b", "ap-southeast-2c"],
            ApSoutheast3 => &["ap-southeast-3a", "ap-southeast-3b", "ap-southeast-3c"],
            ApSoutheast4 => &["ap-southeast-4a", "ap-southeast-4b", "ap-southeast-4c"],
            ApNortheast1 => &[
                "ap-northeast-1a",
                "ap-northeast-1b",
                "ap-northeast-1c",
                "ap-northeast-1d",
            ],
            ApNortheast2 => &["ap-northeast-2a", "ap-northeast-2b", "ap-northeast-2c"],
            ApNortheast3 => &["ap-northeast-3a", "ap-northeast-3b", "ap-northeast-3c"],

            // Canada
            CaCentral1 => &["ca-central-1a", "ca-central-1b", "ca-central-1d"],

            // Europe
            EuCentral1 => &["eu-central-1a", "eu-central-1b", "eu-central-1c"],
            EuCentral2 => &["eu-central-2a", "eu-central-2b", "eu-central-2c"],
            EuWest1 => &["eu-west-1a", "eu-west-1b", "eu-west-1c"],
            EuWest2 => &["eu-west-2a", "eu-west-2b", "eu-west-2c"],
            EuWest3 => &["eu-west-3a", "eu-west-3b", "eu-west-3c"],
            EuNorth1 => &["eu-north-1a", "eu-north-1b", "eu-north-1c"],
            EuSouth1 => &["eu-south-1a", "eu-south-1b", "eu-south-1c"],
            EuSouth2 => &["eu-south-2a", "eu-south-2b", "eu-south-2c"],
            IlCentral1 => &["il-central-1a", "il-central-1b", "il-central-1c"],

            // Middle East
            MeSouth1 => &["me-south-1a", "me-south-1b", "me-south-1c"],
            MeCentral1 => &["me-central-1a", "me-central-1b", "me-central-1c"],

            // South America
            SaEast1 => &["sa-east-1a", "sa-east-1b", "sa-east-1c"],

            // United States
            UsEast1 => &[
                "us-east-1a",
                "us-east-1b",
                "us-east-1c",
                "us-east-1d",
                "us-east-1e",
                "us-east-1f",
            ],
            UsEast2 => &["us-east-2a", "us-east-2b", "us-east-2c"],
            UsWest1 => &["us-west-1a", "us-west-1b", "us-west-1c"],
            UsWest2 => &["us-west-2a", "us-west-2b", "us-west-2c", "us-west-2d"],
        }
    }

    pub fn ec2_instance_connect_cidrs(self) -> &'static [&'static str] {
        use Region::*;
        match self {
            AfSouth1 => &[],
            ApEast1 => &[],
            ApNortheast1 => &["18.182.96.0/29"],
            ApNortheast2 => &[],
            ApNortheast3 => &[],
            ApSouth1 => &[],
            ApSouth2 => &[],
            ApSoutheast1 => &["13.250.186.80/29"],
            ApSoutheast2 => &["13.54.254.216/29"],
            ApSoutheast3 => &[],
            ApSoutheast4 => &[],
            CaCentral1 => &["3.96.0.0/29"],
            EuCentral1 => &["3.120.181.40/29"],
            EuCentral2 => &[],
            EuNorth1 => &["13.48.207.0/29"],
            EuSouth1 => &[],
            EuSouth2 => &[],
            EuWest1 => &["34.247.90.224/29"],
            EuWest2 => &["18.132.158.0/29"],
            EuWest3 => &["35.180.112.0/29"],
            IlCentral1 => &[],
            MeCentral1 => &[],
            MeSouth1 => &[],
            SaEast1 => &["18.228.70.32/29"],
            UsEast1 => &["18.206.107.24/29"],
            UsEast2 => &[],
            UsWest1 => &[],
            UsWest2 => &["18.237.140.160/29"],
        }
    }
}

impl Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.long())
    }
}
