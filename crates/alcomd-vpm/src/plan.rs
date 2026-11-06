use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use alcomd_application::{
    M4Error, M4ErrorCode, PackageChangeSet, PackageDependencyEdge as ChangeSetEdge,
    PackageMutation, PackageMutationKind, PackagePlanDraft, PackageSourcePin, PlanAction,
    ProjectRecord, RepositoryPackageSourcePin, UserPackageSourcePin,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{PROJECT_MANIFEST_LIMIT, PackageDependencyEdge, Resolution, ResolvedPackage};

const TREE_ENTRY_LIMIT: usize = 65_536;
const TREE_FILE_LIMIT: u64 = 1_073_741_824;
const TREE_TOTAL_LIMIT: u64 = 4_294_967_296;

#[derive(Clone, Debug)]
pub struct ProjectPackageSnapshot {
    pub project_id: alcomd_application::ProjectId,
    pub project_revision: alcomd_application::Revision,
    pub root: PathBuf,
    pub path_identity_key: Vec<u8>,
    pub fingerprint: [u8; 32],
    vpm_manifest: Value,
    pub(crate) vpm_manifest_bytes: Vec<u8>,
}

impl ProjectPackageSnapshot {
    pub(super) fn resolution_rules(
        &self,
        explicit: BTreeSet<String>,
        preserve_unselected: bool,
    ) -> Result<crate::resolver::ProjectResolutionRules, M4Error> {
        use crate::range::{ProjectDependencyRange, VpmRange};
        use crate::resolver::{LockedPackage, ProjectResolutionRules};

        let invalid = || M4Error::new(M4ErrorCode::PackageManifestInvalid);
        let mut locked = BTreeMap::new();
        for (id, version) in vpm_version_map(self.vpm_manifest.get("locked"))? {
            let version: semver::Version = version
                .parse()
                .map_err(|cause| invalid().with_source(cause))?;
            let installed = preserve_unselected
                && installed_package_version(&self.root, &id)
                    .is_ok_and(|actual| actual.as_ref() == Some(&version));
            let mut dependencies = Vec::new();
            if let Some(value) = self.vpm_manifest["locked"][&id].get("dependencies") {
                for (package_id, range) in value.as_object().ok_or_else(invalid)? {
                    validate_package_id(package_id)?;
                    let range = VpmRange::parse(range.as_str().ok_or_else(invalid)?)
                        .map_err(|cause| invalid().with_source(cause))?;
                    dependencies.push(crate::PackageDependency {
                        package_id: package_id.clone(),
                        range: range.canonical(),
                    });
                }
            }
            locked.insert(
                id,
                LockedPackage {
                    version,
                    dependencies,
                    installed,
                },
            );
        }
        let direct = vpm_version_map(self.vpm_manifest.get("dependencies"))?
            .into_iter()
            .map(|(id, range)| {
                let range = ProjectDependencyRange::parse(&range)
                    .map_err(|cause| invalid().with_source(cause))?;
                let range = VpmRange::parse(&range.canonical())
                    .map_err(|cause| invalid().with_source(cause))?;
                Ok((id, range))
            })
            .collect::<Result<_, M4Error>>()?;
        Ok(ProjectResolutionRules {
            locked,
            direct,
            explicit,
            preserve_unselected,
        })
    }
}

pub fn inspect_package_project(project: &ProjectRecord) -> Result<ProjectPackageSnapshot, M4Error> {
    let requested_root = PathBuf::from(&project.observation.root_path);
    let (root, identity) = alcomd_platform::resolve_directory_identity(&requested_root)
        .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
    if identity != project.observation.path_identity_key {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_identity_changed",
        ));
    }
    let vpm_bytes = read_regular_component(
        &root,
        &["Packages", "vpm-manifest.json"],
        PROJECT_MANIFEST_LIMIT,
    )?;
    let upm_bytes = read_regular_component(
        &root,
        &["Packages", "manifest.json"],
        PROJECT_MANIFEST_LIMIT,
    )?;
    let vpm_manifest: Value = serde_json::from_slice(
        vpm_bytes
            .strip_prefix(b"\xEF\xBB\xBF")
            .unwrap_or(&vpm_bytes),
    )
    .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let object = vpm_manifest
        .as_object()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let actual_direct = vpm_version_map(object.get("dependencies"))?;
    let actual_locked = vpm_version_map(object.get("locked"))?;
    let observed_direct = project
        .observation
        .direct_dependencies
        .iter()
        .map(|value| (value.package_id.clone(), value.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let observed_locked = project
        .observation
        .locked_dependencies
        .iter()
        .map(|value| (value.package_id.clone(), value.value.clone()))
        .collect::<BTreeMap<_, _>>();
    if actual_direct != observed_direct || actual_locked != observed_locked {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_snapshot_changed",
        ));
    }

    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"alcomd-project-package-snapshot-v1");
    hash_field(&mut hasher, &identity);
    hash_field(&mut hasher, &vpm_bytes);
    hash_field(&mut hasher, &upm_bytes);
    for package_id in actual_locked.keys() {
        validate_package_id(package_id)?;
        hash_field(&mut hasher, package_id.as_bytes());
        let package_root = root.join("Packages").join(package_id);
        hash_field(&mut hasher, &tree_fingerprint(&package_root)?);
    }
    Ok(ProjectPackageSnapshot {
        project_id: project.project_id,
        project_revision: project.revision,
        root,
        path_identity_key: identity,
        fingerprint: hasher.finalize().into(),
        vpm_manifest,
        vpm_manifest_bytes: vpm_bytes,
    })
}

pub fn build_reinstall_plan(
    snapshot: &ProjectPackageSnapshot,
    resolution: &Resolution,
    targets: &BTreeSet<String>,
) -> Result<PackagePlanDraft, M4Error> {
    build_complete_resolution_plan(snapshot, PlanAction::Reinstall, resolution, targets, true)
}

pub fn build_bulk_plan(
    snapshot: &ProjectPackageSnapshot,
    resolution: &Resolution,
    reinstall_targets: &BTreeSet<String>,
) -> Result<PackagePlanDraft, M4Error> {
    build_complete_resolution_plan(
        snapshot,
        PlanAction::Bulk,
        resolution,
        reinstall_targets,
        false,
    )
}

fn build_complete_resolution_plan(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    resolution: &Resolution,
    force_replace: &BTreeSet<String>,
    preserve_manifest_bytes: bool,
) -> Result<PackagePlanDraft, M4Error> {
    let object = snapshot
        .vpm_manifest
        .as_object()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let old_direct = vpm_version_map(object.get("dependencies"))?;
    let old_locked = vpm_version_map(object.get("locked"))?;
    let mut new_direct = BTreeMap::new();
    let mut new_locked = BTreeMap::new();
    let mut selected = BTreeMap::new();
    for package in &resolution.packages {
        validate_package_id(&package.package_id)?;
        let version = package.version.to_string();
        new_locked.insert(package.package_id.clone(), version.clone());
        if package.direct {
            let direct_version = if old_locked.get(&package.package_id) == Some(&version)
                && (preserve_manifest_bytes || !force_replace.contains(&package.package_id))
            {
                old_direct
                    .get(&package.package_id)
                    .cloned()
                    .unwrap_or(version)
            } else {
                version
            };
            new_direct.insert(package.package_id.clone(), direct_version);
        }
        selected.insert(package.package_id.as_str(), package);
    }
    if preserve_manifest_bytes && (new_direct != old_direct || new_locked != old_locked) {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "reinstall_graph_changed",
        ));
    }

    let package_ids = old_locked
        .keys()
        .chain(new_locked.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut mutations = Vec::new();
    let mut source_set = Vec::new();
    for package_id in package_ids {
        let old = old_locked.get(&package_id);
        let new = new_locked.get(&package_id);
        match (old, new) {
            (Some(from), None) => mutations.push(PackageMutation {
                kind: PackageMutationKind::Remove,
                package_id,
                from_version: Some(from.clone()),
                to_version: None,
                source: None,
            }),
            (None, Some(to)) | (Some(_), Some(to))
                if old != Some(to) || force_replace.contains(&package_id) =>
            {
                let package = selected
                    .get(package_id.as_str())
                    .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
                let source = source_pin(package)?;
                mutations.push(PackageMutation {
                    kind: if old.is_some() {
                        PackageMutationKind::Replace
                    } else {
                        PackageMutationKind::Install
                    },
                    package_id,
                    from_version: old.cloned(),
                    to_version: Some(to.clone()),
                    source: Some(source.clone()),
                });
                source_set.push(source);
            }
            _ => {}
        }
    }

    for package in &resolution.packages {
        source_set.push(source_pin(package)?);
    }
    let manifest_bytes = if preserve_manifest_bytes {
        snapshot.vpm_manifest_bytes.clone()
    } else {
        let mut manifest = snapshot.vpm_manifest.clone();
        let object = manifest
            .as_object_mut()
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
        write_manifest_maps(
            object,
            &new_direct,
            &new_locked,
            &frozen_edges(&resolution.dependency_edges),
            &mutations,
            false,
        );
        serialize_manifest(&manifest)?
    };
    build_draft_from_bytes(
        snapshot,
        action,
        manifest_bytes,
        mutations,
        resolution.dependency_edges.clone(),
        source_set,
    )
}

pub fn build_resolution_plan(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    resolution: &Resolution,
) -> Result<PackagePlanDraft, M4Error> {
    if action == PlanAction::Remove {
        return Err(M4Error::new(M4ErrorCode::InvalidInput));
    }
    let mut manifest = snapshot.vpm_manifest.clone();
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let mut direct = vpm_version_map(object.get("dependencies"))?;
    let mut locked = vpm_version_map(object.get("locked"))?;
    let direct_ids = resolution
        .packages
        .iter()
        .filter(|package| package.direct)
        .map(|package| package.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut mutations = Vec::new();
    let mut source_set = Vec::new();
    for package in &resolution.packages {
        validate_package_id(&package.package_id)?;
        let target_version = package.version.to_string();
        let current_version = locked.get(&package.package_id).cloned();
        let version_changed = current_version.as_deref() != Some(target_version.as_str());
        let observed_version = if current_version.is_some()
            && (version_changed || action == PlanAction::Resolve || !package.direct)
        {
            Some(installed_package_version(
                &snapshot.root,
                &package.package_id,
            )?)
        } else {
            None
        };
        let needs_repair = observed_version
            .as_ref()
            .is_some_and(|actual| actual.as_ref() != Some(&package.version));
        if version_changed || needs_repair {
            let source = source_pin(package)?;
            let from_version = match observed_version {
                Some(actual) => actual.map(|version| version.to_string()),
                None => current_version,
            };
            mutations.push(PackageMutation {
                kind: if from_version.is_some() {
                    PackageMutationKind::Replace
                } else {
                    PackageMutationKind::Install
                },
                package_id: package.package_id.clone(),
                from_version,
                to_version: Some(target_version.clone()),
                source: Some(source.clone()),
            });
            source_set.push(source);
        }
        locked.insert(package.package_id.clone(), target_version.clone());
        if direct_ids.contains(package.package_id.as_str())
            && (!direct.contains_key(&package.package_id) || version_changed)
        {
            direct.insert(package.package_id.clone(), target_version);
        }
    }
    for package in &resolution.packages {
        source_set.push(source_pin(package)?);
    }
    let mut edges = resolution.dependency_edges.clone();
    for (id, range) in &direct {
        if !edges
            .iter()
            .any(|edge| edge.direct && edge.to_package_id == *id)
        {
            edges.push(PackageDependencyEdge {
                from_package_id: id.clone(),
                to_package_id: id.clone(),
                range: range.clone(),
                direct: true,
            });
        }
    }
    write_manifest_maps(
        object,
        &direct,
        &locked,
        &frozen_edges(&edges),
        &mutations,
        preserves_repaired_locks(action),
    );
    build_draft(snapshot, action, manifest, mutations, edges, source_set)
}

pub(crate) fn preserves_repaired_locks(action: PlanAction) -> bool {
    matches!(
        action,
        PlanAction::Install | PlanAction::Upgrade | PlanAction::Downgrade | PlanAction::Resolve
    )
}

fn installed_package_version(
    root: &Path,
    package_id: &str,
) -> Result<Option<semver::Version>, M4Error> {
    let directory = root.join("Packages").join(package_id);
    match std::fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {
            crate::package::extracted_package_version(&directory, package_id)
                .map(Some)
                .map_err(|cause| {
                    M4Error::new(M4ErrorCode::ProjectChangedDuringApply).with_source(cause)
                })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        _ => Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
    }
}

pub fn build_remove_plan(
    snapshot: &ProjectPackageSnapshot,
    package_id: &str,
    remaining: Option<&Resolution>,
) -> Result<PackagePlanDraft, M4Error> {
    validate_package_id(package_id)?;
    let mut manifest = snapshot.vpm_manifest.clone();
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let mut direct = vpm_version_map(object.get("dependencies"))?;
    let mut locked = vpm_version_map(object.get("locked"))?;
    if direct.remove(package_id).is_none() {
        return Err(M4Error::new(M4ErrorCode::PackageNotFound));
    }
    let selected = remaining
        .map(|resolution| {
            resolution
                .packages
                .iter()
                .map(|package| (package.package_id.as_str(), package))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut mutations = Vec::new();
    let mut source_set = Vec::new();
    for (installed_id, installed_version) in locked.clone() {
        match selected.get(installed_id.as_str()) {
            None => {
                locked.remove(&installed_id);
                mutations.push(PackageMutation {
                    kind: PackageMutationKind::Remove,
                    package_id: installed_id,
                    from_version: Some(installed_version),
                    to_version: None,
                    source: None,
                });
            }
            Some(package) if package.version.to_string() != installed_version => {
                let source = source_pin(package)?;
                locked.insert(installed_id.clone(), package.version.to_string());
                if direct.contains_key(&installed_id) {
                    direct.insert(installed_id.clone(), package.version.to_string());
                }
                mutations.push(PackageMutation {
                    kind: PackageMutationKind::Replace,
                    package_id: installed_id,
                    from_version: Some(installed_version),
                    to_version: Some(package.version.to_string()),
                    source: Some(source.clone()),
                });
                source_set.push(source);
            }
            Some(_) => {}
        }
    }
    for package in selected.values() {
        if !locked.contains_key(&package.package_id) {
            let source = source_pin(package)?;
            locked.insert(package.package_id.clone(), package.version.to_string());
            mutations.push(PackageMutation {
                kind: PackageMutationKind::Install,
                package_id: package.package_id.clone(),
                from_version: None,
                to_version: Some(package.version.to_string()),
                source: Some(source.clone()),
            });
            source_set.push(source);
        }
    }
    if let Some(resolution) = remaining {
        for package in &resolution.packages {
            source_set.push(source_pin(package)?);
        }
    }
    write_manifest_maps(
        object,
        &direct,
        &locked,
        &remaining
            .map(|resolution| frozen_edges(&resolution.dependency_edges))
            .unwrap_or_default(),
        &mutations,
        false,
    );
    build_draft(
        snapshot,
        PlanAction::Remove,
        manifest,
        mutations,
        remaining
            .map(|resolution| resolution.dependency_edges.clone())
            .unwrap_or_default(),
        source_set,
    )
}

pub fn materialize_vpm_manifest(
    current: &[u8],
    change_set: &PackageChangeSet,
) -> Result<Vec<u8>, M4Error> {
    materialize_plan_manifest(current, change_set, false)
}

pub(crate) fn materialize_plan_manifest(
    current: &[u8],
    change_set: &PackageChangeSet,
    preserve_repaired_locks: bool,
) -> Result<Vec<u8>, M4Error> {
    if !change_set.mutations.is_empty()
        && change_set.mutations.iter().all(|mutation| {
            mutation.kind == PackageMutationKind::Replace
                && mutation.from_version == mutation.to_version
        })
        && <[u8; 32]>::from(Sha256::digest(current)) == change_set.vpm_manifest_sha256
    {
        return Ok(current.to_vec());
    }
    let mut manifest: Value =
        serde_json::from_slice(current.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(current))
            .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let mut direct = vpm_version_map(object.get("dependencies"))?;
    let mut locked = vpm_version_map(object.get("locked"))?;
    let direct_ids = change_set
        .dependency_edges
        .iter()
        .filter(|edge| edge.direct)
        .map(|edge| edge.to_package_id.as_str())
        .collect::<BTreeSet<_>>();
    for mutation in &change_set.mutations {
        match mutation.kind {
            PackageMutationKind::Install | PackageMutationKind::Replace => {
                let version = mutation
                    .to_version
                    .as_ref()
                    .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
                let previous = locked.insert(mutation.package_id.clone(), version.clone());
                if direct_ids.contains(mutation.package_id.as_str())
                    && !(preserve_repaired_locks
                        && previous.as_ref() == Some(version)
                        && direct.contains_key(&mutation.package_id))
                {
                    direct.insert(mutation.package_id.clone(), version.clone());
                }
            }
            PackageMutationKind::Remove => {
                locked.remove(&mutation.package_id);
                direct.remove(&mutation.package_id);
            }
        }
    }
    if change_set.format_version < 3 {
        // Only already-accepted pre-remediation tasks may reach this path.
        // Recreate their frozen bytes; never re-sign them with the new writer.
        for (key, values) in [("dependencies", &direct), ("locked", &locked)] {
            object.insert(
                key.to_owned(),
                Value::Object(
                    values
                        .iter()
                        .map(|(id, version)| (id.clone(), serde_json::json!({"version":version})))
                        .collect(),
                ),
            );
        }
        let bytes = serialize_manifest(&manifest)?;
        if Sha256::digest(&bytes).as_slice() == change_set.vpm_manifest_sha256 {
            return Ok(bytes);
        }
        // Historical builds wrote direct strings. Accept only an exact match
        // to the task's original digest, never a newly computed authority.
        manifest.as_object_mut().expect("validated object").insert(
            "dependencies".to_owned(),
            Value::Object(
                direct
                    .iter()
                    .map(|(id, version)| (id.clone(), Value::String(version.clone())))
                    .collect(),
            ),
        );
        let strings = serialize_manifest(&manifest)?;
        if Sha256::digest(&strings).as_slice() == change_set.vpm_manifest_sha256 {
            return Ok(strings);
        }
        return Ok(bytes);
    }
    direct.retain(|id, _| direct_ids.contains(id.as_str()));
    for id in &direct_ids {
        if !direct.contains_key(*id)
            && let Some(version) = locked.get(*id)
        {
            direct.insert((*id).to_owned(), version.clone());
        }
    }
    write_manifest_maps(
        object,
        &direct,
        &locked,
        &change_set.dependency_edges,
        &change_set.mutations,
        preserve_repaired_locks,
    );
    serialize_manifest(&manifest)
}

pub(crate) fn attach_legacy_removals(
    snapshot: &ProjectPackageSnapshot,
    draft: &mut PackagePlanDraft,
    obsolete: BTreeSet<String>,
    removals: Vec<alcomd_application::LegacyRemoval>,
) -> Result<(), M4Error> {
    let change = &mut draft.change_set;
    let conflict = || M4Error::new(M4ErrorCode::PackageDependencyConflict);
    let locked = snapshot.vpm_manifest.get("locked");
    let direct = snapshot.vpm_manifest.get("dependencies");
    if change.mutations.iter().any(|mutation| {
        mutation.kind != PackageMutationKind::Remove && obsolete.contains(&mutation.package_id)
    }) || change.dependency_edges.iter().any(|edge| {
        !edge.direct
            && !obsolete.contains(&edge.from_package_id)
            && obsolete.contains(&edge.to_package_id)
    }) {
        return Err(conflict());
    }
    if let Some(entries) = locked.and_then(Value::as_object) {
        for (id, entry) in entries {
            if !obsolete.contains(id)
                && !change
                    .mutations
                    .iter()
                    .any(|mutation| &mutation.package_id == id)
                && entry
                    .get("dependencies")
                    .and_then(Value::as_object)
                    .is_some_and(|dependencies| {
                        dependencies
                            .keys()
                            .any(|dependency| obsolete.contains(dependency))
                    })
            {
                return Err(conflict());
            }
        }
    }
    for id in &obsolete {
        validate_package_id(id)?;
        if !change
            .mutations
            .iter()
            .any(|mutation| &mutation.package_id == id)
            && (direct.and_then(|entries| entries.get(id)).is_some()
                || locked.and_then(|entries| entries.get(id)).is_some())
        {
            change.mutations.push(PackageMutation {
                kind: PackageMutationKind::Remove,
                package_id: id.clone(),
                from_version: locked
                    .and_then(|entries| entries.get(id))
                    .and_then(|value| value.get("version"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                to_version: None,
                source: None,
            });
        }
    }
    for removal in &removals {
        let path = removal.path.to_ascii_lowercase();
        for mutation in &change.mutations {
            if mutation.kind == PackageMutationKind::Remove
                && obsolete.contains(&mutation.package_id)
            {
                continue;
            }
            let package = format!("packages/{}", mutation.package_id.to_ascii_lowercase());
            if path == package
                || path
                    .strip_prefix(&package)
                    .is_some_and(|suffix| suffix.starts_with('/'))
                || package
                    .strip_prefix(&path)
                    .is_some_and(|suffix| suffix.starts_with('/'))
            {
                return Err(conflict());
            }
        }
    }
    change
        .mutations
        .sort_by(|left, right| left.package_id.cmp(&right.package_id));
    change.dependency_edges.retain(|edge| {
        !obsolete.contains(&edge.from_package_id) && !obsolete.contains(&edge.to_package_id)
    });
    draft
        .source_set
        .retain(|source| !obsolete.contains(source.package_id()));
    change.format_version = 4;
    change.legacy_removals = removals;
    change.legacy_packages = obsolete.into_iter().collect();
    change.validate_bounds()?;
    change.vpm_manifest_sha256 = Sha256::digest(materialize_plan_manifest(
        &snapshot.vpm_manifest_bytes,
        change,
        preserves_repaired_locks(draft.action),
    )?)
    .into();
    draft.change_set_fingerprint = Sha256::digest(
        serde_json::to_vec(change).map_err(|_| M4Error::new(M4ErrorCode::Internal))?,
    )
    .into();
    Ok(())
}

fn build_draft(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    manifest: Value,
    mutations: Vec<PackageMutation>,
    dependency_edges: Vec<PackageDependencyEdge>,
    source_set: Vec<PackageSourcePin>,
) -> Result<PackagePlanDraft, M4Error> {
    build_draft_from_bytes(
        snapshot,
        action,
        serialize_manifest(&manifest)?,
        mutations,
        dependency_edges,
        source_set,
    )
}

fn build_draft_from_bytes(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    manifest_bytes: Vec<u8>,
    mut mutations: Vec<PackageMutation>,
    dependency_edges: Vec<PackageDependencyEdge>,
    mut source_set: Vec<PackageSourcePin>,
) -> Result<PackagePlanDraft, M4Error> {
    mutations.sort_by(|left, right| left.package_id.as_bytes().cmp(right.package_id.as_bytes()));
    source_set.sort_by(|left, right| {
        left.package_id()
            .as_bytes()
            .cmp(right.package_id().as_bytes())
            .then_with(|| left.version().as_bytes().cmp(right.version().as_bytes()))
            .then_with(|| {
                left.deterministic_source_key()
                    .cmp(&right.deterministic_source_key())
            })
    });
    source_set.dedup();
    let vpm_manifest_sha256 = Sha256::digest(&manifest_bytes).into();
    let mut edges = dependency_edges
        .into_iter()
        .map(|edge| ChangeSetEdge {
            from_package_id: edge.from_package_id,
            to_package_id: edge.to_package_id,
            range: edge.range,
            direct: edge.direct,
        })
        .collect::<Vec<_>>();
    edges.sort();
    edges.dedup();
    let change_set = PackageChangeSet {
        format_version: 3,
        mutations,
        dependency_edges: edges,
        legacy_removals: Vec::new(),
        legacy_packages: Vec::new(),
        vpm_manifest_sha256,
    };
    change_set.validate_bounds()?;
    let change_set_fingerprint = Sha256::digest(
        serde_json::to_vec(&change_set).map_err(|_| M4Error::new(M4ErrorCode::Internal))?,
    )
    .into();
    Ok(PackagePlanDraft {
        project_id: snapshot.project_id,
        action,
        project_revision: snapshot.project_revision,
        project_snapshot_fingerprint: snapshot.fingerprint,
        change_set_fingerprint,
        change_set,
        source_set,
    })
}

fn source_pin(package: &ResolvedPackage) -> Result<PackageSourcePin, M4Error> {
    let archive_sha256 = package
        .source
        .archive_sha256
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageHashRequired))?;
    Ok(match &package.source.authority {
        crate::PackageSourceAuthority::Repository {
            repository_id,
            repository_revision,
            artifact_url,
            credential_reference,
            ..
        } => PackageSourcePin::Repository(RepositoryPackageSourcePin {
            repository_id: repository_id.clone(),
            repository_revision: *repository_revision,
            source_identity: package.source.source_identity.clone(),
            manifest_fingerprint: package.source.manifest_fingerprint,
            package_id: package.package_id.clone(),
            version: package.version.to_string(),
            artifact_url: artifact_url.clone(),
            credential_reference: credential_reference.clone(),
            download_headers: package.source.download_headers(),
            archive_sha256,
        }),
        crate::PackageSourceAuthority::UserPackage {
            user_package_id,
            source_revision,
        } => PackageSourcePin::UserPackage(UserPackageSourcePin {
            user_package_id: *user_package_id,
            source_revision: *source_revision,
            source_identity: package.source.source_identity.clone(),
            manifest_fingerprint: package.source.manifest_fingerprint,
            package_id: package.package_id.clone(),
            version: package.version.to_string(),
            archive_sha256,
        }),
    })
}

fn frozen_edges(edges: &[PackageDependencyEdge]) -> Vec<ChangeSetEdge> {
    edges
        .iter()
        .map(|edge| ChangeSetEdge {
            from_package_id: edge.from_package_id.clone(),
            to_package_id: edge.to_package_id.clone(),
            range: edge.range.clone(),
            direct: edge.direct,
        })
        .collect()
}

fn write_manifest_maps(
    object: &mut Map<String, Value>,
    direct: &BTreeMap<String, String>,
    locked: &BTreeMap<String, String>,
    edges: &[ChangeSetEdge],
    mutations: &[PackageMutation],
    preserve_repaired_locks: bool,
) {
    // Preserve the original objects for entries untouched by this plan. A new
    // version receives only dependency evidence frozen by the solver.
    for (key, versions) in [("dependencies", direct), ("locked", locked)] {
        let previous = object.get(key).and_then(Value::as_object);
        let entries = versions
            .iter()
            .map(|(name, version)| {
                let old = previous
                    .and_then(|entries| entries.get(name))
                    .and_then(Value::as_object);
                let changed = key == "locked"
                    && !(preserve_repaired_locks
                        && old
                            .and_then(|entry| entry.get("version"))
                            .and_then(Value::as_str)
                            == Some(version.as_str()))
                    && mutations.iter().any(|mutation| {
                        mutation.package_id == *name && mutation.kind != PackageMutationKind::Remove
                    });
                let mut entry = if !changed {
                    old.cloned().unwrap_or_default()
                } else {
                    Map::new()
                };
                entry.insert("version".to_owned(), Value::String(version.clone()));
                if key == "locked" && (changed || old.is_none()) {
                    let dependencies = edges
                        .iter()
                        .filter(|edge| !edge.direct && edge.from_package_id == *name)
                        .map(|edge| {
                            (
                                edge.to_package_id.clone(),
                                Value::String(edge.range.clone()),
                            )
                        })
                        .collect::<Map<_, _>>();
                    entry.insert("dependencies".to_owned(), Value::Object(dependencies));
                }
                (name.clone(), Value::Object(entry))
            })
            .collect();
        object.insert(key.to_owned(), Value::Object(entries));
    }
}

fn serialize_manifest(value: &Value) -> Result<Vec<u8>, M4Error> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|_| M4Error::new(M4ErrorCode::Internal))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn vpm_version_map(value: Option<&Value>) -> Result<BTreeMap<String, String>, M4Error> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    object
        .iter()
        .map(|(name, value)| {
            validate_package_id(name)?;
            let version = value
                .as_object()
                .and_then(|value| value.get("version"))
                .and_then(Value::as_str)
                .filter(|version| !version.is_empty() && version.len() <= 1_024)
                .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
            Ok((name.clone(), version.to_owned()))
        })
        .collect()
}

fn validate_package_id(value: &str) -> Result<(), M4Error> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(M4Error::new(M4ErrorCode::InvalidInput));
    }
    Ok(())
}

pub(crate) fn read_regular_component(
    root: &Path,
    components: &[&str],
    limit: usize,
) -> Result<Vec<u8>, M4Error> {
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
        if is_link_or_reparse(&metadata) {
            return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
        }
    }
    let metadata = std::fs::metadata(&path)
        .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    let bytes =
        std::fs::read(&path).map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
    if bytes.len() > limit {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    Ok(bytes)
}

pub(super) fn tree_fingerprint(root: &Path) -> Result<[u8; 32], M4Error> {
    checked_tree_fingerprint(root, |_, _| Ok(()))
}

pub(super) fn checked_tree_fingerprint(
    root: &Path,
    check_entry: impl Fn(&Path, &Path) -> Result<(), M4Error>,
) -> Result<[u8; 32], M4Error> {
    let metadata = match std::fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Sha256::digest(b"missing-package-directory-v1").into());
        }
        Err(_) => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
    };
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    let mut pending = vec![PathBuf::new()];
    let mut paths = Vec::new();
    while let Some(relative) = pending.pop() {
        let directory = root.join(&relative);
        let mut children = std::fs::read_dir(&directory)
            .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?
            .map(|entry| {
                entry
                    .map(|entry| relative.join(entry.file_name()))
                    .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))
            })
            .collect::<Result<Vec<_>, _>>()?;
        children.sort();
        for child in children.into_iter().rev() {
            let name = child
                .file_name()
                .ok_or_else(|| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
            check_entry(&directory, Path::new(name))?;
            let metadata = std::fs::symlink_metadata(root.join(&child))
                .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
            if is_link_or_reparse(&metadata) {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
            if metadata.is_dir() {
                pending.push(child.clone());
            } else if !metadata.is_file() {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
            paths.push((child, metadata.is_dir(), metadata.len()));
            if paths.len() > TREE_ENTRY_LIMIT {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
        }
    }
    paths.sort_by(|left, right| left.0.cmp(&right.0));
    let mut total = 0_u64;
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"alcomd-package-tree-v1");
    for (relative, directory, size) in paths {
        let relative = relative
            .to_str()
            .ok_or_else(|| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
        hash_field(&mut hasher, relative.replace('\\', "/").as_bytes());
        hash_field(&mut hasher, if directory { b"d" } else { b"f" });
        if !directory {
            if size > TREE_FILE_LIMIT {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
            total = total
                .checked_add(size)
                .filter(|value| *value <= TREE_TOTAL_LIMIT)
                .ok_or_else(|| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
            check_entry(root, Path::new(relative))?;
            let bytes = std::fs::read(root.join(relative))
                .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
            hash_field(&mut hasher, &bytes);
        }
    }
    Ok(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_old_writer_uses_original_digest_without_resigning() {
        let current = br#"{"dependencies":{"com.example.a":{"version":"1.0.0"}},"locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"^1.0.0"}}}}"#;
        let historical = serde_json::json!({"dependencies":{"com.example.a":{"version":"1.0.0"}},"locked":{"com.example.a":{"version":"1.0.0"}}});
        let expected = serialize_manifest(&historical).expect("frozen historical output");
        let mut change_set = PackageChangeSet {
            format_version: 1,
            mutations: Vec::new(),
            legacy_removals: Vec::new(),
            legacy_packages: Vec::new(),
            dependency_edges: vec![ChangeSetEdge {
                from_package_id: "com.example.a".to_owned(),
                to_package_id: "com.example.a".to_owned(),
                range: "1.0.0".to_owned(),
                direct: true,
            }],
            vpm_manifest_sha256: Sha256::digest(&expected).into(),
        };
        assert_eq!(
            materialize_vpm_manifest(current, &change_set).expect("recover original output"),
            expected
        );
        let frozen = change_set.vpm_manifest_sha256;
        change_set.format_version = 3;
        let next = materialize_vpm_manifest(current, &change_set).expect("new output");
        assert_ne!(<[u8; 32]>::from(Sha256::digest(next)), frozen);
        assert_eq!(
            change_set.vpm_manifest_sha256, frozen,
            "materialization cannot re-sign authority"
        );
    }

    #[test]
    fn removal_preserves_unmodified_lock_metadata_and_matches_frozen_hash() {
        let original = serde_json::json!({
            "dependencies":{"com.example.a":{"version":"1.0.0","note":"keep"},"com.example.b":{"version":"1.0.0"}},
            "locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.c":"^1.2.x"},"other":{"value":7}},
                "com.example.b":{"version":"1.0.0","dependencies":{}},"com.example.c":{"version":"1.2.1","dependencies":{}}},
            "custom":{"value":"preserved"}
        });
        let snapshot = snapshot(original.clone());
        let remaining = Resolution {
            packages: vec![
                resolved("com.example.a", "1.0.0", true),
                resolved("com.example.c", "1.2.1", false),
            ],
            dependency_edges: vec![
                edge("com.example.a", "com.example.a", "1.0.0", true),
                edge("com.example.a", "com.example.c", "^1.2.x", false),
            ],
        };
        let plan =
            build_remove_plan(&snapshot, "com.example.b", Some(&remaining)).expect("remove B");
        let bytes = materialize_vpm_manifest(&snapshot.vpm_manifest_bytes, &plan.change_set)
            .expect("apply bytes");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            plan.change_set.vpm_manifest_sha256
        );
        let next: Value = serde_json::from_slice(&bytes).expect("manifest");
        assert_eq!(
            next["locked"]["com.example.a"],
            original["locked"]["com.example.a"]
        );
        assert_eq!(
            next["dependencies"]["com.example.a"],
            original["dependencies"]["com.example.a"]
        );
        assert!(next["locked"].get("com.example.b").is_none());
        assert_eq!(next["custom"], original["custom"]);
    }

    #[test]
    fn changed_lock_uses_frozen_edges_and_retained_transitive_root_is_removed() {
        let initial = serde_json::json!({"dependencies":{"com.example.a":{"version":"1.0.0"},"com.example.b":{"version":"1.0.0"}},
            "locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"1.0.0"}},"com.example.b":{"version":"1.0.0","dependencies":{}}}});
        let snapshot = snapshot(initial);
        let resolution = Resolution {
            packages: vec![
                resolved("com.example.a", "2.0.0", true),
                resolved("com.example.b", "1.0.0", false),
            ],
            dependency_edges: vec![
                edge("com.example.a", "com.example.a", "2.0.0", true),
                edge("com.example.a", "com.example.b", "=1.0.0", false),
            ],
        };
        let plan = build_remove_plan(&snapshot, "com.example.b", Some(&resolution))
            .expect("remove direct B but retain transitive");
        let bytes = materialize_vpm_manifest(&snapshot.vpm_manifest_bytes, &plan.change_set)
            .expect("apply bytes");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            plan.change_set.vpm_manifest_sha256
        );
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        assert!(value["dependencies"].get("com.example.b").is_none());
        assert_eq!(
            value["locked"]["com.example.a"]["dependencies"]["com.example.b"],
            "=1.0.0"
        );
        assert!(value["locked"].get("com.example.b").is_some());
    }

    fn snapshot(value: Value) -> ProjectPackageSnapshot {
        ProjectPackageSnapshot {
            project_id: alcomd_application::ProjectId::new(),
            project_revision: alcomd_application::Revision::new(1).expect("revision"),
            root: PathBuf::new(),
            path_identity_key: vec![1],
            fingerprint: [1; 32],
            vpm_manifest_bytes: serialize_manifest(&value).expect("original"),
            vpm_manifest: value,
        }
    }

    fn edge(from: &str, to: &str, range: &str, direct: bool) -> PackageDependencyEdge {
        PackageDependencyEdge {
            from_package_id: from.to_owned(),
            to_package_id: to.to_owned(),
            range: range.to_owned(),
            direct,
        }
    }

    fn resolved(id: &str, version: &str, direct: bool) -> ResolvedPackage {
        ResolvedPackage {
            package_id: id.to_owned(),
            version: semver::Version::parse(version).expect("version"),
            direct,
            source: crate::PackageSource {
                authority: crate::PackageSourceAuthority::Repository {
                    credential_reference: None,
                    download_headers_present: false,
                    repository_locator: None,
                    repository_id: "fixture".to_owned(),
                    repository_revision: 1,
                    priority: 1,
                    artifact_url: "https://fixtures.invalid/package.zip".to_owned(),
                },
                source_identity: "fixture".to_owned(),
                manifest_fingerprint: [1; 32],
                archive_sha256: Some([2; 32]),
            },
        }
    }

    #[test]
    fn materialized_vpm_versions_round_trip_as_objects() {
        let mut manifest = serde_json::json!({"preserved": true});
        let direct = BTreeMap::from([("com.example.direct".to_owned(), "^1.0.x".to_owned())]);
        let locked = BTreeMap::from([
            ("com.example.direct".to_owned(), "1.0.1".to_owned()),
            ("com.example.transitive".to_owned(), "2.0.0".to_owned()),
        ]);
        write_manifest_maps(
            manifest.as_object_mut().expect("object"),
            &direct,
            &locked,
            &[],
            &[],
            false,
        );
        assert_eq!(
            manifest["dependencies"]["com.example.direct"]["version"],
            "^1.0.x"
        );
        assert!(
            manifest["dependencies"]
                .get("com.example.transitive")
                .is_none()
        );
        assert_eq!(
            vpm_version_map(manifest.get("dependencies")).expect("direct"),
            direct
        );
        assert_eq!(
            vpm_version_map(manifest.get("locked")).expect("locked"),
            locked
        );
        let bytes = serialize_manifest(&manifest).expect("serialize");
        let value: Value = serde_json::from_slice(&bytes).expect("JSON");
        assert_eq!(value["preserved"], true);
        assert_eq!(
            crate::parse_vpm_dependencies(value.get("dependencies"))
                .expect("reader")
                .len(),
            1
        );
        assert_eq!(
            crate::parse_vpm_dependencies(value.get("locked"))
                .expect("reader")
                .len(),
            2
        );
        assert!(
            vpm_version_map(Some(&serde_json::json!({"com.example.direct": "1.0.0"}))).is_err()
        );
    }
}
