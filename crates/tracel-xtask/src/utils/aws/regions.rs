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
