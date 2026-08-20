use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    M5TemplateError, M5TemplateErrorCode, ProjectId, ProjectRecord, PublishedTemplate,
    ResolverCatalog, Revision, StoredTemplateRecord, TemplateId, TemplatePlanDraft,
    TemplatePlanKind, TemplatePlanRecord, TemplateSourceKind, UnityWriterStateKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::{
    ArchiveLimits, TemplateDependency, TemplateEngine, TemplateManifest, TemplatePayload,
    TemplateProvenance, TemplateProvenanceKind, TemplateUnityCompatibility,
    inspect_template_bundle,
};

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct SourceFile {
    relative: String,
    path: PathBuf,
    bytes: u64,
}

struct SourceTree {
    files: Vec<SourceFile>,
    tree_sha256: [u8; 32],
    total_bytes: u64,
    project_fingerprint: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeriveAuthority {
    version: u32,
    kind: String,
    template_id: String,
    source_project_id: String,
    source_project_revision: u64,
    source_project_fingerprint: String,
    writer_state_evidence_class: String,
    include_policy_version: u32,
    expected_resulting_manifest_fingerprint: String,
    manifest_json: String,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_plan(
    project: ProjectRecord,
    catalog: ResolverCatalog,
    template_id: TemplateId,
    template_version: String,
    display_name: String,
    description: Option<String>,
    writer_state: UnityWriterStateKind,
) -> Result<TemplatePlanDraft, M5TemplateError> {
    if template_version.is_empty()
        || template_version.len() > 128
        || display_name.is_empty()
        || display_name.len() > 128
        || description
            .as_ref()
            .is_some_and(|value| value.len() > 4_096)
    {
        return Err(error(M5TemplateErrorCode::InvalidInput));
    }
    let dependencies = derive_dependencies(&project, &catalog)?;
    let excluded = project
        .observation
        .locked_dependencies
        .iter()
        .map(|value| value.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let tree = scan_source_tree(Path::new(&project.observation.root_path), &excluded)?;
    let unity = parse_unity_line(&project.observation.unity_version)?;
    let manifest = TemplateManifest {
        format_version: 1,
        template_id: template_id.to_string(),
        template_version,
        display_name,
        description,
        unity,
        dependencies,
        additional_resources: Vec::new(),
        payload: TemplatePayload {
            root: "payload/".to_owned(),
            tree_sha256: hex(&tree.tree_sha256),
            entry_count: tree.files.len() as u64,
            total_bytes: tree.total_bytes,
        },
        provenance: TemplateProvenance {
            created_by: TemplateProvenanceKind::Derived,
            derived_from_template_id: None,
            derived_from_project_id: Some(project.project_id.to_string()),
        },
    };
    let manifest_json = canonical_json(&manifest)?;
    let authority = DeriveAuthority {
        version: 1,
        kind: "derive".to_owned(),
        template_id: template_id.to_string(),
        source_project_id: project.project_id.to_string(),
        source_project_revision: project.revision.get(),
        source_project_fingerprint: hex(&tree.project_fingerprint),
        writer_state_evidence_class: writer_name(writer_state).to_owned(),
        include_policy_version: 1,
        expected_resulting_manifest_fingerprint: hex(
            &Sha256::digest(manifest_json.as_bytes()).into()
        ),
        manifest_json,
    };
    let plan_json = serde_json::to_string(&authority).map_err(|_| internal())?;
    Ok(TemplatePlanDraft {
        kind: TemplatePlanKind::Derive,
        plan_fingerprint: Sha256::digest(plan_json.as_bytes()).into(),
        plan_json,
    })
}

pub(super) fn derive_project_id(plan: &TemplatePlanRecord) -> Result<ProjectId, M5TemplateError> {
    let authority = authority(plan)?;
    ProjectId::parse(&authority.source_project_id).map_err(|_| internal())
}

pub(super) fn publish_derive(
    engine: &TemplateEngine,
    plan: TemplatePlanRecord,
    project: ProjectRecord,
) -> Result<PublishedTemplate, M5TemplateError> {
    let authority = authority(&plan)?;
    if authority.source_project_id != project.project_id.to_string()
        || authority.source_project_revision != project.revision.get()
        || authority.include_policy_version != 1
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let excluded = project
        .observation
        .locked_dependencies
        .iter()
        .map(|value| value.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let initial = scan_source_tree(Path::new(&project.observation.root_path), &excluded)?;
    if hex(&initial.project_fingerprint) != authority.source_project_fingerprint {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    let path = engine.staging.join(format!(
        "derive-{}-{}-{}.partial",
        plan.plan_id,
        std::process::id(),
        STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        write_bundle(&path, &authority.manifest_json, &initial.files)?;
        let final_tree = scan_source_tree(Path::new(&project.observation.root_path), &excluded)?;
        if final_tree.project_fingerprint != initial.project_fingerprint {
            return Err(error(
                M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
            ));
        }
        let inspection = inspect_template_bundle(&path)
            .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
        if hex(&inspection.manifest_fingerprint)
            != authority.expected_resulting_manifest_fingerprint
            || inspection.normalized_manifest_json != authority.manifest_json
        {
            return Err(internal());
        }
        let object = engine
            .objects
            .publish(&path, inspection.bundle_sha256)
            .map_err(super::template_engine::map_object_error)?;
        let template_id = TemplateId::parse(&authority.template_id).map_err(|_| internal())?;
        Ok(PublishedTemplate {
            record: StoredTemplateRecord {
                template_id,
                source_kind: TemplateSourceKind::User,
                template_version: inspection.manifest.template_version,
                manifest_json: inspection.normalized_manifest_json,
                payload_locator: object.locator,
                bundle_sha256: object.digest,
                favorite: false,
                revision: Revision::INITIAL,
                created_at_ms: 0,
                updated_at_ms: 0,
            },
        })
    })();
    let _ = std::fs::remove_file(path);
    result
}

fn authority(plan: &TemplatePlanRecord) -> Result<DeriveAuthority, M5TemplateError> {
    if plan.kind != TemplatePlanKind::Derive {
        return Err(internal());
    }
    let authority: DeriveAuthority =
        serde_json::from_str(&plan.plan_json).map_err(|_| internal())?;
    if authority.version != 1 || authority.kind != "derive" {
        return Err(internal());
    }
    Ok(authority)
}

fn derive_dependencies(
    project: &ProjectRecord,
    catalog: &ResolverCatalog,
) -> Result<Vec<TemplateDependency>, M5TemplateError> {
    let mut dependencies = Vec::new();
    for locked in &project.observation.locked_dependencies {
        let mut candidates = catalog
            .entries
            .iter()
            .filter(|entry| entry.package_id == locked.package_id && entry.version == locked.value)
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .repository_priority
                .cmp(&left.repository_priority)
                .then_with(|| left.repository_id.cmp(&right.repository_id))
        });
        let Some(selected) = candidates.first() else {
            return Err(error(M5TemplateErrorCode::TemplatePlanStale));
        };
        if candidates.get(1).is_some_and(|other| {
            other.repository_priority == selected.repository_priority
                && other.manifest_fingerprint != selected.manifest_fingerprint
        }) {
            return Err(error(M5TemplateErrorCode::TemplateConflict));
        }
        semver::Version::parse(&locked.value)
            .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
        dependencies.push(TemplateDependency {
            package_id: locked.package_id.clone(),
            version_range: format!("={}", locked.value),
            include_prerelease: locked.value.contains('-'),
        });
    }
    dependencies.sort_by(|left, right| left.package_id.cmp(&right.package_id));
    Ok(dependencies)
}

fn scan_source_tree(
    root: &Path,
    excluded_packages: &BTreeSet<&str>,
) -> Result<SourceTree, M5TemplateError> {
    if !root.is_absolute() {
        return Err(error(M5TemplateErrorCode::InvalidInput));
    }
    let root_metadata = std::fs::symlink_metadata(root).map_err(|_| internal())?;
    if !root_metadata.is_dir() || is_link_or_reparse(&root_metadata) {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let mut files = Vec::new();
    for name in ["Assets", "ProjectSettings"] {
        let path = root.join(name);
        if path.exists() {
            walk(root, &path, excluded_packages, &mut files)?;
        }
    }
    let packages = root.join("Packages");
    let metadata = std::fs::symlink_metadata(&packages).map_err(|_| internal())?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let mut entries = std::fs::read_dir(&packages)
        .map_err(|_| internal())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| internal())?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry
            .file_name()
            .to_str()
            .ok_or_else(|| error(M5TemplateErrorCode::TemplateBundleInvalid))?
            .to_owned();
        if matches!(
            name.as_str(),
            "manifest.json" | "vpm-manifest.json" | "packages-lock.json"
        ) || !excluded_packages.contains(name.as_str())
        {
            walk(root, &entry.path(), excluded_packages, &mut files)?;
        }
    }
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    if files.is_empty()
        || !files
            .iter()
            .any(|file| file.relative == "ProjectSettings/ProjectVersion.txt")
        || !files
            .iter()
            .any(|file| file.relative == "Packages/manifest.json")
    {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let limits = ArchiveLimits::template();
    if files.len() > limits.entries {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let mut identities = BTreeSet::new();
    let mut collisions = BTreeSet::new();
    let mut tree = Sha256::new();
    let mut total = 0_u64;
    for file in &files {
        if file.bytes > limits.entry_bytes
            || file.relative.len() > limits.normalized_path_bytes
            || file.relative.split('/').count() > limits.path_depth
            || !collisions.insert(file.relative.nfc().collect::<String>().to_lowercase())
            || !identities
                .insert(alcomd_platform::file_identity_key(&file.path).map_err(|_| internal())?)
        {
            return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
        }
        total = total.checked_add(file.bytes).ok_or_else(internal)?;
        if total > limits.total_uncompressed_bytes {
            return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
        }
        tree.update(
            u32::try_from(file.relative.len())
                .map_err(|_| internal())?
                .to_le_bytes(),
        );
        tree.update(file.relative.as_bytes());
        tree.update(file.bytes.to_le_bytes());
        hash_file_into(&file.path, file.bytes, &mut tree)?;
    }
    let tree_sha256: [u8; 32] = tree.finalize().into();
    let root_identity = alcomd_platform::file_identity_key(root).map_err(|_| internal())?;
    let mut fingerprint = Sha256::new();
    fingerprint.update((root_identity.len() as u32).to_le_bytes());
    fingerprint.update(root_identity);
    fingerprint.update(tree_sha256);
    Ok(SourceTree {
        files,
        tree_sha256,
        total_bytes: total,
        project_fingerprint: fingerprint.finalize().into(),
    })
}

fn walk(
    root: &Path,
    path: &Path,
    excluded_packages: &BTreeSet<&str>,
    files: &mut Vec<SourceFile>,
) -> Result<(), M5TemplateError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| internal())?;
    if is_link_or_reparse(&metadata) {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    if metadata.is_file() {
        verify_single_link(path, &metadata)?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| internal())?
            .to_str()
            .ok_or_else(|| error(M5TemplateErrorCode::TemplateBundleInvalid))?
            .replace('\\', "/");
        files.push(SourceFile {
            relative,
            path: path.to_path_buf(),
            bytes: metadata.len(),
        });
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let mut entries = std::fs::read_dir(path)
        .map_err(|_| internal())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| internal())?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let child = entry.path();
        if path == root.join("Packages") {
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
            if excluded_packages.contains(name) {
                continue;
            }
        }
        walk(root, &child, excluded_packages, files)?;
    }
    Ok(())
}

fn write_bundle(
    target: &Path,
    manifest_json: &str,
    files: &[SourceFile],
) -> Result<(), M5TemplateError> {
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|_| internal())?;
    let mut archive = zip::ZipWriter::new(output);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    archive
        .start_file("template.json", options)
        .map_err(|_| internal())?;
    archive
        .write_all(manifest_json.as_bytes())
        .map_err(|_| internal())?;
    let mut buffer = [0_u8; 64 * 1024];
    for file in files {
        archive
            .start_file(format!("payload/{}", file.relative), options)
            .map_err(|_| internal())?;
        let mut input = File::open(&file.path).map_err(|_| internal())?;
        let mut remaining = file.bytes;
        while remaining > 0 {
            let maximum = remaining.min(buffer.len() as u64) as usize;
            let read = input.read(&mut buffer[..maximum]).map_err(|_| internal())?;
            if read == 0 {
                return Err(error(
                    M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
                ));
            }
            archive.write_all(&buffer[..read]).map_err(|_| internal())?;
            remaining -= read as u64;
        }
        if input.read(&mut buffer[..1]).map_err(|_| internal())? != 0 {
            return Err(error(
                M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
            ));
        }
    }
    let output = archive.finish().map_err(|_| internal())?;
    output.sync_all().map_err(|_| internal())
}

fn hash_file_into(path: &Path, expected: u64, digest: &mut Sha256) -> Result<(), M5TemplateError> {
    let mut file = File::open(path).map_err(|_| internal())?;
    let mut remaining = expected;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let maximum = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..maximum]).map_err(|_| internal())?;
        if read == 0 {
            return Err(error(
                M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
            ));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file.read(&mut buffer[..1]).map_err(|_| internal())? != 0 {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    Ok(())
}

fn parse_unity_line(value: &str) -> Result<TemplateUnityCompatibility, M5TemplateError> {
    let numeric = value
        .split_once('.')
        .and_then(|(major, rest)| rest.split_once('.').map(|(minor, _)| (major, minor)))
        .ok_or_else(|| error(M5TemplateErrorCode::InvalidInput))?;
    Ok(TemplateUnityCompatibility {
        major: numeric.0.parse().map_err(|_| internal())?,
        minor: numeric.1.parse().map_err(|_| internal())?,
    })
}

fn canonical_json(value: &impl Serialize) -> Result<String, M5TemplateError> {
    let mut value = serde_json::to_value(value).map_err(|_| internal())?;
    sort_json(&mut value);
    serde_json::to_string(&value).map_err(|_| internal())
}

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut sorted = std::mem::take(object)
                .into_iter()
                .collect::<BTreeMap<_, _>>();
            for value in sorted.values_mut() {
                sort_json(value);
            }
            *object = sorted.into_iter().collect::<Map<_, _>>();
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

fn writer_name(value: UnityWriterStateKind) -> &'static str {
    match value {
        UnityWriterStateKind::RunningConfirmed => "running_confirmed",
        UnityWriterStateKind::RunningSuspected => "running_suspected",
        UnityWriterStateKind::NotObserved => "not_observed",
        UnityWriterStateKind::Unknown => "unknown",
    }
}

#[cfg(unix)]
fn verify_single_link(_: &Path, metadata: &std::fs::Metadata) -> Result<(), M5TemplateError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() == 1 {
        Ok(())
    } else {
        Err(error(M5TemplateErrorCode::TemplateBundleInvalid))
    }
}

#[cfg(windows)]
fn verify_single_link(path: &Path, _: &std::fs::Metadata) -> Result<(), M5TemplateError> {
    match alcomd_platform::file_link_count(path) {
        Ok(1) => Ok(()),
        Ok(_) | Err(_) => Err(error(M5TemplateErrorCode::TemplateBundleInvalid)),
    }
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

fn hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn internal() -> M5TemplateError {
    error(M5TemplateErrorCode::Internal)
}

const fn error(code: M5TemplateErrorCode) -> M5TemplateError {
    M5TemplateError::new(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn fixture(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "alcomd-template-derive-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("Assets")).expect("Assets");
        std::fs::create_dir_all(root.join("ProjectSettings")).expect("ProjectSettings");
        std::fs::create_dir_all(root.join("Packages")).expect("Packages");
        std::fs::write(root.join("Assets/source.txt"), b"source").expect("asset");
        std::fs::write(
            root.join("ProjectSettings/ProjectVersion.txt"),
            b"m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("version");
        std::fs::write(
            root.join("Packages/manifest.json"),
            b"{\"dependencies\":{}}",
        )
        .expect("manifest");
        root
    }

    #[test]
    fn ordinary_project_tree_is_stable_and_project_changes_are_visible() {
        let root = fixture("ordinary");
        let excluded = BTreeSet::new();
        let first = scan_source_tree(&root, &excluded).expect("first scan");
        let second = scan_source_tree(&root, &excluded).expect("second scan");
        assert_eq!(first.project_fingerprint, second.project_fingerprint);
        std::fs::write(root.join("Assets/source.txt"), b"changed").expect("change asset");
        let changed = scan_source_tree(&root, &excluded).expect("changed scan");
        assert_ne!(first.project_fingerprint, changed.project_fingerprint);
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[cfg(windows)]
    #[test]
    fn windows_derive_rejects_inside_and_outside_hard_links_and_query_failure() {
        let root = fixture("windows-links");
        let asset = root.join("Assets/source.txt");
        let inside = root.join("Assets/inside.txt");
        std::fs::hard_link(&asset, &inside).expect("inside hard link");
        match scan_source_tree(&root, &BTreeSet::new()) {
            Err(source) => assert_eq!(source.code(), M5TemplateErrorCode::TemplateBundleInvalid),
            Ok(_) => panic!("inside hard link must fail closed"),
        }
        std::fs::remove_file(&inside).expect("remove inside link");

        let outside = root.parent().expect("fixture parent").join(format!(
            "outside-{}.txt",
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::hard_link(&asset, &outside).expect("outside hard link");
        match scan_source_tree(&root, &BTreeSet::new()) {
            Err(source) => assert_eq!(source.code(), M5TemplateErrorCode::TemplateBundleInvalid),
            Ok(_) => panic!("outside hard link must fail closed"),
        }
        std::fs::remove_file(&outside).expect("remove outside link");

        let metadata = std::fs::metadata(&asset).expect("source metadata");
        let missing = root.join("Assets/missing.txt");
        assert_eq!(
            verify_single_link(&missing, &metadata)
                .expect_err("link-count query failure must fail closed")
                .code(),
            M5TemplateErrorCode::TemplateBundleInvalid
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
