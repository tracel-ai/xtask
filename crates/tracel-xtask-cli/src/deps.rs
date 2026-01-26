/// Sync dependency versions from a monorepo “source of truth” Cargo.toml into subrepo Cargo.toml
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

#[derive(Debug, Clone, Default)]
struct CanonicalDep {
    version: Option<String>,
    features: Option<Vec<String>>,
    path: Option<String>,
    default_features: Option<bool>,

    git: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    branch: Option<String>,
    package: Option<String>,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub changed_manifests: Vec<PathBuf>,
    pub unchanged_manifests: Vec<PathBuf>,
    pub missing_manifests: Vec<PathBuf>,
    pub updated_dependencies: usize,
    pub missing_canonical_dependencies: Vec<(PathBuf, String, String)>,
}

/// Sync all subrepo manifests by pushing canonical dependency fields from the root manifest.
/// Returns a report listing changed/unchanged/missing manifests and any missing canonical deps.
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

        // Prefix needed to rebase root-relative paths into this subrepo.
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

/// Sync one Cargo.toml by updating only explicitly declared dependencies.
/// Writes the file back if its textual representation changes.
fn sync_one_manifest(
    manifest_path: &Path,
    canonical: &HashMap<String, CanonicalDep>,
    root_prefix: Option<&Path>,
) -> Result<SyncReport> {
    let before = fs::read_to_string(manifest_path)
        .map_err(|e| format!("failed to read {}: {e}", manifest_path.display()))?;

    let mut doc = before
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse TOML {}: {e}", manifest_path.display()))?;

    let mut report = SyncReport::default();
    let mut updated = 0usize;

    // Prefer subrepo workspace dependencies if present.
    updated += sync_subrepo_workspace_dependencies(
        doc.as_table_mut(),
        canonical,
        manifest_path,
        &mut report,
        root_prefix,
    );

    // Also sync top-level dependency tables when present.
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

    // Sync target-specific dependency tables.
    updated += sync_target_dep_tables(
        doc.as_table_mut(),
        canonical,
        manifest_path,
        &mut report,
        root_prefix,
    );

    let after = doc.to_string();
    let changed = after != before;

    report.updated_dependencies = updated;

    if changed {
        fs::write(manifest_path, after)
            .map_err(|e| format!("failed to write {}: {e}", manifest_path.display()))?;
        report.changed_manifests.push(manifest_path.to_path_buf());
    } else {
        report.unchanged_manifests.push(manifest_path.to_path_buf());
    }

    Ok(report)
}

/// Sync the `[workspace.dependencies]` table of a subrepo (if it exists).
/// Returns the number of dependency entries updated.
fn sync_subrepo_workspace_dependencies(
    root: &mut Table,
    canonical: &HashMap<String, CanonicalDep>,
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

/// Parse the root manifest and extract canonical dependency fields from `[workspace.dependencies]`.
/// Supports string and inline-table dependency specs but not expanded tables.
fn read_canonical_deps(root_manifest_path: &Path) -> Result<HashMap<String, CanonicalDep>> {
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

    let mut out: HashMap<String, CanonicalDep> = HashMap::new();
    for (dep_name, item) in ws_deps.iter() {
        let canon = extract_canonical_dep(item);
        if canon.version.is_some()
            || canon.features.is_some()
            || canon.path.is_some()
            || canon.default_features.is_some()
            || canon.git.is_some()
            || canon.tag.is_some()
            || canon.rev.is_some()
            || canon.branch.is_some()
            || canon.package.is_some()
        {
            out.insert(dep_name.to_string(), canon);
        }
    }

    Ok(out)
}

/// Convert a dependency spec item from the root `[workspace.dependencies]` table into `CanonicalDep`.
/// Supported forms: `dep = "..."` and `dep = { ... }`
fn extract_canonical_dep(item: &Item) -> CanonicalDep {
    // dep = "1.2.3"
    if let Some(v) = item.as_value().and_then(|v| v.as_str()) {
        return CanonicalDep {
            version: Some(v.to_string()),
            ..CanonicalDep::default()
        };
    }

    // dep = { ... } inline table
    if let Some(inline) = item.as_inline_table() {
        return CanonicalDep {
            version: inline
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            features: extract_features_from_value(inline.get("features")),
            path: inline
                .get("path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            default_features: inline.get("default-features").and_then(|v| v.as_bool()),

            git: inline
                .get("git")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            tag: inline
                .get("tag")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            rev: inline
                .get("rev")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            branch: inline
                .get("branch")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            package: inline
                .get("package")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        };
    }

    // Expanded canonical form ignored by design (inline-only policy).
    CanonicalDep::default()
}

/// Read `features = ["..."]` into a string vector.
/// Returns None if the value is missing or not an array of strings.
fn extract_features_from_value(v: Option<&Value>) -> Option<Vec<String>> {
    let arr = v?.as_array()?;
    let mut out = Vec::new();
    for val in arr.iter() {
        let s = val.as_str()?;
        out.push(s.to_string());
    }
    Some(out)
}

/// Sync all dependency tables under `[target.*]` (dependencies/dev-dependencies/build-dependencies).
/// Returns the number of dependency entries updated.
fn sync_target_dep_tables(
    root: &mut Table,
    canonical: &HashMap<String, CanonicalDep>,
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

/// Sync a single dependency table (e.g. `dependencies`) within `root`.
/// Only dependencies already present in the table are considered.
/// Missing canonical entries are recorded in the report.
fn sync_dep_table(
    root: &mut Table,
    table_name: &str,
    canonical: &HashMap<String, CanonicalDep>,
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

    let mut updated = 0usize;
    let keys: Vec<String> = deps_table.iter().map(|(k, _)| k.to_string()).collect();

    for dep in keys {
        match canonical.get(&dep) {
            Some(canon) => {
                if let Some(dep_item) = deps_table.get_mut(&dep) {
                    if update_dep_item_inline_only(dep_item, canon, root_prefix) {
                        updated += 1;
                    }
                }
            }
            None => {
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
            }
        }
    }

    updated
}

/// Update one dependency entry using inline-only rules:
/// - string shorthand stays shorthand unless canonical requires inline fields
/// - inline table is updated in-place
/// - `workspace = true` entries are left untouched
fn update_dep_item_inline_only(
    dep_item: &mut Item,
    canon: &CanonicalDep,
    root_prefix: Option<&Path>,
) -> bool {
    // dep = "..." (string shorthand)
    if dep_item.as_value().and_then(|v| v.as_str()).is_some() {
        let needs_inline = canon_needs_inline(canon);

        if needs_inline {
            let inline = build_inline_from_canonical(canon, root_prefix);
            *dep_item = Item::Value(Value::InlineTable(inline));
            return true;
        }

        if let Some(version) = canon.version.as_deref() {
            *dep_item = value(version);
            return true;
        }

        return false;
    }

    // dep = { ... } (inline table)
    if let Some(inline) = dep_item.as_inline_table_mut() {
        if inline
            .get("workspace")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return false;
        }

        let mut changed = false;

        if let Some(version) = canon.version.as_deref() {
            changed |= set_inline_table_string(inline, "version", version);
        }
        if let Some(features) = canon.features.as_ref() {
            changed |= set_inline_table_features(inline, features);
        }
        if let Some(path) = canon.path.as_deref() {
            let rebased = rebase_path_for_subrepo(path, root_prefix);
            changed |= set_inline_table_string(inline, "path", rebased.as_str());
        }
        if let Some(df) = canon.default_features {
            changed |= set_inline_table_bool(inline, "default-features", df);
        }

        if let Some(git) = canon.git.as_deref() {
            changed |= set_inline_table_string(inline, "git", git);
        }
        if let Some(tag) = canon.tag.as_deref() {
            changed |= set_inline_table_string(inline, "tag", tag);
        }
        if let Some(rev) = canon.rev.as_deref() {
            changed |= set_inline_table_string(inline, "rev", rev);
        }
        if let Some(branch) = canon.branch.as_deref() {
            changed |= set_inline_table_string(inline, "branch", branch);
        }
        if let Some(package) = canon.package.as_deref() {
            changed |= set_inline_table_string(inline, "package", package);
        }

        return changed;
    }

    false
}

/// Return true if canonical requires an inline table (anything beyond `version`).
fn canon_needs_inline(canon: &CanonicalDep) -> bool {
    canon.features.is_some()
        || canon.path.is_some()
        || canon.default_features.is_some()
        || canon.git.is_some()
        || canon.tag.is_some()
        || canon.rev.is_some()
        || canon.branch.is_some()
        || canon.package.is_some()
}

/// Build an inline dependency spec from canonical fields, rebasing `path` if needed.
fn build_inline_from_canonical(canon: &CanonicalDep, root_prefix: Option<&Path>) -> InlineTable {
    let mut inline = InlineTable::default();

    if let Some(version) = canon.version.as_deref() {
        inline.insert("version", Value::from(version));
    }
    if let Some(features) = canon.features.as_ref() {
        inline.insert("features", Value::Array(features_to_array(features)));
    }
    if let Some(path) = canon.path.as_deref() {
        let rebased = rebase_path_for_subrepo(path, root_prefix);
        inline.insert("path", Value::from(rebased.as_str()));
    }
    if let Some(df) = canon.default_features {
        inline.insert("default-features", Value::from(df));
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

/// Set/replace a string value in an inline table, returning true if it changed.
fn set_inline_table_string(inline: &mut InlineTable, key: &str, val: &str) -> bool {
    let current = inline.get(key).and_then(|v| v.as_str());
    if current == Some(val) {
        return false;
    }
    inline.insert(key, Value::from(val));
    true
}

/// Set/replace a bool value in an inline table, returning true if it changed.
fn set_inline_table_bool(inline: &mut InlineTable, key: &str, val: bool) -> bool {
    let current = inline.get(key).and_then(|v| v.as_bool());
    if current == Some(val) {
        return false;
    }
    inline.insert(key, Value::from(val));
    true
}

/// Set/replace `features = [...]` in an inline table, returning true if it changed.
fn set_inline_table_features(inline: &mut InlineTable, features: &[String]) -> bool {
    let desired = features_to_array(features);

    let current = inline.get("features").and_then(|v| v.as_array());
    if let Some(cur) = current {
        if arrays_equal_str(cur, &desired) {
            return false;
        }
    }

    inline.insert("features", Value::Array(desired));
    true
}

/// Convert a list of feature strings into a TOML array value.
fn features_to_array(features: &[String]) -> Array {
    let mut arr = Array::default();
    for f in features {
        arr.push(Value::from(f.as_str()));
    }
    arr
}

/// Return true if two TOML arrays contain identical string elements in the same order.
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
