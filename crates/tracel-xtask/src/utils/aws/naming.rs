/// Define naming conventions for AWS resources
use crate::prelude::{Environment, ExplicitIndex};

use super::regions::Region;

/// AWS named resources.
#[derive(Debug, Clone, Copy)]
pub enum ResourceKind {
    // Load Balancing
    Alb,
    Nlb,
    TargetGroup,
    Listener,
    ListenerRule,
    // Autoscaling
    LaunchTemplate,
    AutoScalingGroup,
    SecurityGroup,
    SecurityGroupRule,
    // IAM
    IamRole,
    IamInstanceProfile,
    IamRolePolicy,
    // DNS,
    Route53Zone,
    Route53Record,
    // Compute
    Instance,
    Volume,
    VolumeAttachment,
    // Network,
    Eip,
    InternetGateway,
    NatGateway,
    Route,
    RouteTable,
    RouteTableAssociation,
    Subnet,
    Vpc,
    // Data
    DataAmi,
    // Secrets
    SecretsManagerSecret,
    // Storage
    S3Bucket,
}

impl ResourceKind {
    /// Map each kind to a length cap
    pub fn max_len(self) -> Option<usize> {
        use ResourceKind::*;
        Some(match self {
            Alb | Nlb | TargetGroup => 32,
            S3Bucket => 63,
            IamRole => 64,
            LaunchTemplate | IamInstanceProfile | IamRolePolicy | Instance | Volume | Vpc
            | Subnet | SecretsManagerSecret => 128,
            AutoScalingGroup | InternetGateway | NatGateway | Eip | SecurityGroup => 255,
            DataAmi
            | Listener
            | ListenerRule
            | Route
            | Route53Record
            | Route53Zone
            | RouteTable
            | RouteTableAssociation
            | SecurityGroupRule
            | VolumeAttachment => return None,
        })
    }

    /// Canonical short prefix used in Terraform IDs and AWS names.
    pub fn prefix(self) -> &'static str {
        use ResourceKind::*;
        match self {
            Alb => "alb",
            AutoScalingGroup => "asg",
            DataAmi => "ami-data",
            Eip => "eip",
            IamInstanceProfile => "profile",
            IamRole => "role",
            IamRolePolicy => "policy",
            Instance => "inst",
            InternetGateway => "igw",
            LaunchTemplate => "lt",
            Listener => "listnr",
            ListenerRule => "lrule",
            NatGateway => "natgw",
            Nlb => "nlb",
            Route => "route",
            Route53Record => "r53rec",
            Route53Zone => "r53zone",
            RouteTable => "rt",
            RouteTableAssociation => "rta",
            S3Bucket => "bucket",
            SecurityGroup => "sg",
            SecurityGroupRule => "sgrule",
            Subnet => "subnet",
            TargetGroup => "tg",
            Volume => "vol",
            VolumeAttachment => "volatt",
            Vpc => "vpc",
            SecretsManagerSecret => "secret",
        }
    }
}

/// Build a sanitized resource name with canonical prefix and per-kind limit.
pub fn res_name(
    infra_env: &Environment<ExplicitIndex>,
    kind: ResourceKind,
    app_prefix: &str,
    region: &Region,
    base: &str,
    res_idx: usize,
) -> String {
    let env = infra_env.short();
    let res_prefix = kind.prefix();
    let region = region.short();

    // delegate to the testable helper
    let name = build_name_parts(&env, app_prefix, base, res_prefix, region);
    let sanitized_name = sanitize(&name);
    let suffix = format!("-{res_idx}");

    match kind.max_len() {
        None => format!("{}{}", sanitized_name, suffix),
        // TODO make the cutoff happen on the 'base' component only
        Some(max_len) => {
            let cutoff = max_len.saturating_sub(suffix.len());
            let trimmed: String = if sanitized_name.len() > cutoff {
                sanitized_name.chars().take(cutoff).collect()
            } else {
                sanitized_name
            };
            format!("{trimmed}{suffix}")
        }
    }
}

/// Join only non-empty parts to avoid double hyphens in names (--).
fn build_name_parts(
    env: &str,
    app_prefix: &str,
    base: &str,
    res_prefix: &str,
    region: &str,
) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(5);
    if !env.is_empty() {
        parts.push(env);
    }
    if !app_prefix.is_empty() {
        parts.push(app_prefix);
    }
    if !base.is_empty() {
        parts.push(base);
    }
    if !res_prefix.is_empty() {
        parts.push(res_prefix);
    }
    if !region.is_empty() {
        parts.push(region);
    }
    parts.join("-").to_ascii_lowercase()
}

/// Keep only [A-Za-z0-9-]
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! build_name_cases {
        ( $( { $name:ident, env:$env:expr, app:$app:expr, base:$base:expr, kind:$kind:expr, region:$region:expr, expect:$expect:expr } ),* $(,)? ) => {
            $(
                #[test]
                fn $name() {
                    let got = build_name_parts($env, $app, $base, $kind, $region);
                    assert_eq!(got, $expect, "env={:?} app={:?} base={:?} kind={:?} region={:?}", $env, $app, $base, $kind, $region);
                    // ensure we never produce a double hyphen in the raw assembly
                    assert!(!got.contains("--"));
                }
            )*
        };
    }

    build_name_cases!(
        { empty_base_no_double_dash,
          env:"stg", app:"burn", base:"", kind:"lrule", region:"ue1",
          expect:"stg-burn-lrule-ue1"
        },
        { with_base_api,
          env:"stg", app:"burn", base:"api", kind:"lrule", region:"ue1",
          expect:"stg-burn-api-lrule-ue1"
        },
        { uppercased_inputs_lowercased_output,
          env:"STG", app:"BURN", base:"API", kind:"LRULE", region:"UE1",
          expect:"stg-burn-api-lrule-ue1"
        },
        { base_with_dash_is_kept_in_assembly,
          env:"stg", app:"burn", base:"front-end", kind:"lrule", region:"ue1",
          expect:"stg-burn-front-end-lrule-ue1"
        },
    );
}
