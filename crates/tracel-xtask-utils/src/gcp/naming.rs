/// Define naming conventions for GCP resources.
///
/// Most Google Cloud resource IDs follow RFC 1035 style naming:
/// - 1-63 characters
/// - start with a lowercase letter
/// - contain lowercase letters, digits, and hyphens
/// - end with a lowercase letter or digit
use crate::environment::{Environment, ExplicitIndex};

use super::regions::Region;

/// GCP named resources.
#[derive(Debug, Clone, Copy)]
pub enum ResourceKind {
    // Load Balancing
    BackendService,
    ForwardingRule,
    GlobalAddress,
    HealthCheck,
    ManagedSslCertificate,
    RegionalAddress,
    TargetHttpProxy,
    TargetHttpsProxy,
    UrlMap,
    // Certificate Manager
    CertificateManagerCertificate,
    CertificateMap,
    CertificateMapEntry,
    DnsAuthorization,
    // Compute
    Disk,
    Firewall,
    Instance,
    InstanceTemplate,
    RegionInstanceGroupManager,
    // Network
    Network,
    Router,
    RouterNat,
    Route,
    Subnetwork,
    // IAM
    ServiceAccount,
    // DNS
    CloudDnsManagedZone,
    CloudDnsRecordSet,
    // Artifact Registry
    ArtifactRegistryRepository,
    // Secrets
    SecretManagerSecret,
    // Storage
    StorageBucket,
}

impl ResourceKind {
    /// Map each kind to a length cap.
    pub fn max_len(self) -> Option<usize> {
        use ResourceKind::*;

        Some(match self {
            // IAM service account IDs are stricter than most GCP resources.
            ServiceAccount => 30,
            // Secret Manager allows longer IDs.
            SecretManagerSecret => 255,
            // Keep buckets under the simple no-dot bucket-name limit.
            StorageBucket => 63,
            // Most Compute, Load Balancing, Cloud DNS, Certificate Manager, and
            // Artifact Registry resource IDs are safest as RFC 1035-style 63-char IDs.
            ArtifactRegistryRepository
            | BackendService
            | CertificateManagerCertificate
            | CertificateMap
            | CertificateMapEntry
            | CloudDnsManagedZone
            | Disk
            | DnsAuthorization
            | Firewall
            | ForwardingRule
            | GlobalAddress
            | HealthCheck
            | Instance
            | InstanceTemplate
            | ManagedSslCertificate
            | Network
            | RegionalAddress
            | RegionInstanceGroupManager
            | Route
            | Router
            | RouterNat
            | Subnetwork
            | TargetHttpProxy
            | TargetHttpsProxy
            | UrlMap => 63,
            // DNS record-set names are DNS names, not simple resource IDs.
            // The Terraform resource ID can be long and provider-specific.
            CloudDnsRecordSet => return None,
        })
    }

    /// Canonical short prefix used in Terraform IDs and GCP names.
    pub fn prefix(self) -> &'static str {
        use ResourceKind::*;

        match self {
            ArtifactRegistryRepository => "repo",
            BackendService => "backend",
            CertificateManagerCertificate => "cert",
            CertificateMap => "cert-map",
            CertificateMapEntry => "cert-map-entry",
            CloudDnsManagedZone => "dns-zone",
            CloudDnsRecordSet => "dns-record",
            Disk => "disk",
            DnsAuthorization => "dns-auth",
            Firewall => "fw",
            ForwardingRule => "fwd",
            GlobalAddress => "gaddr",
            HealthCheck => "hc",
            Instance => "inst",
            InstanceTemplate => "tmpl",
            ManagedSslCertificate => "ssl-cert",
            Network => "net",
            RegionalAddress => "raddr",
            RegionInstanceGroupManager => "mig",
            Route => "route",
            Router => "router",
            RouterNat => "nat",
            SecretManagerSecret => "secret",
            ServiceAccount => "sa",
            StorageBucket => "bucket",
            Subnetwork => "subnet",
            TargetHttpProxy => "http-proxy",
            TargetHttpsProxy => "https-proxy",
            UrlMap => "urlmap",
        }
    }
}

/// Build a sanitized GCP resource name with canonical prefix and per-kind limit.
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

    // Delegate to testable helpers.
    let name = build_name_parts(&env, app_prefix, base, res_prefix, region);
    let sanitized_name = sanitize_rfc1035(&name);
    let suffix = format!("-{res_idx}");

    match kind.max_len() {
        None => format!("{sanitized_name}{suffix}"),
        Some(max_len) => {
            let cutoff = max_len.saturating_sub(suffix.len());
            let trimmed = trim_to_rfc1035_boundary(&sanitized_name, cutoff);
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

/// Keep names valid for the common GCP RFC 1035 resource-ID profile:
/// `[a-z]([-a-z0-9]*[a-z0-9])?`.
///
/// This intentionally uses the stricter common denominator even for resource
/// kinds that allow `_`, `.`, or uppercase characters.
fn sanitize_rfc1035(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut previous_was_dash = false;

    for c in s.chars() {
        let next = if c.is_ascii_lowercase() || c.is_ascii_digit() {
            c
        } else if c.is_ascii_uppercase() {
            c.to_ascii_lowercase()
        } else {
            '-'
        };

        if next == '-' {
            if !previous_was_dash {
                out.push(next);
                previous_was_dash = true;
            }
        } else {
            out.push(next);
            previous_was_dash = false;
        }
    }

    let out = out.trim_matches('-');

    if out.is_empty() {
        return "x".to_string();
    }

    match out.as_bytes()[0] {
        b'a'..=b'z' => out.to_string(),
        _ => format!("x-{out}"),
    }
}

/// Trim to `max_len` chars without leaving the partial name empty or ending
/// with a hyphen.
fn trim_to_rfc1035_boundary(s: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    let mut trimmed: String = s.chars().take(max_len).collect();

    while trimmed.ends_with('-') {
        trimmed.pop();
    }

    if trimmed.is_empty() {
        "x".to_string()
    } else {
        trimmed
    }
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
                    assert_eq!(
                        got,
                        $expect,
                        "env={:?} app={:?} base={:?} kind={:?} region={:?}",
                        $env,
                        $app,
                        $base,
                        $kind,
                        $region
                    );
                    assert!(!got.contains("--"));
                }
            )*
        };
    }

    build_name_cases!(
        { empty_base_no_double_dash,
          env:"stg", app:"thyme", base:"", kind:"mig", region:"nane1",
          expect:"stg-thyme-mig-nane1"
        },
        { with_base_api,
          env:"stg", app:"thyme", base:"api", kind:"backend", region:"nane1",
          expect:"stg-thyme-api-backend-nane1"
        },
        { uppercased_inputs_lowercased_output,
          env:"STG", app:"THYME", base:"API", kind:"MIG", region:"NANE1",
          expect:"stg-thyme-api-mig-nane1"
        },
        { base_with_dash_is_kept_in_assembly,
          env:"stg", app:"thyme", base:"front-end", kind:"urlmap", region:"nane1",
          expect:"stg-thyme-front-end-urlmap-nane1"
        },
    );

    #[test]
    fn sanitize_replaces_invalid_chars() {
        let got = sanitize_rfc1035("Stg_Thyme.API@gaddr_NANE1");
        assert_eq!(got, "stg-thyme-api-gaddr-nane1");
    }

    #[test]
    fn sanitize_collapses_repeated_dashes() {
        let got = sanitize_rfc1035("stg---thyme___api");
        assert_eq!(got, "stg-thyme-api");
    }

    #[test]
    fn sanitize_forces_lowercase_letter_start() {
        let got = sanitize_rfc1035("123-thyme-api");
        assert_eq!(got, "x-123-thyme-api");
    }

    #[test]
    fn sanitize_never_returns_empty_name() {
        let got = sanitize_rfc1035("___---...");
        assert_eq!(got, "x");
    }

    #[test]
    fn trim_does_not_end_with_hyphen() {
        let got = trim_to_rfc1035_boundary("stg-thyme-api", 4);
        assert_eq!(got, "stg");
    }

    #[test]
    fn service_account_has_gcp_specific_limit() {
        assert_eq!(ResourceKind::ServiceAccount.max_len(), Some(30));
    }

    #[test]
    fn secret_manager_secret_has_gcp_specific_limit() {
        assert_eq!(ResourceKind::SecretManagerSecret.max_len(), Some(255));
    }

    #[test]
    fn dns_record_set_has_no_simple_resource_name_limit() {
        assert_eq!(ResourceKind::CloudDnsRecordSet.max_len(), None);
    }

    #[test]
    fn all_bounded_names_keep_room_for_suffix() {
        let kind = ResourceKind::RegionInstanceGroupManager;
        let suffix = "-123";
        let max_len = kind
            .max_len()
            .expect("GCP region instance group manager names should have a max length");

        let raw = sanitize_rfc1035("stg-thyme-this-is-a-very-long-component-that-will-be-trimmed-mig-nane1");
        let trimmed = trim_to_rfc1035_boundary(&raw, max_len - suffix.len());
        let name = format!("{trimmed}{suffix}");

        assert!(name.len() <= max_len);
        assert!(!name.ends_with('-'));
    }
}
