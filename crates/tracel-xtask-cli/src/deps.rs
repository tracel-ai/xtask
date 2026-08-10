/// Sync dependency specs between a root `Dependencies.toml` and subrepo `Cargo.toml` files.
use std::{
    collections::{HashMap, HashSet},
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
    pub added_canonical_dependencies: Vec<String>,
    pub conflicting_import_dependencies: Vec<(PathBuf, String, String)>,
    pub missing_canonical_dependencies: Vec<(PathBuf, String, String)>,
}

/// Normalized dependency spec
#[derive(Debug, Clone, Default, PartialEq)]
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

#[derive(Debug)]
struct ImportCandidate {
    name: String,
    spec: DepSpec,
    manifest_path: PathBuf,
    table_path: String,
}

struct ImportDiscovery<'a> {
    root_dir: &'a Path,
    canonical_names: &'a HashSet<String>,
    candidates: Vec<ImportCandidate>,
    candidate_positions: HashMap<String, usize>,
    report: &'a mut SyncReport,
}

/// Sync canonical fields into all subrepos provided, writing changes to disk.
pub fn sync_subrepos(root_manifest_path: &Path, subrepo_roots: &[PathBuf]) -> Result<SyncReport> {
    sync_subrepos_inner(root_manifest_path, subrepo_roots, false)
}

/// Import missing dependency source specs, then sync canonical fields into all subrepos.
pub fn sync_subrepos_two_way(
    root_manifest_path: &Path,
    subrepo_roots: &[PathBuf],
) -> Result<SyncReport> {
    sync_subrepos_inner(root_manifest_path, subrepo_roots, true)
}

fn sync_subrepos_inner(
    root_manifest_path: &Path,
    subrepo_roots: &[PathBuf],
    import_missing: bool,
) -> Result<SyncReport> {
    let mut report = SyncReport::default();

    let root_dir = root_manifest_path
        .parent()
        .ok_or_else(|| "root manifest should have a parent directory".to_string())?;

    if import_missing {
        let canonical_names = read_canonical_dep_names(root_manifest_path)?;
        let candidates =
            discover_import_candidates(subrepo_roots, root_dir, &canonical_names, &mut report)?;
        import_candidates(root_manifest_path, candidates, &mut report)?;
    }

    let canonical = read_canonical_deps(root_manifest_path)?;

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

/// Return all dependency names already declared in root `[workspace.dependencies]`.
fn read_canonical_dep_names(root_manifest_path: &Path) -> Result<HashSet<String>> {
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

    Ok(ws_deps.iter().map(|(name, _)| name.to_string()).collect())
}

/// Discover specs that can be imported from subrepo manifests.
fn discover_import_candidates(
    subrepo_roots: &[PathBuf],
    root_dir: &Path,
    canonical_names: &HashSet<String>,
    report: &mut SyncReport,
) -> Result<Vec<ImportCandidate>> {
    let mut discovery = ImportDiscovery {
        root_dir,
        canonical_names,
        candidates: Vec::new(),
        candidate_positions: HashMap::new(),
        report,
    };

    for subrepo_root in subrepo_roots {
        let manifest_path = subrepo_root.join("Cargo.toml");
        if !manifest_path.exists() {
            continue;
        }

        let contents = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("failed to read {}: {e}", manifest_path.display()))?;
        let doc = contents
            .parse::<DocumentMut>()
            .map_err(|e| format!("failed to parse TOML {}: {e}", manifest_path.display()))?;
        let manifest_dir = manifest_path.parent().ok_or_else(|| {
            format!(
                "manifest {} should have a parent directory",
                manifest_path.display()
            )
        })?;

        discovery.collect_manifest(doc.as_table(), &manifest_path, manifest_dir);
    }

    Ok(discovery.candidates)
}

impl ImportDiscovery<'_> {
    fn collect_manifest(&mut self, root: &Table, manifest_path: &Path, manifest_dir: &Path) {
        if let Some(workspace) = root.get("workspace").and_then(Item::as_table) {
            self.collect_dep_table(
                workspace,
                "dependencies",
                "workspace",
                manifest_path,
                manifest_dir,
            );
        }

        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            self.collect_dep_table(root, table_name, "", manifest_path, manifest_dir);
        }

        let Some(targets) = root.get("target").and_then(Item::as_table) else {
            return;
        };

        for (target_name, target_item) in targets.iter() {
            let Some(target) = target_item.as_table() else {
                continue;
            };
            let prefix = format!("target.{target_name}");
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                self.collect_dep_table(target, table_name, &prefix, manifest_path, manifest_dir);
            }
        }
    }

    fn collect_dep_table(
        &mut self,
        root: &Table,
        table_name: &str,
        prefix: &str,
        manifest_path: &Path,
        manifest_dir: &Path,
    ) {
        let Some(deps_table) = root.get(table_name).and_then(Item::as_table_like) else {
            return;
        };
        let table_path = if prefix.is_empty() {
            table_name.to_string()
        } else {
            format!("{prefix}.{table_name}")
        };

        for (name, item) in deps_table.iter() {
            if self.canonical_names.contains(name) {
                continue;
            }

            let Some(mut spec) = dep_spec_from_item(item) else {
                continue;
            };

            // Features are local policy and must never be inferred into Dependencies.toml.
            spec.features = None;
            spec.default_features = None;
            if let Some(path) = spec.path.take() {
                spec.path = Some(rebase_path_for_root(&path, manifest_dir, self.root_dir));
            }

            if spec.version.is_none() && !canon_requires_inline_for_source(&spec) {
                continue;
            }

            if let Some(position) = self.candidate_positions.get(name).copied() {
                if self.candidates[position].spec != spec {
                    self.report.conflicting_import_dependencies.push((
                        manifest_path.to_path_buf(),
                        table_path.clone(),
                        name.to_string(),
                    ));
                }
                continue;
            }

            self.candidate_positions
                .insert(name.to_string(), self.candidates.len());
            self.candidates.push(ImportCandidate {
                name: name.to_string(),
                spec,
                manifest_path: manifest_path.to_path_buf(),
                table_path: table_path.clone(),
            });
        }
    }
}

/// Append discovered specs to root `[workspace.dependencies]`, preserving root formatting.
fn import_candidates(
    root_manifest_path: &Path,
    candidates: Vec<ImportCandidate>,
    report: &mut SyncReport,
) -> Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }

    let before = fs::read_to_string(root_manifest_path)
        .map_err(|e| format!("failed to read {}: {e}", root_manifest_path.display()))?;
    let mut doc = before
        .parse::<DocumentMut>()
        .map_err(|e| format!("failed to parse TOML {}: {e}", root_manifest_path.display()))?;
    let ws_deps = doc
        .as_table_mut()
        .get_mut("workspace")
        .and_then(Item::as_table_mut)
        .and_then(|workspace| workspace.get_mut("dependencies"))
        .and_then(Item::as_table_like_mut)
        .ok_or_else(|| {
            format!(
                "root manifest {} must contain [workspace.dependencies]",
                root_manifest_path.display()
            )
        })?;

    for candidate in candidates {
        eprintln!(
            "  + {}: {} (from {} [{}])",
            candidate.name,
            fmt_dep_spec(&candidate.spec),
            candidate.manifest_path.display(),
            candidate.table_path,
        );
        ws_deps.insert(&candidate.name, dep_spec_to_item(&candidate.spec));
        report.added_canonical_dependencies.push(candidate.name);
    }

    fs::write(root_manifest_path, doc.to_string())
        .map_err(|e| format!("failed to write {}: {e}", root_manifest_path.display()))?;
    Ok(())
}

/// Render only the version and source-identifying keys of a dependency spec.
fn dep_spec_to_item(spec: &DepSpec) -> Item {
    if spec.version.is_some() && !canon_requires_inline_for_source(spec) {
        return value(spec.version.as_deref().unwrap());
    }

    Item::Value(Value::InlineTable(to_inline_table(spec, None)))
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

/// Parse features from an Item::Value holding an array.
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

        if let Some((from, to)) = apply_canonical_to_item(dep_item, canon, root_prefix) {
            updated += 1;
            eprintln!(
                "  - {}: {} → {}",
                dep,
                fmt_dep_spec(&from),
                fmt_dep_spec(&to),
            );
        }
    }

    updated
}

/// Apply canonical rules to a subrepo dependency item.
/// Returns Some((from, to)) when the semantic spec changed, otherwise None.
fn apply_canonical_to_item(
    dep_item: &mut Item,
    canon: &DepSpec,
    root_prefix: Option<&Path>,
) -> Option<(DepSpec, DepSpec)> {
    let before_spec = dep_spec_from_item(dep_item)?;

    // dep = "..." shorthand
    if dep_item.as_value().and_then(|v| v.as_str()).is_some() {
        // If canonical requires source keys, we must expand to inline.
        if canon_requires_inline_for_source(canon) {
            let inline = to_inline_table(canon, root_prefix);
            *dep_item = Item::Value(Value::InlineTable(inline));
        } else if let Some(version) = canon.version.as_deref() {
            // Otherwise keep shorthand and only apply canonical version if present.
            *dep_item = value(version);
        } else {
            // No canonical version and no canonical source keys: keep as-is.
        }

        let after_spec = dep_spec_from_item(dep_item)?;
        return if after_spec != before_spec {
            Some((before_spec, after_spec))
        } else {
            None
        };
    }

    // dep = { ... } inline table
    let inline = dep_item.as_inline_table_mut()?;

    // dep = { workspace = true } => do not touch
    if inline
        .get("workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }

    // version:
    // - set when canonical has one
    // - remove when canonical has no version AND canonical is source-based (path/git/...)
    match canon.version.as_deref() {
        Some(v) => {
            let _ = set_k_str(inline, "version", v);
        }
        None => {
            if canon_requires_inline_for_source(canon) {
                let _ = remove_key_if_present(inline, "version");
            }
        }
    }

    // features/default-features are authoritative only if root defines them
    if let Some(features) = canon.features.as_ref() {
        let _ = set_k_features(inline, features);
    }
    if let Some(df) = canon.default_features {
        let _ = set_k_bool(inline, "default-features", df);
    }

    // Source keys are authoritative: set when present in canonical, remove when absent
    let _ = sync_source_keys_inline(inline, canon, root_prefix);

    let after_spec = dep_spec_from_item(dep_item)?;
    if after_spec != before_spec {
        Some((before_spec, after_spec))
    } else {
        None
    }
}

/// Build a normalized DepSpec from a dependency TOML item.
/// Returns None when the item is not a supported dependency form.
fn dep_spec_from_item(item: &Item) -> Option<DepSpec> {
    // dep = "1.2.3"
    if let Some(v) = item.as_value().and_then(|v| v.as_str()) {
        return Some(DepSpec {
            version: Some(v.to_string()),
            ..DepSpec::default()
        });
    }

    // dep = { ... } inline table
    let inline = item.as_inline_table()?;

    // treat workspace deps as "unsupported for sync"
    if inline
        .get("workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return None;
    }

    Some(DepSpec {
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
    })
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

    let rebased = normalize_relative_path(&prefix.join(p));
    rebased.to_string_lossy().replace('\\', "/")
}

/// Rebase a subrepo-relative path so it is relative to `Dependencies.toml`.
fn rebase_path_for_root(subrepo_path: &str, manifest_dir: &Path, root_dir: &Path) -> String {
    let path = Path::new(subrepo_path);
    if path.is_absolute() {
        return subrepo_path.to_string();
    }

    let Ok(subrepo_prefix) = manifest_dir.strip_prefix(root_dir) else {
        return subrepo_path.to_string();
    };
    let rebased = normalize_relative_path(&subrepo_prefix.join(path));
    rebased.to_string_lossy().replace('\\', "/")
}

/// Lexically normalize `.` and `..` without requiring the dependency path to exist.
fn normalize_relative_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let can_pop = normalized
                    .file_name()
                    .is_some_and(|name| name != std::ffi::OsStr::new(".."));
                if can_pop {
                    normalized.pop();
                } else {
                    normalized.push("..");
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        normalized.push(".");
    }
    normalized
}

/// Format dependencies spec for logging
fn fmt_dep_spec(spec: &DepSpec) -> String {
    // version-only
    if spec.version.is_some()
        && spec.features.is_none()
        && spec.default_features.is_none()
        && spec.path.is_none()
        && spec.git.is_none()
        && spec.tag.is_none()
        && spec.rev.is_none()
        && spec.branch.is_none()
        && spec.package.is_none()
    {
        return spec.version.clone().unwrap();
    }

    let mut parts = Vec::new();

    if let Some(v) = &spec.version {
        parts.push(format!("version={v}"));
    }
    if let Some(p) = &spec.path {
        parts.push(format!("path={p}"));
    }
    if let Some(g) = &spec.git {
        parts.push(format!("git={g}"));
    }
    if let Some(t) = &spec.tag {
        parts.push(format!("tag={t}"));
    }
    if let Some(r) = &spec.rev {
        parts.push(format!("rev={r}"));
    }
    if let Some(b) = &spec.branch {
        parts.push(format!("branch={b}"));
    }
    if let Some(pkg) = &spec.package {
        parts.push(format!("package={pkg}"));
    }
    if let Some(df) = spec.default_features {
        parts.push(format!("default-features={df}"));
    }
    if let Some(f) = &spec.features {
        parts.push(format!("features={:?}", f));
    }

    format!("{{ {} }}", parts.join(", "))
}
