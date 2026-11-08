use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use alcomd_application::{
    M4Error, M4ErrorCode, PackageChangeSet, PackageDependencyEdge as ChangeSetEdge,
    PackageMutation, PackageMutationKind, PackagePlanDraft, PackageRemovalConflict,
    PackageRemovalRequirement, PackageSourcePin, PlanAction, ProjectRecord,
    RepositoryPackageSourcePin, UserPackageSourcePin,
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
    pub(super) unlocked: BTreeMap<String, Vec<crate::PackageDependency>>,
    absent_packages: BTreeSet<String>,
}

impl ProjectPackageSnapshot {
    fn dependency_edges(&self) -> Result<Vec<PackageDependencyEdge>, M4Error> {
        let invalid = || M4Error::new(M4ErrorCode::PackageManifestInvalid);
        let mut edges = vpm_version_map(self.vpm_manifest.get("dependencies"))?
            .into_iter()
            .map(|(id, range)| PackageDependencyEdge {
                from_package_id: id.clone(),
                to_package_id: id,
                range,
                direct: true,
            })
            .collect::<Vec<_>>();
        for (id, entry) in self
            .vpm_manifest
            .get("locked")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if let Some(dependencies) = entry.get("dependencies") {
                for (target, range) in dependencies.as_object().ok_or_else(invalid)? {
                    validate_package_id(target)?;
                    let range = range
                        .as_str()
                        .filter(|range| !range.is_empty() && range.len() <= 1_024)
                        .ok_or_else(invalid)?;
                    edges.push(PackageDependencyEdge {
                        from_package_id: id.clone(),
                        to_package_id: target.clone(),
                        range: range.into(),
                        direct: false,
                    });
                }
            }
        }
        edges.extend(self.unlocked_edges());
        if edges.len() > alcomd_application::MAX_DEPENDENCY_EDGES {
            return Err(M4Error::new(M4ErrorCode::PlanTooLarge));
        }
        Ok(edges)
    }

    fn unlocked_edges(&self) -> Vec<PackageDependencyEdge> {
        self.unlocked
            .iter()
            .flat_map(|(id, dependencies)| {
                dependencies.iter().map(|dependency| PackageDependencyEdge {
                    from_package_id: id.clone(),
                    to_package_id: dependency.package_id.clone(),
                    range: dependency.range.clone(),
                    direct: false,
                })
            })
            .collect()
    }

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
            unlocked: self.unlocked.clone(),
        })
    }
}

pub fn inspect_package_project(project: &ProjectRecord) -> Result<ProjectPackageSnapshot, M4Error> {
    inspect_package_inputs(project, PackageSnapshotScope::Metadata)
}

pub(super) fn inspect_migration_package_project(
    project: &ProjectRecord,
) -> Result<ProjectPackageSnapshot, M4Error> {
    inspect_package_inputs(project, PackageSnapshotScope::Contents)
}

#[derive(Clone, Copy)]
enum PackageSnapshotScope {
    Metadata,
    Contents,
}

fn inspect_package_inputs(
    project: &ProjectRecord,
    scope: PackageSnapshotScope,
) -> Result<ProjectPackageSnapshot, M4Error> {
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
    hash_field(
        &mut hasher,
        match scope {
            PackageSnapshotScope::Metadata => b"alcomd-project-package-metadata-v3",
            PackageSnapshotScope::Contents => b"alcomd-project-package-snapshot-v1",
        },
    );
    hash_field(&mut hasher, &identity);
    hash_field(&mut hasher, &vpm_bytes);
    hash_field(&mut hasher, &upm_bytes);
    let mut absent_packages = BTreeSet::new();
    for package_id in actual_locked.keys() {
        validate_package_id(package_id)?;
        hash_field(&mut hasher, package_id.as_bytes());
        let package_root = root.join("Packages").join(package_id);
        match scope {
            PackageSnapshotScope::Metadata => {
                if !hash_package_metadata(&mut hasher, &package_root)?.present {
                    absent_packages.insert(package_id.clone());
                }
            }
            PackageSnapshotScope::Contents => {
                hash_field(&mut hasher, &tree_fingerprint(&package_root)?)
            }
        }
    }
    let unlocked = match scope {
        PackageSnapshotScope::Metadata => inspect_unlocked(&root, &actual_locked, &mut hasher)?,
        PackageSnapshotScope::Contents => BTreeMap::new(),
    };
    Ok(ProjectPackageSnapshot {
        project_id: project.project_id,
        project_revision: project.revision,
        root,
        path_identity_key: identity,
        fingerprint: hasher.finalize().into(),
        vpm_manifest,
        vpm_manifest_bytes: vpm_bytes,
        unlocked,
        absent_packages,
    })
}

// Planning depends on package identity and metadata, not the installed payload.
// Keep this bounded to one directory identity and one manifest per locked package.
// Payload bytes are moved atomically by Apply; they are not resolver inputs.
struct PackageMetadataInput {
    present: bool,
    manifest: Option<Vec<u8>>,
}

fn hash_package_metadata(
    hasher: &mut Sha256,
    root: &Path,
) -> Result<PackageMetadataInput, M4Error> {
    match std::fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hash_field(hasher, b"missing-package-directory");
            return Ok(PackageMetadataInput {
                present: false,
                manifest: None,
            });
        }
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
        _ => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
    }
    let identity = alcomd_platform::file_identity_key(root)
        .map_err(|cause| M4Error::new(M4ErrorCode::ProjectChangedDuringApply).with_source(cause))?;
    hash_field(hasher, &identity);
    let manifest = match std::fs::symlink_metadata(root.join("package.json")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hash_field(hasher, b"missing-package-manifest");
            None
        }
        Ok(_) => {
            hash_field(hasher, b"package-manifest");
            let bytes = read_regular_component(root, &["package.json"], PROJECT_MANIFEST_LIMIT)?;
            hash_field(hasher, &bytes);
            Some(bytes)
        }
        Err(_) => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
    };
    Ok(PackageMetadataInput {
        present: true,
        manifest,
    })
}

// Observe only direct package folders and their manifests, never their payload.
fn inspect_unlocked(
    root: &Path,
    locked: &BTreeMap<String, String>,
    hasher: &mut Sha256,
) -> Result<BTreeMap<String, Vec<crate::PackageDependency>>, M4Error> {
    let invalid = || M4Error::new(M4ErrorCode::PackageManifestInvalid);
    let packages = root.join("Packages");
    let entries = std::fs::read_dir(&packages).map_err(|cause| invalid().with_source(cause))?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|cause| invalid().with_source(cause))?;
        if paths.len() >= alcomd_application::MAX_PRESENTATION_PACKAGES {
            return Err(invalid());
        }
        paths.push(entry.path());
    }
    paths.sort();
    let mut total = 0usize;
    let mut unlocked = BTreeMap::new();
    let directory_key = |name: &str| {
        if cfg!(windows) {
            name.to_ascii_lowercase()
        } else {
            name.to_owned()
        }
    };
    let locked_directories = locked
        .keys()
        .map(|id| directory_key(id))
        .collect::<BTreeSet<_>>();
    for path in paths {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid)?;
        if locked_directories.contains(&directory_key(name)) {
            continue;
        }
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|cause| invalid().with_source(cause))?;
        if is_link_or_reparse(&metadata) {
            return Err(invalid());
        }
        if !metadata.is_dir() {
            continue;
        }
        hash_field(hasher, b"unlocked-package-folder");
        hash_field(hasher, name.as_bytes());
        let observed = hash_package_metadata(hasher, &path)?;
        if !observed.present {
            return Err(invalid());
        }
        let Some(bytes) = observed.manifest else {
            continue;
        };
        total = total.checked_add(bytes.len()).ok_or_else(invalid)?;
        if total > alcomd_application::MAX_PRESENTATION_BYTES {
            return Err(invalid());
        }
        let Ok(manifest) =
            crate::parse_bounded_json(&bytes, crate::M3ErrorCode::ProjectManifestInvalid)
        else {
            continue;
        };
        let Some(id) = manifest
            .get("name")
            .and_then(Value::as_str)
            .filter(|id| validate_package_id(id).is_ok())
        else {
            continue;
        };
        if !manifest
            .get("version")
            .and_then(Value::as_str)
            .is_some_and(|version| semver::Version::parse(version).is_ok())
        {
            continue;
        }
        if locked.contains_key(id) || unlocked.contains_key(id) {
            return Err(invalid());
        }
        let (dependencies, _) = crate::package::parse_dependencies(manifest.get("vpmDependencies"))
            .map_err(|cause| invalid().with_source(cause))?;
        unlocked.insert(id.to_owned(), dependencies);
    }
    Ok(unlocked)
}

pub fn build_reinstall_plan(
    snapshot: &ProjectPackageSnapshot,
    resolution: &Resolution,
    targets: &BTreeSet<String>,
) -> Result<PackagePlanDraft, M4Error> {
    let locked = vpm_version_map(snapshot.vpm_manifest.get("locked"))?;
    let selected = resolution
        .packages
        .iter()
        .map(|package| (package.package_id.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut mutations = Vec::new();
    let mut sources = Vec::new();
    for id in targets {
        let version = locked
            .get(id)
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotInstalled))?;
        let package = selected
            .get(id.as_str())
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotFound))?;
        if package.version.to_string() != *version {
            return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
        }
        let source = source_pin(package)?;
        mutations.push(PackageMutation {
            kind: PackageMutationKind::Replace,
            package_id: id.clone(),
            from_version: Some(version.clone()),
            to_version: Some(version.clone()),
            source: Some(source.clone()),
        });
        sources.push(source);
    }
    build_draft_from_bytes(
        snapshot,
        PlanAction::Reinstall,
        snapshot.vpm_manifest_bytes.clone(),
        mutations,
        snapshot.dependency_edges()?,
        sources,
    )
}

pub fn build_bulk_plan(
    snapshot: &ProjectPackageSnapshot,
    resolution: &Resolution,
    reinstall_targets: &BTreeSet<String>,
) -> Result<PackagePlanDraft, M4Error> {
    build_complete_resolution_plan(snapshot, PlanAction::Bulk, resolution, reinstall_targets)
}

fn build_complete_resolution_plan(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    resolution: &Resolution,
    force_replace: &BTreeSet<String>,
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
                && !force_replace.contains(&package.package_id)
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
    let manifest_bytes = serialize_manifest(&manifest)?;
    let mut edges = resolution.dependency_edges.clone();
    edges.extend(snapshot.unlocked_edges());
    build_draft_from_bytes(
        snapshot,
        action,
        manifest_bytes,
        mutations,
        edges,
        source_set,
    )
}

pub fn build_resolution_plan(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    resolution: &Resolution,
) -> Result<PackagePlanDraft, M4Error> {
    build_resolution_plan_with_ranges(snapshot, action, resolution, false)
}

pub(crate) fn build_template_resolution_plan(
    snapshot: &ProjectPackageSnapshot,
    resolution: &Resolution,
) -> Result<PackagePlanDraft, M4Error> {
    build_resolution_plan_with_ranges(snapshot, PlanAction::Resolve, resolution, true)
}

fn build_resolution_plan_with_ranges(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    resolution: &Resolution,
    preserve_input_ranges: bool,
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
            && (!direct.contains_key(&package.package_id)
                || (version_changed && !preserve_input_ranges))
        {
            direct.insert(package.package_id.clone(), target_version);
        }
    }
    for package in &resolution.packages {
        source_set.push(source_pin(package)?);
    }
    let mut edges = resolution.dependency_edges.clone();
    edges.extend(snapshot.unlocked_edges());
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
) -> Result<PackagePlanDraft, M4Error> {
    build_removal_plan(
        snapshot,
        &BTreeSet::from([package_id.to_owned()]),
        PlanAction::Remove,
    )
}

pub(crate) fn build_removal_plan(
    snapshot: &ProjectPackageSnapshot,
    removed: &BTreeSet<String>,
    action: PlanAction,
) -> Result<PackagePlanDraft, M4Error> {
    let mut manifest = snapshot.vpm_manifest.clone();
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let mut direct = vpm_version_map(object.get("dependencies"))?;
    let mut locked = vpm_version_map(object.get("locked"))?;
    for package_id in removed {
        validate_package_id(package_id)?;
        if !locked.contains_key(package_id) {
            return Err(M4Error::new(M4ErrorCode::PackageNotFound));
        }
        direct.remove(package_id);
    }
    let all_edges = snapshot.dependency_edges()?;
    let mut graph = BTreeMap::<&str, Vec<&str>>::new();
    for edge in all_edges.iter().filter(|edge| !edge.direct) {
        graph
            .entry(&edge.from_package_id)
            .or_default()
            .push(&edge.to_package_id);
    }
    // Only the selected packages' old dependency closure is eligible for cleanup.
    let removable = dependency_closure(removed.iter().cloned(), &graph, &BTreeSet::new());
    let retained = dependency_closure(
        direct.keys().chain(snapshot.unlocked.keys()).cloned(),
        &graph,
        removed,
    );
    let mut mutations = Vec::new();
    for (installed_id, installed_version) in locked.clone() {
        if removed.contains(&installed_id)
            || (removable.contains(&installed_id) && !retained.contains(&installed_id))
        {
            locked.remove(&installed_id);
            mutations.push(PackageMutation {
                kind: PackageMutationKind::Remove,
                package_id: installed_id,
                from_version: Some(installed_version),
                to_version: None,
                source: None,
            });
        }
    }
    let removed_ids = mutations
        .iter()
        .map(|mutation| mutation.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let edges = all_edges
        .into_iter()
        .filter(|edge| {
            if edge.direct {
                !removed_ids.contains(edge.to_package_id.as_str())
            } else {
                !removed_ids.contains(edge.from_package_id.as_str())
            }
        })
        .collect::<Vec<_>>();
    let removal_conflicts = removed_ids
        .iter()
        .filter_map(|id| {
            let required_by = edges
                .iter()
                .filter(|edge| !edge.direct && edge.to_package_id == *id)
                .map(|edge| PackageRemovalRequirement {
                    package_id: edge.from_package_id.clone(),
                    range: edge.range.clone(),
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            (!required_by.is_empty()).then(|| PackageRemovalConflict {
                package_id: (*id).to_owned(),
                required_by,
            })
        })
        .collect::<Vec<_>>();
    write_manifest_maps(
        object,
        &direct,
        &locked,
        &frozen_edges(&edges),
        &mutations,
        false,
    );
    let mut draft = build_draft(snapshot, action, manifest, mutations, edges, Vec::new())?;
    if !removal_conflicts.is_empty() {
        draft.change_set.format_version = 4;
        draft.change_set.removal_conflicts = removal_conflicts;
        draft.change_set.validate_bounds()?;
        draft.change_set_fingerprint = Sha256::digest(
            serde_json::to_vec(&draft.change_set)
                .map_err(|_| M4Error::new(M4ErrorCode::Internal))?,
        )
        .into();
    }
    Ok(draft)
}

fn dependency_closure(
    roots: impl IntoIterator<Item = String>,
    graph: &BTreeMap<&str, Vec<&str>>,
    excluded: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    let mut reached = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if excluded.contains(&id) || !reached.insert(id.clone()) {
            continue;
        }
        pending.extend(
            graph
                .get(id.as_str())
                .into_iter()
                .flatten()
                .map(|id| (*id).to_owned()),
        );
    }
    reached
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
    materialize_plan_manifest_with_ranges(current, change_set, preserve_repaired_locks, false)
}

pub(crate) fn materialize_template_manifest(
    current: &[u8],
    change_set: &PackageChangeSet,
) -> Result<Vec<u8>, M4Error> {
    materialize_plan_manifest_with_ranges(current, change_set, true, true)
}

fn materialize_plan_manifest_with_ranges(
    current: &[u8],
    change_set: &PackageChangeSet,
    preserve_repaired_locks: bool,
    preserve_input_ranges: bool,
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
                    && !(direct.contains_key(&mutation.package_id)
                        && (preserve_input_ranges
                            || (preserve_repaired_locks && previous.as_ref() == Some(version))))
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
    let absent_removal_packages = mutations
        .iter()
        .filter(|mutation| {
            mutation.kind == PackageMutationKind::Remove
                && snapshot.absent_packages.contains(&mutation.package_id)
        })
        .map(|mutation| mutation.package_id.clone())
        .collect::<Vec<_>>();
    let change_set = PackageChangeSet {
        absent_removal_packages: absent_removal_packages.clone(),
        unity_compatibility_overrides: Vec::new(),
        removal_conflicts: Vec::new(),
        format_version: if absent_removal_packages.is_empty() {
            3
        } else {
            4
        },
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
            unity_compatibility_overrides: Vec::new(),
            removal_conflicts: Vec::new(),
            absent_removal_packages: Vec::new(),
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
        let plan = build_remove_plan(&snapshot, "com.example.b").expect("remove B without sources");
        assert!(plan.source_set.is_empty());
        assert_eq!(plan.change_set.mutations.len(), 1);
        assert_eq!(plan.change_set.mutations[0].package_id, "com.example.b");
        assert_eq!(
            plan.change_set.mutations[0].kind,
            PackageMutationKind::Remove
        );
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
    fn removal_freezes_conflicts_and_removes_requested_package_without_changing_dependents() {
        let initial = serde_json::json!({"dependencies":{"com.example.a":{"version":"1.0.0"},"com.example.b":{"version":"1.0.0"}},
            "locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"1.0.0"}},"com.example.b":{"version":"1.0.0","dependencies":{}}}});
        let snapshot = snapshot(initial.clone());
        let plan = build_remove_plan(&snapshot, "com.example.b")
            .expect("review removal of required direct B");
        assert!(plan.source_set.is_empty());
        assert_eq!(plan.change_set.mutations.len(), 1);
        assert_eq!(plan.change_set.mutations[0].package_id, "com.example.b");
        assert_eq!(
            plan.change_set.mutations[0].kind,
            PackageMutationKind::Remove
        );
        assert_eq!(plan.change_set.format_version, 4);
        assert_eq!(
            plan.change_set.removal_conflicts,
            vec![PackageRemovalConflict {
                package_id: "com.example.b".into(),
                required_by: vec![PackageRemovalRequirement {
                    package_id: "com.example.a".into(),
                    range: "1.0.0".into()
                }]
            }]
        );
        let serialized = serde_json::to_vec(&plan.change_set).expect("frozen conflicts");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&serialized)),
            plan.change_set_fingerprint
        );
        let round_trip: PackageChangeSet = serde_json::from_slice(&serialized).expect("saved plan");
        assert_eq!(round_trip, plan.change_set);
        let mut hidden = plan.change_set.clone();
        hidden.removal_conflicts.clear();
        assert_ne!(
            <[u8; 32]>::from(Sha256::digest(
                serde_json::to_vec(&hidden).expect("altered")
            )),
            plan.change_set_fingerprint
        );
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
            "1.0.0"
        );
        assert!(value["locked"].get("com.example.b").is_none());
        assert_eq!(
            value["locked"]["com.example.a"],
            initial["locked"]["com.example.a"]
        );
        // A dependency-only target uses the same removal and review evidence.
        let mut indirect = initial;
        indirect["dependencies"]
            .as_object_mut()
            .expect("roots")
            .remove("com.example.b");
        let indirect = self::snapshot(indirect);
        let transitive =
            build_remove_plan(&indirect, "com.example.b").expect("installed transitive target");
        assert_eq!(transitive.change_set, plan.change_set);
        assert_eq!(
            transitive.change_set_fingerprint,
            plan.change_set_fingerprint
        );
        for change in [
            {
                let mut invalid = plan.change_set.clone();
                invalid.format_version = 3;
                invalid
            },
            {
                let mut invalid = plan.change_set.clone();
                invalid.dependency_edges.clear();
                invalid
            },
            {
                let mut invalid = plan.change_set.clone();
                let duplicate = invalid.removal_conflicts[0].required_by[0].clone();
                invalid.removal_conflicts[0].required_by.push(duplicate);
                invalid
            },
            {
                let mut invalid = plan.change_set.clone();
                invalid.mutations[0].kind = PackageMutationKind::Replace;
                invalid
            },
        ] {
            assert_eq!(
                change
                    .validate_bounds()
                    .expect_err("invalid conflict authority")
                    .code(),
                M4ErrorCode::PlanTooLarge
            );
        }
    }

    #[test]
    fn bulk_removal_uses_installed_graph_including_cycles_and_shared_dependencies() {
        let original = serde_json::json!({
            "dependencies": {"com.example.a":{"version":"1.0.0"},"com.example.b":{"version":"1.0.0"},"com.example.keep":{"version":"1.0.0"}},
            "locked": {
                "com.example.a":{"version":"1.0.0","dependencies":{"com.example.shared":"1.0.0","com.example.cycle":"1.0.0"}},
                "com.example.b":{"version":"1.0.0","dependencies":{"com.example.shared":"1.0.0"}},
                "com.example.cycle":{"version":"1.0.0","dependencies":{"com.example.a":"1.0.0"}},
                "com.example.keep":{"version":"1.0.0","dependencies":{"com.example.shared":"1.0.0"},"custom":"preserved"},
                "com.example.shared":{"version":"1.0.0","dependencies":{}},
                "com.example.orphan":{"version":"9.0.0","dependencies":{}}
            }
        });
        let snapshot = snapshot(original.clone());
        let removed = BTreeSet::from(["com.example.a".into(), "com.example.b".into()]);
        let plan = build_removal_plan(&snapshot, &removed, PlanAction::Bulk).expect("local graph");
        assert!(plan.source_set.is_empty());
        assert!(
            plan.change_set
                .mutations
                .iter()
                .all(|mutation| mutation.kind == PackageMutationKind::Remove
                    && mutation.source.is_none())
        );
        assert_eq!(
            plan.change_set
                .mutations
                .iter()
                .map(|mutation| mutation.package_id.as_str())
                .collect::<Vec<_>>(),
            vec!["com.example.a", "com.example.b", "com.example.cycle"]
        );
        let bytes = materialize_vpm_manifest(&snapshot.vpm_manifest_bytes, &plan.change_set)
            .expect("frozen output");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            plan.change_set.vpm_manifest_sha256
        );
        let next: Value = serde_json::from_slice(&bytes).expect("manifest");
        assert_eq!(
            next["locked"]["com.example.keep"],
            original["locked"]["com.example.keep"]
        );
        assert_eq!(
            next["locked"]["com.example.shared"],
            original["locked"]["com.example.shared"]
        );
        assert_eq!(
            next["locked"]["com.example.orphan"],
            original["locked"]["com.example.orphan"]
        );
    }

    #[test]
    fn bulk_removal_reviews_required_targets_and_rejects_only_missing_installed_targets() {
        let snapshot = snapshot(serde_json::json!({
            "dependencies":{"com.example.a":{"version":"1.0.0"},"com.example.b":{"version":"1.0.0"}},
            "locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"1.0.0"}},"com.example.b":{"version":"1.0.0"}}
        }));
        let plan = build_removal_plan(
            &snapshot,
            &BTreeSet::from(["com.example.b".into()]),
            PlanAction::Bulk,
        )
        .expect("review dependency conflict");
        let single = build_remove_plan(&snapshot, "com.example.b").expect("same single plan");
        assert_eq!(plan.change_set, single.change_set);
        assert_eq!(plan.change_set_fingerprint, single.change_set_fingerprint);
        assert_eq!(
            build_remove_plan(&snapshot, "com.example.absent")
                .expect_err("missing root")
                .code(),
            M4ErrorCode::PackageNotFound
        );
    }

    #[test]
    fn removal_conflicts_are_sorted_and_exclude_other_requested_targets_in_cycles() {
        let original = serde_json::json!({
            "dependencies":{"com.example.d":{"version":"1.0.0"},"com.example.c":{"version":"1.0.0"},"com.example.absent":{"version":"1.0.0"}},
            "locked":{
                "com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"^1.0.0"}},
                "com.example.b":{"version":"1.0.0","dependencies":{"com.example.a":"^1.0.0"}},
                "com.example.c":{"version":"1.0.0","dependencies":{"com.example.b":">=1.0.0","com.example.a":"^1.0.0"}},
                "com.example.d":{"version":"1.0.0","dependencies":{"com.example.a":">=1.0.0","com.example.b":"^1.0.0"}}
            }
        });
        let snapshot = snapshot(original.clone());
        let plan = build_removal_plan(
            &snapshot,
            &BTreeSet::from(["com.example.b".into(), "com.example.a".into()]),
            PlanAction::Bulk,
        )
        .expect("cyclic targets");
        assert_eq!(plan.change_set.mutations.len(), 2);
        assert_eq!(
            plan.change_set
                .removal_conflicts
                .iter()
                .map(|item| item.package_id.as_str())
                .collect::<Vec<_>>(),
            vec!["com.example.a", "com.example.b"]
        );
        for conflict in &plan.change_set.removal_conflicts {
            assert_eq!(
                conflict
                    .required_by
                    .iter()
                    .map(|item| item.package_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["com.example.c", "com.example.d"]
            );
        }
        let bytes = materialize_vpm_manifest(&snapshot.vpm_manifest_bytes, &plan.change_set)
            .expect("frozen manifest");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&bytes)),
            plan.change_set.vpm_manifest_sha256
        );
        let after: Value = serde_json::from_slice(&bytes).expect("JSON");
        for retained in ["com.example.c", "com.example.d"] {
            assert_eq!(after["locked"][retained], original["locked"][retained]);
        }
        assert_eq!(
            build_remove_plan(&snapshot, "com.example.absent")
                .expect_err("a declaration is not installed")
                .code(),
            M4ErrorCode::PackageNotFound
        );
        let mut oversized = plan.change_set;
        oversized.removal_conflicts[0].required_by =
            vec![
                oversized.removal_conflicts[0].required_by[0].clone();
                alcomd_application::MAX_DEPENDENCY_EDGES + 1
            ];
        assert_eq!(
            oversized
                .validate_bounds()
                .expect_err("bounded total conflict relations")
                .code(),
            M4ErrorCode::PlanTooLarge
        );
    }

    fn snapshot(value: Value) -> ProjectPackageSnapshot {
        ProjectPackageSnapshot {
            project_id: alcomd_application::ProjectId::new(),
            project_revision: alcomd_application::Revision::new(1).expect("revision"),
            root: PathBuf::new(),
            unlocked: BTreeMap::new(),
            absent_packages: BTreeSet::new(),
            path_identity_key: vec![1],
            fingerprint: [1; 32],
            vpm_manifest_bytes: serialize_manifest(&value).expect("original"),
            vpm_manifest: value,
        }
    }
}
