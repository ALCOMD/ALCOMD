use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use alcomd_application::{
    M4Error, M4ErrorCode, PackageChangeSet, PackageDependencyEdge as ChangeSetEdge,
    PackageMutation, PackageMutationKind, PackagePlanDraft, PackageSourcePin, PlanAction,
    ProjectRecord,
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
    let actual_direct = string_map(object.get("dependencies"))?;
    let actual_locked = locked_map(object.get("locked"))?;
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
    })
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
    let mut direct = string_map(object.get("dependencies"))?;
    let mut locked = locked_map(object.get("locked"))?;
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
        if current_version.as_deref() != Some(target_version.as_str()) {
            let source = source_pin(package);
            mutations.push(PackageMutation {
                kind: if current_version.is_some() {
                    PackageMutationKind::Replace
                } else {
                    PackageMutationKind::Install
                },
                package_id: package.package_id.clone(),
                from_version: current_version,
                to_version: Some(target_version.clone()),
                source: Some(source.clone()),
            });
            source_set.push(source);
        }
        locked.insert(package.package_id.clone(), target_version.clone());
        if direct_ids.contains(package.package_id.as_str()) {
            direct.insert(package.package_id.clone(), target_version);
        }
    }
    write_manifest_maps(object, &direct, &locked);
    build_draft(
        snapshot,
        action,
        manifest,
        mutations,
        resolution.dependency_edges.clone(),
        source_set,
    )
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
    let mut direct = string_map(object.get("dependencies"))?;
    let mut locked = locked_map(object.get("locked"))?;
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
                let source = source_pin(package);
                locked.insert(installed_id.clone(), package.version.to_string());
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
            let source = source_pin(package);
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
    write_manifest_maps(object, &direct, &locked);
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
    let mut manifest: Value =
        serde_json::from_slice(current.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(current))
            .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let mut direct = string_map(object.get("dependencies"))?;
    let mut locked = locked_map(object.get("locked"))?;
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
                locked.insert(mutation.package_id.clone(), version.clone());
                if direct_ids.contains(mutation.package_id.as_str()) {
                    direct.insert(mutation.package_id.clone(), version.clone());
                }
            }
            PackageMutationKind::Remove => {
                locked.remove(&mutation.package_id);
                direct.remove(&mutation.package_id);
            }
        }
    }
    write_manifest_maps(object, &direct, &locked);
    serialize_manifest(&manifest)
}

fn build_draft(
    snapshot: &ProjectPackageSnapshot,
    action: PlanAction,
    manifest: Value,
    mut mutations: Vec<PackageMutation>,
    dependency_edges: Vec<PackageDependencyEdge>,
    mut source_set: Vec<PackageSourcePin>,
) -> Result<PackagePlanDraft, M4Error> {
    mutations.sort_by(|left, right| left.package_id.as_bytes().cmp(right.package_id.as_bytes()));
    source_set.sort_by(|left, right| {
        left.package_id
            .as_bytes()
            .cmp(right.package_id.as_bytes())
            .then_with(|| left.version.as_bytes().cmp(right.version.as_bytes()))
            .then_with(|| {
                left.repository_id
                    .as_bytes()
                    .cmp(right.repository_id.as_bytes())
            })
    });
    source_set.dedup();
    let manifest_bytes = serialize_manifest(&manifest)?;
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
        format_version: 1,
        mutations,
        dependency_edges: edges,
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

fn source_pin(package: &ResolvedPackage) -> PackageSourcePin {
    PackageSourcePin {
        repository_id: package.source.repository_id.clone(),
        repository_revision: package.source.repository_revision,
        source_identity: package.source.source_identity.clone(),
        manifest_fingerprint: package.source.manifest_fingerprint,
        package_id: package.package_id.clone(),
        version: package.version.to_string(),
        artifact_url: package.source.artifact_url.clone(),
        archive_sha256: package.source.archive_sha256,
    }
}

fn write_manifest_maps(
    object: &mut Map<String, Value>,
    direct: &BTreeMap<String, String>,
    locked: &BTreeMap<String, String>,
) {
    object.insert(
        "dependencies".to_owned(),
        Value::Object(
            direct
                .iter()
                .map(|(name, version)| (name.clone(), Value::String(version.clone())))
                .collect(),
        ),
    );
    object.insert(
        "locked".to_owned(),
        Value::Object(
            locked
                .iter()
                .map(|(name, version)| {
                    (
                        name.clone(),
                        Value::Object(Map::from_iter([(
                            "version".to_owned(),
                            Value::String(version.clone()),
                        )])),
                    )
                })
                .collect(),
        ),
    );
}

fn serialize_manifest(value: &Value) -> Result<Vec<u8>, M4Error> {
    let mut bytes =
        serde_json::to_vec_pretty(value).map_err(|_| M4Error::new(M4ErrorCode::Internal))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn string_map(value: Option<&Value>) -> Result<BTreeMap<String, String>, M4Error> {
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
                .as_str()
                .filter(|version| !version.is_empty() && version.len() <= 1_024)
                .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
            Ok((name.clone(), version.to_owned()))
        })
        .collect()
}

fn locked_map(value: Option<&Value>) -> Result<BTreeMap<String, String>, M4Error> {
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

fn read_regular_component(
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

fn tree_fingerprint(root: &Path) -> Result<[u8; 32], M4Error> {
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
        let mut children = std::fs::read_dir(directory)
            .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?
            .map(|entry| {
                entry
                    .map(|entry| relative.join(entry.file_name()))
                    .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))
            })
            .collect::<Result<Vec<_>, _>>()?;
        children.sort();
        for child in children.into_iter().rev() {
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
