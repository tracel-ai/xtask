use std::fmt::{self, Display};

use strum::EnumIter;

/// Canonical Google Cloud commercial Compute Engine regions.
///
/// This module models standard Compute Engine regions/zones, while keeping
/// specialized AI zones separate through `Region::ai_zones()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter)]
pub enum Region {
    // Africa
    AfricaSouth1, // africa-south1
    // Americas - Canada / Mexico
    NorthAmericaNortheast1, // northamerica-northeast1
    NorthAmericaNortheast2, // northamerica-northeast2
    NorthAmericaSouth1,     // northamerica-south1
    // Americas - South America
    SouthAmericaEast1, // southamerica-east1
    SouthAmericaWest1, // southamerica-west1
    // Americas - United States
    UsCentral1, // us-central1
    UsEast1,    // us-east1
    UsEast4,    // us-east4
    UsEast5,    // us-east5
    UsSouth1,   // us-south1
    UsWest1,    // us-west1
    UsWest2,    // us-west2
    UsWest3,    // us-west3
    UsWest4,    // us-west4
    // Asia Pacific
    AsiaEast1,           // asia-east1
    AsiaEast2,           // asia-east2
    AsiaNortheast1,      // asia-northeast1
    AsiaNortheast2,      // asia-northeast2
    AsiaNortheast3,      // asia-northeast3
    AsiaSouth1,          // asia-south1
    AsiaSouth2,          // asia-south2
    AsiaSoutheast1,      // asia-southeast1
    AsiaSoutheast2,      // asia-southeast2
    AustraliaSoutheast1, // australia-southeast1
    AustraliaSoutheast2, // australia-southeast2
    // Europe
    EuropeCentral2,   // europe-central2
    EuropeNorth1,     // europe-north1
    EuropeSouthwest1, // europe-southwest1
    EuropeWest1,      // europe-west1
    EuropeWest2,      // europe-west2
    EuropeWest3,      // europe-west3
    EuropeWest4,      // europe-west4
    EuropeWest6,      // europe-west6
    EuropeWest8,      // europe-west8
    EuropeWest9,      // europe-west9
    EuropeWest10,     // europe-west10
    EuropeWest12,     // europe-west12
    // Middle East
    MeCentral1, // me-central1
    MeCentral2, // me-central2
    MeWest1,    // me-west1
}

impl Region {
    /// Stable shard index usable for deterministic CIDR derivation.
    pub fn cidr_shard(self) -> u8 {
        use Region::*;

        match self {
            UsCentral1 => 0,
            UsEast1 => 1,
            UsEast4 => 2,
            UsEast5 => 3,
            UsSouth1 => 4,
            UsWest1 => 5,
            UsWest2 => 6,
            UsWest3 => 7,
            UsWest4 => 8,
            NorthAmericaNortheast1 => 9,
            NorthAmericaNortheast2 => 10,
            NorthAmericaSouth1 => 11,
            SouthAmericaEast1 => 12,
            SouthAmericaWest1 => 13,
            EuropeWest1 => 14,
            EuropeWest2 => 15,
            EuropeWest3 => 16,
            EuropeWest4 => 17,
            EuropeWest6 => 18,
            EuropeWest8 => 19,
            EuropeWest9 => 20,
            EuropeWest10 => 21,
            EuropeWest12 => 22,
            EuropeCentral2 => 23,
            EuropeNorth1 => 24,
            EuropeSouthwest1 => 25,
            MeWest1 => 26,
            MeCentral1 => 27,
            MeCentral2 => 28,
            AfricaSouth1 => 29,
            AsiaEast1 => 30,
            AsiaEast2 => 31,
            AsiaNortheast1 => 32,
            AsiaNortheast2 => 33,
            AsiaNortheast3 => 34,
            AsiaSouth1 => 35,
            AsiaSouth2 => 36,
            AsiaSoutheast1 => 37,
            AsiaSoutheast2 => 38,
            AustraliaSoutheast1 => 39,
            AustraliaSoutheast2 => 40,
            // max 63 (6-bit)
        }
    }

    pub fn long(self) -> &'static str {
        use Region::*;

        match self {
            AfricaSouth1 => "africa-south1",
            NorthAmericaNortheast1 => "northamerica-northeast1",
            NorthAmericaNortheast2 => "northamerica-northeast2",
            NorthAmericaSouth1 => "northamerica-south1",
            SouthAmericaEast1 => "southamerica-east1",
            SouthAmericaWest1 => "southamerica-west1",
            UsCentral1 => "us-central1",
            UsEast1 => "us-east1",
            UsEast4 => "us-east4",
            UsEast5 => "us-east5",
            UsSouth1 => "us-south1",
            UsWest1 => "us-west1",
            UsWest2 => "us-west2",
            UsWest3 => "us-west3",
            UsWest4 => "us-west4",
            AsiaEast1 => "asia-east1",
            AsiaEast2 => "asia-east2",
            AsiaNortheast1 => "asia-northeast1",
            AsiaNortheast2 => "asia-northeast2",
            AsiaNortheast3 => "asia-northeast3",
            AsiaSouth1 => "asia-south1",
            AsiaSouth2 => "asia-south2",
            AsiaSoutheast1 => "asia-southeast1",
            AsiaSoutheast2 => "asia-southeast2",
            AustraliaSoutheast1 => "australia-southeast1",
            AustraliaSoutheast2 => "australia-southeast2",
            EuropeCentral2 => "europe-central2",
            EuropeNorth1 => "europe-north1",
            EuropeSouthwest1 => "europe-southwest1",
            EuropeWest1 => "europe-west1",
            EuropeWest2 => "europe-west2",
            EuropeWest3 => "europe-west3",
            EuropeWest4 => "europe-west4",
            EuropeWest6 => "europe-west6",
            EuropeWest8 => "europe-west8",
            EuropeWest9 => "europe-west9",
            EuropeWest10 => "europe-west10",
            EuropeWest12 => "europe-west12",
            MeCentral1 => "me-central1",
            MeCentral2 => "me-central2",
            MeWest1 => "me-west1",
        }
    }

    /// Short, compact code for naming.
    pub fn short(self) -> &'static str {
        use Region::*;

        match self {
            AfricaSouth1 => "afs1",

            NorthAmericaNortheast1 => "nane1",
            NorthAmericaNortheast2 => "nane2",
            NorthAmericaSouth1 => "nas1",

            SouthAmericaEast1 => "sae1",
            SouthAmericaWest1 => "saw1",

            UsCentral1 => "usc1",
            UsEast1 => "use1",
            UsEast4 => "use4",
            UsEast5 => "use5",
            UsSouth1 => "uss1",
            UsWest1 => "usw1",
            UsWest2 => "usw2",
            UsWest3 => "usw3",
            UsWest4 => "usw4",

            AsiaEast1 => "ase1",
            AsiaEast2 => "ase2",
            AsiaNortheast1 => "asne1",
            AsiaNortheast2 => "asne2",
            AsiaNortheast3 => "asne3",
            AsiaSouth1 => "ass1",
            AsiaSouth2 => "ass2",
            AsiaSoutheast1 => "asse1",
            AsiaSoutheast2 => "asse2",
            AustraliaSoutheast1 => "ause1",
            AustraliaSoutheast2 => "ause2",

            EuropeCentral2 => "euc2",
            EuropeNorth1 => "eun1",
            EuropeSouthwest1 => "eusw1",
            EuropeWest1 => "euw1",
            EuropeWest2 => "euw2",
            EuropeWest3 => "euw3",
            EuropeWest4 => "euw4",
            EuropeWest6 => "euw6",
            EuropeWest8 => "euw8",
            EuropeWest9 => "euw9",
            EuropeWest10 => "euw10",
            EuropeWest12 => "euw12",

            MeCentral1 => "mec1",
            MeCentral2 => "mec2",
            MeWest1 => "mew1",
        }
    }

    /// Standard Compute Engine zones for the region.
    ///
    /// Specialized AI zones are intentionally excluded. Use `ai_zones()` when
    /// explicitly targeting AI/ML accelerator capacity in AI zones.
    pub fn zones(self) -> &'static [&'static str] {
        use Region::*;

        match self {
            // Africa
            AfricaSouth1 => &["africa-south1-a", "africa-south1-b", "africa-south1-c"],
            // Americas - Canada / Mexico
            NorthAmericaNortheast1 => &[
                "northamerica-northeast1-a",
                "northamerica-northeast1-b",
                "northamerica-northeast1-c",
            ],
            NorthAmericaNortheast2 => &[
                "northamerica-northeast2-a",
                "northamerica-northeast2-b",
                "northamerica-northeast2-c",
            ],
            NorthAmericaSouth1 => &[
                "northamerica-south1-a",
                "northamerica-south1-b",
                "northamerica-south1-c",
            ],
            // Americas - South America
            SouthAmericaEast1 => &[
                "southamerica-east1-a",
                "southamerica-east1-b",
                "southamerica-east1-c",
            ],
            SouthAmericaWest1 => &[
                "southamerica-west1-a",
                "southamerica-west1-b",
                "southamerica-west1-c",
            ],
            // Americas - United States
            UsCentral1 => &[
                "us-central1-a",
                "us-central1-b",
                "us-central1-c",
                "us-central1-f",
            ],
            UsEast1 => &["us-east1-b", "us-east1-c", "us-east1-d"],
            UsEast4 => &["us-east4-a", "us-east4-b", "us-east4-c"],
            UsEast5 => &["us-east5-a", "us-east5-b", "us-east5-c"],
            UsSouth1 => &["us-south1-a", "us-south1-b", "us-south1-c"],
            UsWest1 => &["us-west1-a", "us-west1-b", "us-west1-c"],
            UsWest2 => &["us-west2-a", "us-west2-b", "us-west2-c"],
            UsWest3 => &["us-west3-a", "us-west3-b", "us-west3-c"],
            UsWest4 => &["us-west4-a", "us-west4-b", "us-west4-c"],
            // Asia Pacific
            AsiaEast1 => &["asia-east1-a", "asia-east1-b", "asia-east1-c"],
            AsiaEast2 => &["asia-east2-a", "asia-east2-b", "asia-east2-c"],
            AsiaNortheast1 => &[
                "asia-northeast1-a",
                "asia-northeast1-b",
                "asia-northeast1-c",
            ],
            AsiaNortheast2 => &[
                "asia-northeast2-a",
                "asia-northeast2-b",
                "asia-northeast2-c",
            ],
            AsiaNortheast3 => &[
                "asia-northeast3-a",
                "asia-northeast3-b",
                "asia-northeast3-c",
            ],
            AsiaSouth1 => &["asia-south1-a", "asia-south1-b", "asia-south1-c"],
            AsiaSouth2 => &["asia-south2-a", "asia-south2-b", "asia-south2-c"],
            AsiaSoutheast1 => &[
                "asia-southeast1-a",
                "asia-southeast1-b",
                "asia-southeast1-c",
            ],
            AsiaSoutheast2 => &[
                "asia-southeast2-a",
                "asia-southeast2-b",
                "asia-southeast2-c",
            ],
            AustraliaSoutheast1 => &[
                "australia-southeast1-a",
                "australia-southeast1-b",
                "australia-southeast1-c",
            ],
            AustraliaSoutheast2 => &[
                "australia-southeast2-a",
                "australia-southeast2-b",
                "australia-southeast2-c",
            ],
            // Europe
            EuropeCentral2 => &[
                "europe-central2-a",
                "europe-central2-b",
                "europe-central2-c",
            ],
            EuropeNorth1 => &["europe-north1-a", "europe-north1-b", "europe-north1-c"],
            EuropeSouthwest1 => &[
                "europe-southwest1-a",
                "europe-southwest1-b",
                "europe-southwest1-c",
            ],
            EuropeWest1 => &["europe-west1-b", "europe-west1-c", "europe-west1-d"],
            EuropeWest2 => &["europe-west2-a", "europe-west2-b", "europe-west2-c"],
            EuropeWest3 => &["europe-west3-a", "europe-west3-b", "europe-west3-c"],
            EuropeWest4 => &["europe-west4-a", "europe-west4-b", "europe-west4-c"],
            EuropeWest6 => &["europe-west6-a", "europe-west6-b", "europe-west6-c"],
            EuropeWest8 => &["europe-west8-a", "europe-west8-b", "europe-west8-c"],
            EuropeWest9 => &["europe-west9-a", "europe-west9-b", "europe-west9-c"],
            EuropeWest10 => &["europe-west10-a", "europe-west10-b", "europe-west10-c"],
            EuropeWest12 => &["europe-west12-a", "europe-west12-b", "europe-west12-c"],
            // Middle East
            MeCentral1 => &["me-central1-a", "me-central1-b", "me-central1-c"],
            MeCentral2 => &["me-central2-a", "me-central2-b", "me-central2-c"],
            MeWest1 => &["me-west1-a", "me-west1-b", "me-west1-c"],
        }
    }

    /// Specialized Google Cloud AI zones for this region.
    pub fn ai_zones(self) -> &'static [&'static str] {
        use Region::*;

        match self {
            UsCentral1 => &["us-central1-ai1a"],
            UsSouth1 => &["us-south1-ai1b"],
            AfricaSouth1
            | AsiaEast1
            | AsiaEast2
            | AsiaNortheast1
            | AsiaNortheast2
            | AsiaNortheast3
            | AsiaSouth1
            | AsiaSouth2
            | AsiaSoutheast1
            | AsiaSoutheast2
            | AustraliaSoutheast1
            | AustraliaSoutheast2
            | EuropeCentral2
            | EuropeNorth1
            | EuropeSouthwest1
            | EuropeWest1
            | EuropeWest2
            | EuropeWest3
            | EuropeWest4
            | EuropeWest6
            | EuropeWest8
            | EuropeWest9
            | EuropeWest10
            | EuropeWest12
            | MeCentral1
            | MeCentral2
            | MeWest1
            | NorthAmericaNortheast1
            | NorthAmericaNortheast2
            | NorthAmericaSouth1
            | SouthAmericaEast1
            | SouthAmericaWest1
            | UsEast1
            | UsEast4
            | UsEast5
            | UsWest1
            | UsWest2
            | UsWest3
            | UsWest4 => &[],
        }
    }
}

impl Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.long())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn all_regions_have_long_name_matching_gcp_shape() {
        for region in Region::iter() {
            let long = region.long();

            assert!(
                long.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{long} should contain only lowercase ASCII letters, digits, and hyphens"
            );
            assert!(
                long.contains('-'),
                "{long} should use the standard GCP region shape"
            );
        }
    }

    #[test]
    fn all_regions_have_short_name_for_resource_naming() {
        for region in Region::iter() {
            let short = region.short();

            assert!(
                !short.is_empty(),
                "{} should have a short name",
                region.long()
            );
            assert!(
                short
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "{short} should contain only lowercase ASCII letters and digits"
            );
        }
    }

    #[test]
    fn all_regions_have_unique_cidr_shards() {
        let mut shards = Vec::new();

        for region in Region::iter() {
            let shard = region.cidr_shard();

            assert!(
                shard < 64,
                "{} should fit in a 6-bit CIDR shard",
                region.long()
            );
            assert!(
                !shards.contains(&shard),
                "{} should have a unique CIDR shard",
                region.long()
            );

            shards.push(shard);
        }
    }

    #[test]
    fn all_regions_have_at_least_three_standard_zones() {
        for region in Region::iter() {
            let zones = region.zones();

            assert!(
                zones.len() >= 3,
                "{} should have at least three standard zones",
                region.long()
            );
        }
    }

    #[test]
    fn standard_zones_match_their_region_prefix() {
        for region in Region::iter() {
            let region_name = region.long();

            for zone in region.zones() {
                assert!(
                    zone.starts_with(region_name),
                    "{zone} should start with region {region_name}"
                );
                assert!(!zone.contains("-ai"), "{zone} should not be an AI zone");
            }
        }
    }

    #[test]
    fn ai_zones_are_separate_from_standard_zones() {
        for region in Region::iter() {
            for ai_zone in region.ai_zones() {
                assert!(
                    !region.zones().contains(ai_zone),
                    "{ai_zone} should not be part of standard availability_zones()"
                );
            }
        }
    }

    #[test]
    fn ai_zones_match_their_region_prefix() {
        for region in Region::iter() {
            let region_name = region.long();

            for ai_zone in region.ai_zones() {
                assert!(
                    ai_zone.starts_with(region_name),
                    "{ai_zone} should start with region {region_name}"
                );
                assert!(
                    ai_zone.contains("-ai"),
                    "{ai_zone} should use the AI zone naming shape"
                );
            }
        }
    }

    #[test]
    fn known_project_regions_are_supported() {
        assert_eq!(
            Region::NorthAmericaNortheast1.long(),
            "northamerica-northeast1"
        );
        assert_eq!(Region::NorthAmericaNortheast1.short(), "nane1");
        assert_eq!(
            Region::NorthAmericaNortheast1.zones(),
            &[
                "northamerica-northeast1-a",
                "northamerica-northeast1-b",
                "northamerica-northeast1-c",
            ]
        );

        assert_eq!(Region::UsCentral1.long(), "us-central1");
        assert_eq!(Region::UsCentral1.short(), "usc1");
        assert_eq!(
            Region::UsCentral1.zones(),
            &[
                "us-central1-a",
                "us-central1-b",
                "us-central1-c",
                "us-central1-f",
            ]
        );
    }

    #[test]
    fn known_ai_zones_are_supported() {
        assert_eq!(Region::UsCentral1.ai_zones(), &["us-central1-ai1a"]);
        assert_eq!(Region::UsSouth1.ai_zones(), &["us-south1-ai1b"]);
        assert!(Region::NorthAmericaNortheast1.ai_zones().is_empty());
    }

    #[test]
    fn display_uses_long_name() {
        assert_eq!(
            Region::NorthAmericaNortheast1.to_string(),
            "northamerica-northeast1"
        );
    }
}
