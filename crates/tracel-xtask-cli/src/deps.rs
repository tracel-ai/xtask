/// Sync dependency specs from a monorepo “source of truth” Cargo.toml into subrepo Cargo.toml
/// files, updating only dependencies that are explicitly declared in each subrepo.
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table, Value, value};

pub type DynError = Box<dyn Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, DynError>;

#[derive(Debug, Default)]
pub struct SyncReport {
    pub changed_manifests: Vec<PathBuf>,
    pub unchanged_manifests: Vec<PathBuf>,
    pub missing_manifests: Vec<PathBuf>,
    pub updated_dependencies: usize,
    pub missing_canonical_dependencies: Vec<(PathBuf, String, String)>,
}

/// Normalized dependency spec
#[derive(Debug, Clone, Default)]
struct DepSpec {
    branch: Option<String>,
    default_features: Option<bool>,
    features: Option<Vec<String>>,
    git: Option<String>,
    package: Option<String>,
    path: Option<String>,
    rev: Option<String>,
    tag: Option<String>,
    version: Option<String>,
}

impl DepSpec {
    /// True if the spec requires inline representation
    fn needs_inline(&self) -> bool {
        self.features.is_some()
            || self.default_features.is_some()
            || self.path.is_some()
            || self.git.is_some()
            || self.tag.is_some()
            || self.rev.is_some()
            || self.branch.is_some()
            || self.package.is_some()
    }
}

/// Sync canonical fields into all subrepos provided, writing changes to disk.
pub fn sync_subrepos(root_manifest_path: &Path, subrepo_roots: &[PathBuf]) -> Result<SyncReport> {
    let canonical = read_canonical_deps(root_manifest_path)?;

    let root_dir = root_manifest_path
        .parent()
        .ok_or_else(|| "root manifest should have a parent directory".to_string())?;

    let mut report = SyncReport::default();

    for subrepo_root in subrepo_roots {
        let manifest_path = subrepo_root.join("Cargo.toml");
        if !manifest_path.exists() {
            report.missing_manifests.push(manifest_path);
            continue;
        }

        let manifest_dir = manifest_path.parent().ok_or_else(|| {
            format!(
                "manifest {} should have a parent directory",
                manifest_path.display()
            )
        })?;

        // Prefix used to rebase root-relative `path = "..."`
        let root_prefix = relative_prefix_to_ancestor(manifest_dir, root_dir);

        let r = sync_one_manifest(&manifest_path, &canonical, root_prefix.as_deref())?;
        report.changed_manifests.extend(r.changed_manifests);
        report.unchanged_manifests.extend(r.unchanged_manifests);
        report
            .missing_canonical_dependencies
            .extend(r.missing_canonical_dependencies);
        report.updated_dependencies += r.updated_dependencies;
    }

    Ok(report)
}

/// Sync a single Cargo.toml by applying canonical fields to dependency entries.
fn sync_one_manifest(
    manifest_path: &Path,
    canonical: &HashMap<String, DepSpec>,
    root_prefix: Option<&Path>,
) -> Result<SyncReport> {
    let before = fs::read_to_string(manifest_path)
        .map_err(|e| format!("failed to read {}: {e}", manifest_path.display()))?;

    let mut doc = before
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse TOML {}: {e}", manifest_path.display()))?;

    let mut report = SyncReport::default();
    let mut updated = 0usize;

    // [workspace.dependencies]
    updated += sync_subrepo_workspace_dependencies(
        doc.as_table_mut(),
        canonical,
        manifest_path,
        &mut report,
        root_prefix,
    );

    // also support top-level dependencies tables
    updated += sync_dep_table(
        doc.as_table_mut(),
        "dependencies",
        canonical,
        manifest_path,
        &mut report,
        "",
        root_prefix,
    );
    updated += sync_dep_table(
        doc.as_table_mut(),
        "dev-dependencies",
        canonical,
        manifest_path,
        &mut report,
        "",
        root_prefix,
    );
    updated += sync_dep_table(
        doc.as_table_mut(),
        "build-dependencies",
        canonical,
        manifest_path,
        &mut report,
        "",
        root_prefix,
    );

    // and [target.*.<dep-table>] sections
    updated += sync_target_dep_tables(
        doc.as_table_mut(),
        canonical,
        manifest_path,
        &mut report,
        root_prefix,
    );

    let after = doc.to_string();
    report.updated_dependencies = updated;

    if after != before {
        fs::write(manifest_path, after)
            .map_err(|e| format!("failed to write {}: {e}", manifest_path.display()))?;
        report.changed_manifests.push(manifest_path.to_path_buf());
    } else {
        report.unchanged_manifests.push(manifest_path.to_path_buf());
    }

    Ok(report)
}

/// Sync subrepo `[workspace.dependencies]` if present.
fn sync_subrepo_workspace_dependencies(
    root: &mut Table,
    canonical: &HashMap<String, DepSpec>,
    manifest_path: &Path,
    report: &mut SyncReport,
    root_prefix: Option<&Path>,
) -> usize {
    let Some(ws_item) = root.get_mut("workspace") else {
        return 0;
    };
    let Some(ws_table) = ws_item.as_table_mut() else {
        return 0;
    };

    sync_dep_table(
        ws_table,
        "dependencies",
        canonical,
        manifest_path,
        report,
        "workspace",
        root_prefix,
    )
}

/// Read canonical dependencies from root `[workspace.dependencies]`.
fn read_canonical_deps(root_manifest_path: &Path) -> Result<HashMap<String, DepSpec>> {
    let contents = fs::read_to_string(root_manifest_path)
        .map_err(|e| format!("failed to read {}: {e}", root_manifest_path.display()))?;

    let doc = contents
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse TOML {}: {e}", root_manifest_path.display()))?;

    let ws_deps = doc
        .as_table()
        .get("workspace")
        .and_then(Item::as_table)
        .and_then(|t| t.get("dependencies"))
        .and_then(Item::as_table)
        .ok_or_else(|| {
            format!(
                "root manifest {} must contain [workspace.dependencies]",
                root_manifest_path.display()
            )
        })?;

    let mut out: HashMap<String, DepSpec> = HashMap::new();
    for (dep_name, item) in ws_deps.iter() {
        let spec = parse_dep_item_inline_only(item);
        if spec.version.is_some() || spec.needs_inline() {
            out.insert(dep_name.to_string(), spec);
        }
    }

    Ok(out)
}

/// Parse a dependency item with the inline-only policy.
fn parse_dep_item_inline_only(item: &Item) -> DepSpec {
    // dep = "1.2.3"
    if let Some(v) = item.as_value().and_then(|v| v.as_str()) {
        return DepSpec {
            version: Some(v.to_string()),
            ..DepSpec::default()
        };
    }

    // dep = { ... } inline table
    if let Some(inline) = item.as_inline_table() {
        return DepSpec {
            version: inline
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string),

            features: parse_features(inline.get("features")),
            default_features: inline.get("default-features").and_then(|v| v.as_bool()),

            path: inline
                .get("path")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            git: inline
                .get("git")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            tag: inline
                .get("tag")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            rev: inline
                .get("rev")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            branch: inline
                .get("branch")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            package: inline
                .get("package")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        };
    }

    DepSpec::default()
}

/// Parse `features = ["..."]` into a Vec<String>.
fn parse_features(v: Option<&Value>) -> Option<Vec<String>> {
    let arr = v?.as_array()?;
    let mut out = Vec::new();
    for val in arr.iter() {
        out.push(val.as_str()?.to_string());
    }
    Some(out)
}

/// Sync dependency tables under `[target.*]` (dependencies/dev-dependencies/build-dependencies)
fn sync_target_dep_tables(
    root: &mut Table,
    canonical: &HashMap<String, DepSpec>,
    manifest_path: &Path,
    report: &mut SyncReport,
    root_prefix: Option<&Path>,
) -> usize {
    let Some(target_item) = root.get_mut("target") else {
        return 0;
    };
    let Some(target_table) = target_item.as_table_mut() else {
        return 0;
    };

    let mut updated = 0usize;

    for (target_key, per_target_item) in target_table.iter_mut() {
        let Some(per_target_table) = per_target_item.as_table_mut() else {
            continue;
        };

        let prefix = format!("target.{target_key}");

        updated += sync_dep_table(
            per_target_table,
            "dependencies",
            canonical,
            manifest_path,
            report,
            &prefix,
            root_prefix,
        );
        updated += sync_dep_table(
            per_target_table,
            "dev-dependencies",
            canonical,
            manifest_path,
            report,
            &prefix,
            root_prefix,
        );
        updated += sync_dep_table(
            per_target_table,
            "build-dependencies",
            canonical,
            manifest_path,
            report,
            &prefix,
            root_prefix,
        );
    }

    updated
}

/// Sync one dependency table by updating only dependencies already declared in that table.
fn sync_dep_table(
    root: &mut Table,
    table_name: &str,
    canonical: &HashMap<String, DepSpec>,
    manifest_path: &Path,
    report: &mut SyncReport,
    prefix: &str,
    root_prefix: Option<&Path>,
) -> usize {
    let Some(item) = root.get_mut(table_name) else {
        return 0;
    };

    let Some(deps_table) = item.as_table_like_mut() else {
        return 0;
    };

    let keys: Vec<String> = deps_table.iter().map(|(k, _)| k.to_string()).collect();
    let mut updated = 0usize;

    for dep in keys {
        let Some(dep_item) = deps_table.get_mut(&dep) else {
            continue;
        };

        let Some(canon) = canonical.get(&dep) else {
            let table_path = if prefix.is_empty() {
                table_name.to_string()
            } else {
                format!("{prefix}.{table_name}")
            };

            report.missing_canonical_dependencies.push((
                manifest_path.to_path_buf(),
                table_path,
                dep,
            ));
            continue;
        };

        if apply_canonical_to_item(dep_item, canon, root_prefix) {
            updated += 1;
        }
    }

    updated
}

/// Apply canonical rules to a subrepo dependency item.
/// Returns true if the TOML item was modified.
fn apply_canonical_to_item(
    dep_item: &mut Item,
    canon: &DepSpec,
    root_prefix: Option<&Path>,
) -> bool {
    // dep = "..." shorthand
    if dep_item.as_value().and_then(|v| v.as_str()).is_some() {
        // If canonical requires source keys, we must expand to inline.
        if canon_requires_inline_for_source(canon) {
            let inline = to_inline_table(canon, root_prefix);
            *dep_item = Item::Value(Value::InlineTable(inline));
            return true;
        }

        // Otherwise keep shorthand and only apply canonical version if present.
        if let Some(version) = canon.version.as_deref() {
            *dep_item = value(version);
            return true;
        }

        return false;
    }

    // dep = { ... } inline table
    let Some(inline) = dep_item.as_inline_table_mut() else {
        return false;
    };

    // dep = { workspace = true } => do not touch
    if inline
        .get("workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return false;
    }

    let mut changed = false;

    // version: canonical when present, removed when absent and source-based dep
    match canon.version.as_deref() {
        Some(version) => {
            changed |= set_k_str(inline, "version", version);
        }
        None => {
            // If canonical switches to path/git/etc, version must disappear
            if canon_requires_inline_for_source(canon) {
                changed |= remove_key_if_present(inline, "version");
            }
        }
    }

    // features/default-features: authoritative only if root defines them
    if let Some(features) = canon.features.as_ref() {
        changed |= set_k_features(inline, features);
    }
    if let Some(df) = canon.default_features {
        changed |= set_k_bool(inline, "default-features", df);
    }

    // Source keys: canonical always (set when present, prune when absent)
    changed |= sync_source_keys_inline(inline, canon, root_prefix);

    // Collapse if now exactly `{ version = "..." }`
    if inline_is_version_only(inline)
        && let Some(v) = inline.get("version").and_then(|v| v.as_str())
    {
        *dep_item = value(v);
        return true;
    }

    changed
}

/// Return true if canonical includes any “source key” that forces inline form.
fn canon_requires_inline_for_source(canon: &DepSpec) -> bool {
    canon.path.is_some()
        || canon.git.is_some()
        || canon.tag.is_some()
        || canon.rev.is_some()
        || canon.branch.is_some()
        || canon.package.is_some()
}

/// Convert canonical spec to an inline table, rebasing `path` as needed.
fn to_inline_table(canon: &DepSpec, root_prefix: Option<&Path>) -> InlineTable {
    let mut inline = InlineTable::default();

    if let Some(version) = canon.version.as_deref() {
        inline.insert("version", Value::from(version));
    }

    // Root-defined features/default-features are authoritative (so include if present in root)
    if let Some(features) = canon.features.as_ref() {
        inline.insert("features", Value::Array(features_to_array(features)));
    }
    if let Some(df) = canon.default_features {
        inline.insert("default-features", Value::from(df));
    }

    if let Some(path) = canon.path.as_deref() {
        let rebased = rebase_path_for_subrepo(path, root_prefix);
        inline.insert("path", Value::from(rebased.as_str()));
    }

    if let Some(git) = canon.git.as_deref() {
        inline.insert("git", Value::from(git));
    }
    if let Some(tag) = canon.tag.as_deref() {
        inline.insert("tag", Value::from(tag));
    }
    if let Some(rev) = canon.rev.as_deref() {
        inline.insert("rev", Value::from(rev));
    }
    if let Some(branch) = canon.branch.as_deref() {
        inline.insert("branch", Value::from(branch));
    }
    if let Some(package) = canon.package.as_deref() {
        inline.insert("package", Value::from(package));
    }

    inline
}

/// Sync “source keys” on an inline table: set when present in canonical, remove when absent.
/// Returns true if anything changed.
fn sync_source_keys_inline(
    inline: &mut InlineTable,
    canon: &DepSpec,
    root_prefix: Option<&Path>,
) -> bool {
    let mut changed = false;

    // path property needs rebasing
    match canon.path.as_deref() {
        Some(p) => {
            let rebased = rebase_path_for_subrepo(p, root_prefix);
            changed |= set_k_str(inline, "path", rebased.as_str());
        }
        None => {
            changed |= remove_key_if_present(inline, "path");
        }
    }

    changed |= sync_opt_str(inline, "git", canon.git.as_deref());
    changed |= sync_opt_str(inline, "tag", canon.tag.as_deref());
    changed |= sync_opt_str(inline, "rev", canon.rev.as_deref());
    changed |= sync_opt_str(inline, "branch", canon.branch.as_deref());
    changed |= sync_opt_str(inline, "package", canon.package.as_deref());

    changed
}

/// Set string if Some, else remove key. Returns true if changed.
fn sync_opt_str(inline: &mut InlineTable, key: &str, desired: Option<&str>) -> bool {
    match desired {
        Some(v) => set_k_str(inline, key, v),
        None => remove_key_if_present(inline, key),
    }
}

/// Remove `key` if present, returning true if it was removed.
fn remove_key_if_present(inline: &mut InlineTable, key: &str) -> bool {
    if inline.get(key).is_some() {
        inline.remove(key);
        true
    } else {
        false
    }
}

/// Set a string key in an inline table, returning true if changed.
fn set_k_str(inline: &mut InlineTable, key: &str, val: &str) -> bool {
    if inline.get(key).and_then(|v| v.as_str()) == Some(val) {
        return false;
    }
    inline.insert(key, Value::from(val));
    true
}

/// Set a bool key in an inline table, returning true if changed.
fn set_k_bool(inline: &mut InlineTable, key: &str, val: bool) -> bool {
    if inline.get(key).and_then(|v| v.as_bool()) == Some(val) {
        return false;
    }
    inline.insert(key, Value::from(val));
    true
}

/// Set `features = [...]` in an inline table, returning true if changed.
fn set_k_features(inline: &mut InlineTable, features: &[String]) -> bool {
    let desired = features_to_array(features);

    let current = inline.get("features").and_then(|v| v.as_array());
    if let Some(cur) = current
        && arrays_equal_str(cur, &desired)
    {
        return false;
    }

    inline.insert("features", Value::Array(desired));
    true
}

/// Return true if the inline table contains exactly one key: `version`.
fn inline_is_version_only(inline: &InlineTable) -> bool {
    if inline.get("version").and_then(|v| v.as_str()).is_none() {
        return false;
    }

    for (k, _) in inline.iter() {
        // NOTE: `k` is a String here, so compare directly.
        if k != "version" {
            return false;
        }
    }

    true
}

/// Convert a list of features into a TOML array.
fn features_to_array(features: &[String]) -> Array {
    let mut arr = Array::default();
    for f in features {
        arr.push(Value::from(f.as_str()));
    }
    arr
}

/// Check equality of two TOML arrays containing strings (order-sensitive).
fn arrays_equal_str(a: &Array, b: &Array) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (va, vb) in a.iter().zip(b.iter()) {
        if va.as_str() != vb.as_str() {
            return false;
        }
    }
    true
}

/// Compute a relative prefix from `from_dir` up to `ancestor_dir` (e.g. `../../..`).
/// Returns None when `ancestor_dir` is not an ancestor of `from_dir`.
fn relative_prefix_to_ancestor(from_dir: &Path, ancestor_dir: &Path) -> Option<PathBuf> {
    let mut cur = from_dir;
    let mut prefix = PathBuf::new();

    while cur != ancestor_dir {
        let parent = cur.parent()?;
        prefix.push("..");
        cur = parent;
    }

    Some(prefix)
}

/// Rebase a root-relative `path` dependency into a subrepo by prefixing `root_prefix`.
/// Absolute paths are returned unchanged. Output uses forward slashes for Cargo.toml.
fn rebase_path_for_subrepo(canonical_path: &str, root_prefix: Option<&Path>) -> String {
    let Some(prefix) = root_prefix else {
        return canonical_path.to_string();
    };

    let p = Path::new(canonical_path);
    if p.is_absolute() {
        return canonical_path.to_string();
    }

    let rebased = prefix.join(p);
    rebased.to_string_lossy().replace('\\', "/")
}
