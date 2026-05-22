use super::*;

fn workspace(name: &str) -> Workspace {
    Workspace {
        path: PathBuf::from(name),
        dir_name: name.to_string(),
        xtask_bin: format!("xtask-{name}"),
        xtask: XtaskInvocation::WorkspaceMember {
            package: "xtask".to_string(),
        },
        toolchain: None,
    }
}

fn select<'a>(subrepos: &'a [Workspace], selector: &str) -> Result<&'a str, String> {
    select_subrepo_workspace_from_list(subrepos, selector).map(|ws| ws.dir_name.as_str())
}

#[test]
fn shorthand_uses_first_letter_of_each_name_segment() {
    assert_eq!(subrepo_shorthand("product-backend").as_deref(), Some("pb"));
    assert_eq!(
        subrepo_shorthand("burn-central-app").as_deref(),
        Some("bca")
    );
}

#[test]
fn shorthand_ignores_repeated_separators() {
    assert_eq!(subrepo_shorthand("product--backend").as_deref(), Some("pb"));
    assert_eq!(subrepo_shorthand("product_backend").as_deref(), Some("pb"));
    assert_eq!(subrepo_shorthand("product.backend").as_deref(), Some("pb"));
}

#[test]
fn exact_selector_matches_subrepo_name() {
    let subrepos = vec![workspace("product-backend"), workspace("frontend")];

    assert_eq!(
        select(&subrepos, "product-backend").expect("selector should match exact subrepo"),
        "product-backend"
    );
}

#[test]
fn prefix_selector_matches_unambiguous_prefix() {
    let subrepos = vec![workspace("product-backend"), workspace("frontend")];

    assert_eq!(
        select(&subrepos, "product").expect("selector should match prefix"),
        "product-backend"
    );
}

#[test]
fn prefix_selector_stays_ambiguous_before_trying_shorthand() {
    let subrepos = vec![
        workspace("product-backend"),
        workspace("product-frontend"),
        workspace("platform-build"),
    ];

    let err = select(&subrepos, "p").expect_err("selector should be ambiguous");

    assert!(err.contains("Ambiguous subrepo selector 'p'"));
    assert!(err.contains("product-backend"));
    assert!(err.contains("product-frontend"));
    assert!(err.contains("platform-build"));
}

#[test]
fn shorthand_selector_matches_unambiguous_shorthand() {
    let subrepos = vec![workspace("product-backend"), workspace("frontend")];

    assert_eq!(
        select(&subrepos, "pb").expect("selector should match shorthand"),
        "product-backend"
    );
}

#[test]
fn shorthand_selector_is_case_insensitive() {
    let subrepos = vec![workspace("product-backend"), workspace("frontend")];

    assert_eq!(
        select(&subrepos, "PB").expect("selector should match shorthand case-insensitively"),
        "product-backend"
    );
}

#[test]
fn shorthand_selector_fails_when_ambiguous() {
    let subrepos = vec![workspace("product-backend"), workspace("payment-broker")];

    let err = select(&subrepos, "pb").expect_err("shorthand selector should be ambiguous");

    assert!(err.contains("Ambiguous subrepo shorthand selector 'pb'"));
    assert!(err.contains("product-backend (:pb)"));
    assert!(err.contains("payment-broker (:pb)"));
}

#[test]
fn selector_fails_when_no_exact_prefix_or_shorthand_match_exists() {
    let subrepos = vec![workspace("product-backend"), workspace("frontend")];

    let err = select(&subrepos, "unknown").expect_err("selector should not match");

    assert_eq!(err, "No subrepo matches selector 'unknown'.");
}

#[test]
fn exact_selector_takes_precedence_over_prefix() {
    let subrepos = vec![workspace("product"), workspace("product-backend")];

    assert_eq!(
        select(&subrepos, "product").expect("selector should match exact subrepo"),
        "product"
    );
}
