use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use alcomd_application::{
    OperationId, ProjectCreateError, ProjectCreateErrorCode, ProjectRecord,
    ProjectTemplateDeclaration, ProjectTemplateDependency, ProjectTemplateSource,
    canonical_unity_version, valid_project_template_id,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::archive::{
    ArchiveErrorCode, ArchiveLimits, ArchivePreflight, extract_archive_checked, normalize_path,
    preflight_archive,
};
use crate::range::{ProjectDependencyRange, VpmRange};

const LIMITS: ArchiveLimits = ArchiveLimits::package();
const TEMPLATE_EXTENSION: &str = "alcomdtemplate";
const ROOTS: [&str; 3] = ["Assets", "Packages", "ProjectSettings"];
const UPM: &str = "Packages/manifest.json";
const VPM: &str = "Packages/vpm-manifest.json";
const EDITOR: &str = "ProjectSettings/ProjectVersion.txt";
const PLAYER: &str = "ProjectSettings/ProjectSettings.asset";
const RESOLVER: &str = "com.vrchat.core.vpm-resolver";
const BUILTIN_PROJECTS: &[u8] =
    include_bytes!("../../../specs/templates/builtin-scaffolds/v3-projects.zip");
const BUILTIN_PROJECT_LICENSE: &[u8] =
    include_bytes!("../../../specs/templates/builtin-scaffolds/LICENSE");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuiltinFamily {
    id: String,
    name: String,
    package_id: String,
    variants: Vec<BuiltinVariant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BuiltinVariant {
    editor_version: String,
    locked_versions: BTreeMap<String, String>,
}

#[derive(Clone)]
pub(crate) struct TemplateArchiveStore {
    root: PathBuf,
    root_identity: Vec<u8>,
}

impl TemplateArchiveStore {
    pub(crate) fn new(root: PathBuf) -> Result<Self, ProjectCreateError> {
        fs::create_dir_all(&root).map_err(|error| io_error(&root, error))?;
        let (root, root_identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|error| io_error(&root, error))?;
        Scratch::clean_interrupted(&root)?;
        Ok(Self {
            root,
            root_identity,
        })
    }

    pub(crate) fn prepare(
        &self,
        source: ProjectTemplateSource,
        project: Option<ProjectRecord>,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        self.verify_root()?;
        match source {
            ProjectTemplateSource::Manual {
                id,
                name,
                unity_version,
                dependencies,
            } => self.manual(id, name, unity_version, dependencies, BTreeMap::new()),
            ProjectTemplateSource::Project {
                project_id,
                expected_project_revision,
                id,
                name,
                include_assets,
                lock_package_versions,
            } => {
                let project = project.ok_or_else(stale)?;
                if project.project_id != project_id
                    || project.revision.get() != expected_project_revision
                {
                    return Err(stale());
                }
                let mut capture =
                    ProjectCapture::read(&project, include_assets, lock_package_versions)?;
                capture
                    .archive
                    .add_bytes("package.json", template_metadata(&id, &name)?)?;
                let scratch = Scratch::new(&self.root)?;
                let path = scratch.path.join("template.zip");
                capture.archive.write(&path)?;
                capture.verify()?;
                self.publish(&path)
            }
            ProjectTemplateSource::File { source_file } => {
                self.import_file(template_path(&source_file)?)
            }
        }
    }

    fn import_file(&self, source: &Path) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        let scratch = Scratch::new(&self.root)?;
        let copy = scratch.path.join("input.zip");
        copy_checked_file(source, &copy, LIMITS.archive_bytes)?;
        let unpacked = scratch.path.join("input");
        fs::create_dir(&unpacked).map_err(|error| io_error(&unpacked, error))?;
        unpack(&copy, &unpacked)?;
        // Canonicalize container metadata and entry order without changing file content.
        let mut contents = Inventory::default();
        contents.add_file(&unpacked, "package.json")?;
        for entry in fs::read_dir(&unpacked).map_err(|error| io_error(&unpacked, error))? {
            let entry = entry.map_err(|error| io_error(&unpacked, error))?;
            if let Some(name) = entry
                .file_name()
                .to_str()
                .filter(|name| optional_root_file(name))
            {
                contents.add_file(&unpacked, name)?;
            }
        }
        for folder in ROOTS {
            contents.add_tree(&unpacked, folder)?;
        }
        let canonical = scratch.path.join("template.zip");
        contents.write(&canonical)?;
        self.publish(&canonical)
    }

    pub(crate) fn builtins(&self) -> Result<Vec<ProjectTemplateDeclaration>, ProjectCreateError> {
        self.verify_root()?;
        let mut projects = zip::ZipArchive::new(Cursor::new(BUILTIN_PROJECTS))
            .map_err(|error| invalid().with_source(error))?;
        let mut result = Vec::with_capacity(4);
        for input in [
            include_str!("../../../specs/templates/vrchat-avatars.json"),
            include_str!("../../../specs/templates/vrchat-worlds.json"),
        ] {
            let family: BuiltinFamily =
                serde_json::from_str(input).map_err(|error| invalid().with_source(error))?;
            let kind = family
                .package_id
                .strip_prefix("com.vrchat.")
                .ok_or_else(invalid)?;
            for variant in family.variants {
                let (year, _) = variant.editor_version.split_once('.').ok_or_else(invalid)?;
                let prefix = format!("{kind}-{}/", variant.editor_version);
                let mut inventory = Inventory::default();
                for index in 0..projects.len() {
                    let mut entry = projects
                        .by_index(index)
                        .map_err(|error| invalid().with_source(error))?;
                    let entry_name = entry.name().map_err(|error| invalid().with_source(error))?;
                    let Some(relative) = entry_name.strip_prefix(&prefix).map(str::to_owned) else {
                        continue;
                    };
                    let mut bytes = Vec::new();
                    entry
                        .read_to_end(&mut bytes)
                        .map_err(|error| invalid().with_source(error))?;
                    let bytes = match relative.as_str() {
                        PLAYER => rewrite_product_name(Some(&bytes), "AlcomdTemplate", None)?,
                        VPM if !variant.locked_versions.is_empty() => {
                            let (direct, _, _) = read_vpm(&bytes)?;
                            make_manifest(&direct, &variant.locked_versions)?
                        }
                        _ => bytes,
                    };
                    inventory.add_bytes(&relative, bytes)?;
                }
                if [UPM, VPM, EDITOR, PLAYER]
                    .iter()
                    .any(|file| !inventory.entries.contains_key(*file))
                {
                    return Err(invalid());
                }
                let id = format!("{}.{year}", family.id);
                let name = format!("{} {year}", family.name);
                inventory.add_bytes("package.json", template_metadata(&id, &name)?)?;
                inventory.add_bytes("LICENSE", BUILTIN_PROJECT_LICENSE.to_vec())?;
                let scratch = Scratch::new(&self.root)?;
                let archive = scratch.path.join("builtin.zip");
                inventory.write(&archive)?;
                let declaration = self.import_file(&archive)?;
                if declaration.editor_version.as_deref() != Some(variant.editor_version.as_str()) {
                    return Err(invalid());
                }
                result.push(declaration);
            }
        }
        Ok(result)
    }

    fn manual(
        &self,
        id: String,
        name: String,
        version: String,
        dependencies: Vec<ProjectTemplateDependency>,
        locked: BTreeMap<String, String>,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        validate_editor(&version)?;
        let direct = dependency_map(dependencies)?;
        let mut inventory = Inventory::default();
        inventory.add_bytes("package.json", template_metadata(&id, &name)?)?;
        // VCC replaces an unquoted token when giving the new project its name.
        inventory.add_bytes(PLAYER, rewrite_product_name(None, "AlcomdTemplate", None)?)?;
        inventory.add_bytes(UPM, b"{\"dependencies\":{}}\n".to_vec())?;
        inventory.add_bytes(VPM, make_manifest(&direct, &locked)?)?;
        inventory.add_bytes(EDITOR, format!("m_EditorVersion: {version}\n").into_bytes())?;
        self.publish_inventory(inventory)
    }

    fn publish_inventory(
        &self,
        mut inventory: Inventory,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        let scratch = Scratch::new(&self.root)?;
        let path = scratch.path.join("template.zip");
        inventory.write(&path)?;
        self.publish(&path)
    }

    fn publish(&self, path: &Path) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        let planned = preflight_archive(path).map_err(|error| invalid().with_source(error))?;
        validate_layout(&planned).map_err(|_| {
            invalid_at(
                path,
                "template entries do not match the VCC directory layout",
            )
        })?;
        let digest = digest_file(path)?;
        let declaration = read_declaration(path, digest.clone())?;
        let object = self.object(&digest)?;
        if !optional_exists(&object)? {
            if let Err(error) = alcomd_platform::publish_path_no_replace(path, &object)
                && !optional_exists(&object)?
            {
                return Err(io_error(&object, error));
            }
            alcomd_platform::sync_directory(&self.root)
                .map_err(|error| io_error(&self.root, error))?;
        }
        if digest_file(&object)? != digest {
            return Err(invalid_at(&object, "template archive digest mismatch"));
        }
        Ok(declaration)
    }

    pub(crate) fn expand(
        &self,
        declaration: &ProjectTemplateDeclaration,
        destination: &Path,
    ) -> Result<(), ProjectCreateError> {
        let path = self.verified_object(declaration)?;
        unpack(&path, destination)?;
        for entry in fs::read_dir(destination).map_err(|error| io_error(destination, error))? {
            let entry = entry.map_err(|error| io_error(destination, error))?;
            if entry.file_name().to_str().is_some_and(|name| {
                name == "package.json" || (optional_root_file(name) && !license_root_file(name))
            }) {
                fs::remove_file(entry.path()).map_err(|error| io_error(&entry.path(), error))?;
            }
        }
        Ok(())
    }

    pub(crate) fn manifest_digest(
        &self,
        declaration: &ProjectTemplateDeclaration,
        name: &str,
        default: &[u8],
    ) -> Result<[u8; 32], ProjectCreateError> {
        let path = self.verified_object(declaration)?;
        let mut archive = open_zip(&path)?;
        let bytes = zip_bytes(&mut archive, name, crate::PROJECT_MANIFEST_LIMIT)?;
        Ok(Sha256::digest(bytes.as_deref().unwrap_or(default)).into())
    }

    fn object(&self, digest: &str) -> Result<PathBuf, ProjectCreateError> {
        self.verify_root()?;
        if !is_archive_digest(digest) {
            return Err(invalid());
        }
        Ok(self.root.join(format!("{digest}.{TEMPLATE_EXTENSION}")))
    }

    pub(crate) fn prune_unreferenced(
        &self,
        referenced: &BTreeSet<String>,
    ) -> Result<(), ProjectCreateError> {
        self.verify_root()?;
        let mut removed = false;
        for entry in fs::read_dir(&self.root).map_err(|error| io_error(&self.root, error))? {
            let entry = entry.map_err(|error| io_error(&self.root, error))?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some(TEMPLATE_EXTENSION)
                || !path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|digest| is_archive_digest(digest) && !referenced.contains(digest))
                || !entry
                    .file_type()
                    .map_err(|error| io_error(&path, error))?
                    .is_file()
            {
                continue;
            }
            self.verify_root()?;
            match fs::remove_file(&path) {
                Ok(()) => removed = true,
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(io_error(&path, error)),
            }
        }
        if removed {
            alcomd_platform::sync_directory(&self.root)
                .map_err(|error| io_error(&self.root, error))?;
        }
        Ok(())
    }

    fn verify_root(&self) -> Result<(), ProjectCreateError> {
        if stamp(&self.root, true)?.identity != self.root_identity {
            return Err(stale());
        }
        Ok(())
    }

    fn verified_object(
        &self,
        declaration: &ProjectTemplateDeclaration,
    ) -> Result<PathBuf, ProjectCreateError> {
        let path = self.object(&declaration.archive_sha256)?;
        if digest_file(&path)? != declaration.archive_sha256 {
            return Err(invalid_at(&path, "template archive digest mismatch"));
        }
        Ok(path)
    }

    pub(crate) fn export(
        &self,
        declaration: &ProjectTemplateDeclaration,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError> {
        let object = self.verified_object(declaration)?;
        let export = Export::resolve(destination_file, id)?;
        if export.parent_identity == self.root_identity || export.parent.starts_with(&self.root) {
            return Err(invalid_at(
                &export.target,
                "export destination is in the managed template store",
            ));
        }
        let stage = export.stage.join("template");
        if optional_exists(&export.target)? {
            if export.owns_stage()? {
                let identity = export.published_identity()?;
                if stamp(&export.target, false)
                    .is_ok_and(|target| identity.as_ref() == Some(&target.identity))
                    && digest_file(&export.target)? == declaration.archive_sha256
                {
                    return export
                        .sync_parent()
                        .map_err(ProjectCreateError::recover_on_restart);
                }
                export.finish()?;
            }
            return Err(target_exists(&export.target));
        }
        if export.owns_stage()? {
            export.finish()?;
        }
        export.create_stage()?;
        let published = (|| {
            copy_checked_file(&object, &stage, LIMITS.archive_bytes)?;
            if digest_file(&stage)? != declaration.archive_sha256 {
                return Err(invalid_at(&stage, "template archive digest mismatch"));
            }
            let identity = stamp(&stage, false)?.identity;
            let receipt = export.stage.join("file-identity.json");
            let bytes =
                serde_json::to_vec(&identity).map_err(|error| invalid().with_source(error))?;
            let mut file = new_file(&receipt)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| io_error(&receipt, error))?;
            alcomd_platform::sync_directory(&export.stage)
                .map_err(|error| io_error(&export.stage, error))?;
            export.sync_parent()?;
            export.verify_parent()?;
            alcomd_platform::publish_path_no_replace(&stage, &export.target).map_err(|error| {
                if optional_exists(&export.target).unwrap_or(false) {
                    target_exists(&export.target)
                } else {
                    io_error(&export.target, error)
                }
            })
        })();
        if let Err(error) = published {
            export.finish()?;
            return Err(error);
        }
        export
            .sync_parent()
            .map_err(ProjectCreateError::recover_on_restart)
    }

    pub(crate) fn finish_export(
        &self,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError> {
        Export::resolve(destination_file, id)?.finish()
    }
}

fn is_archive_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn player_settings(
    original: Option<&[u8]>,
    name: &str,
    product_guid: &str,
) -> Result<Vec<u8>, ProjectCreateError> {
    let name = serde_json::to_string(name).map_err(|error| invalid().with_source(error))?;
    rewrite_product_name(original, &name, Some(product_guid))
}

fn rewrite_product_name(
    original: Option<&[u8]>,
    yaml_scalar: &str,
    product_guid: Option<&str>,
) -> Result<Vec<u8>, ProjectCreateError> {
    let name_line = format!("  productName: {yaml_scalar}");
    let Some(original) = original else {
        return Ok(format!(
            "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!129 &1\nPlayerSettings:\n{name_line}\n"
        ).into_bytes());
    };
    let settings = std::str::from_utf8(original).map_err(|error| invalid().with_source(error))?;
    let mut header = None;
    let mut previous = None;
    let mut offset = 0;
    for line in settings.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "PlayerSettings:" {
            header = Some((offset + line.len(), &line[trimmed.len()..]));
        }
        if header.is_some()
            && trimmed.starts_with("  productName:")
            && previous.replace(offset..offset + trimmed.len()).is_some()
        {
            return Err(invalid());
        }
        offset += line.len();
    }
    let mut updated = settings.to_owned();
    if let Some(previous) = previous {
        updated.replace_range(previous, &name_line);
    } else {
        let (offset, end) = header.ok_or_else(invalid)?;
        let inserted = if end.is_empty() {
            format!("\n{name_line}\n")
        } else {
            format!("{name_line}{end}")
        };
        updated.insert_str(offset, &inserted);
    }
    if let Some(guid) = product_guid {
        const PLACEHOLDER: &str = "  productGUID: 00000000000000000000000000000000";
        let mut offset = 0;
        for line in updated.split_inclusive('\n') {
            if line.trim_end_matches(['\r', '\n']) == PLACEHOLDER {
                let start = offset + "  productGUID: ".len();
                updated.replace_range(start..offset + PLACEHOLDER.len(), guid);
                break;
            }
            offset += line.len();
        }
    }
    Ok(updated.into_bytes())
}

struct Export {
    parent: PathBuf,
    parent_identity: Vec<u8>,
    target: PathBuf,
    stage: PathBuf,
}

impl Export {
    fn resolve(path: &str, id: OperationId) -> Result<Self, ProjectCreateError> {
        let path = template_path(path)?;
        let requested_parent = path.parent().ok_or_else(invalid)?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(invalid)?;
        normalize_path(name, LIMITS).map_err(|error| invalid().with_source(error))?;
        let (parent, parent_identity) =
            alcomd_platform::resolve_directory_identity(requested_parent)
                .map_err(|error| io_error(requested_parent, error))?;
        Ok(Self {
            target: parent.join(name),
            stage: parent.join(format!(".alcomd-template-export-{id}")),
            parent,
            parent_identity,
        })
    }

    fn verify_parent(&self) -> Result<(), ProjectCreateError> {
        if stamp(&self.parent, true)?.identity != self.parent_identity {
            return Err(stale());
        }
        Ok(())
    }

    fn create_stage(&self) -> Result<(), ProjectCreateError> {
        self.verify_parent()?;
        fs::create_dir(&self.stage).map_err(|error| io_error(&self.stage, error))?;
        let identity = stamp(&self.stage, true)?.identity;
        let result = (|| {
            let receipt = self.stage.join("identity.json");
            let bytes =
                serde_json::to_vec(&identity).map_err(|error| invalid().with_source(error))?;
            let mut file = new_file(&receipt)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| io_error(&receipt, error))?;
            alcomd_platform::sync_directory(&self.stage)
                .map_err(|error| io_error(&self.stage, error))
        })();
        if result.is_err() && stamp(&self.stage, true).is_ok_and(|value| value.identity == identity)
        {
            let _ = fs::remove_dir_all(&self.stage);
        }
        result
    }

    fn owns_stage(&self) -> Result<bool, ProjectCreateError> {
        if !optional_exists(&self.stage)? {
            return Ok(false);
        }
        let identity = stamp(&self.stage, true)?.identity;
        let receipt = self.stage.join("identity.json");
        let data = read_bounded(&receipt, 4096)?;
        let saved: Vec<u8> =
            serde_json::from_slice(&data).map_err(|error| invalid().with_source(error))?;
        if identity != saved {
            return Err(stale());
        }
        Ok(true)
    }

    fn sync_parent(&self) -> Result<(), ProjectCreateError> {
        self.verify_parent()?;
        alcomd_platform::sync_directory(&self.parent).map_err(|error| io_error(&self.parent, error))
    }

    fn published_identity(&self) -> Result<Option<Vec<u8>>, ProjectCreateError> {
        let receipt = self.stage.join("file-identity.json");
        if !optional_exists(&receipt)? {
            return Ok(None);
        }
        let data = read_bounded(&receipt, 4096)?;
        serde_json::from_slice(&data)
            .map(Some)
            .map_err(|error| invalid().with_source(error))
    }

    fn finish(&self) -> Result<(), ProjectCreateError> {
        self.verify_parent()?;
        if self.owns_stage()? {
            fs::remove_dir_all(&self.stage).map_err(|error| io_error(&self.stage, error))?;
            self.sync_parent()?;
        }
        Ok(())
    }
}

struct ProjectCapture {
    root: PathBuf,
    root_identity: Vec<u8>,
    archive: Inventory,
    trees: Vec<(String, BTreeMap<String, Stamp>)>,
    sources: BTreeMap<String, Option<Stamp>>,
    package_directories: Vec<String>,
}

impl ProjectCapture {
    fn read(
        project: &ProjectRecord,
        include_assets: bool,
        lock: bool,
    ) -> Result<Self, ProjectCreateError> {
        let root = PathBuf::from(&project.observation.root_path);
        let (canonical, identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|error| io_error(&root, error))?;
        if root != canonical || identity != project.observation.path_identity_key {
            return Err(stale());
        }
        let mut capture = Self {
            root,
            root_identity: identity,
            archive: Inventory::default(),
            trees: Vec::new(),
            sources: BTreeMap::new(),
            package_directories: Vec::new(),
        };
        let version = capture
            .read_source(EDITOR, crate::PROJECT_VERSION_LIMIT)?
            .ok_or_else(stale)?;
        let (unity, _) =
            crate::parse_project_version(&version).map_err(|error| stale().with_source(error))?;
        if unity != project.observation.unity_version {
            return Err(stale());
        }
        validate_editor(&unity)?;
        capture.add_tree("ProjectSettings")?;
        if include_assets {
            capture.add_tree("Assets")?;
        }
        let upm = capture.read_source(UPM, crate::PROJECT_MANIFEST_LIMIT)?;
        let vpm = capture.read_source(VPM, crate::PROJECT_MANIFEST_LIMIT)?;
        let manifests = vpm
            .as_deref()
            .map(read_vpm)
            .transpose()?
            .unwrap_or_default();
        let (mut direct, locked, _) = manifests;
        let observed_direct = project
            .observation
            .direct_dependencies
            .iter()
            .map(|dependency| {
                Ok((
                    dependency.package_id.clone(),
                    project_range(&dependency.value)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ProjectCreateError>>()?;
        let observed_locked = project
            .observation
            .locked_dependencies
            .iter()
            .map(|dependency| (dependency.package_id.clone(), dependency.value.clone()))
            .collect::<BTreeMap<_, _>>();
        if direct != observed_direct || locked != observed_locked {
            return Err(stale());
        }
        capture.package_directories = package_directories(&capture.root)?;
        for name in capture.package_directories.clone() {
            if name != RESOLVER && (locked.contains_key(&name) || direct.contains_key(&name)) {
                continue;
            }
            let component = format!("Packages/{name}");
            let path = format!("{component}/package.json");
            if let Some(manifest) = capture.read_source(&path, crate::PROJECT_MANIFEST_LIMIT)? {
                let package = parse_json(&manifest)?;
                let package_id = package
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid)?;
                validate_package_id(package_id)?;
                if package_id == RESOLVER
                    || (!locked.contains_key(package_id) && !direct.contains_key(package_id))
                {
                    capture.add_tree(&component)?;
                }
            }
        }
        let vpm = if lock {
            for id in direct.keys() {
                if !locked.contains_key(id) {
                    return Err(dependency_missing(id));
                }
            }
            verify_lock_closure(vpm.as_deref(), &locked)?;
            vpm.unwrap_or(make_manifest(&direct, &locked)?)
        } else {
            direct
                .values_mut()
                .for_each(|value| *value = "*".to_owned());
            make_manifest(&direct, &BTreeMap::new())?
        };
        capture.archive.add_bytes(VPM, vpm)?;
        if let Some(upm) = upm {
            capture.archive.add_bytes(UPM, upm)?;
        }
        Ok(capture)
    }

    fn read_source(
        &mut self,
        relative: &str,
        limit: usize,
    ) -> Result<Option<Vec<u8>>, ProjectCreateError> {
        let path = self.root.join(relative);
        let fingerprint = source_stamp(&self.root, relative)?;
        let bytes = fingerprint
            .as_ref()
            .map(|expected| {
                let bytes = read_bounded(&path, limit)?;
                if source_stamp(&self.root, relative)?.as_ref() != Some(expected) {
                    return Err(stale());
                }
                Ok(bytes)
            })
            .transpose()?;
        self.sources.insert(relative.to_owned(), fingerprint);
        Ok(bytes)
    }

    fn add_tree(&mut self, relative: &str) -> Result<(), ProjectCreateError> {
        let tree = collect_tree(&self.root, relative)?;
        self.archive.add_stamps(&self.root, &tree)?;
        self.trees.push((relative.to_owned(), tree));
        Ok(())
    }

    fn verify(&self) -> Result<(), ProjectCreateError> {
        if stamp(&self.root, true)?.identity != self.root_identity
            || package_directories(&self.root)? != self.package_directories
        {
            return Err(stale());
        }
        for (relative, expected) in &self.sources {
            if &source_stamp(&self.root, relative)? != expected {
                return Err(stale());
            }
        }
        for (relative, expected) in &self.trees {
            if &collect_tree(&self.root, relative)? != expected {
                return Err(stale());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct Stamp {
    directory: bool,
    len: u64,
    modified: SystemTime,
    identity: Vec<u8>,
}

fn source_stamp(root: &Path, relative: &str) -> Result<Option<Stamp>, ProjectCreateError> {
    let mut path = root.to_path_buf();
    let mut components = relative.split('/').peekable();
    while let Some(component) = components.next() {
        path.push(component);
        if !optional_exists(&path)? {
            return Ok(None);
        }
        let directory = components.peek().is_some();
        let fingerprint = stamp(&path, directory)?;
        if !directory {
            return Ok(Some(fingerprint));
        }
    }
    Err(invalid())
}

fn package_directories(root: &Path) -> Result<Vec<String>, ProjectCreateError> {
    let packages = root.join("Packages");
    if !optional_exists(&packages)? {
        return Ok(Vec::new());
    }
    stamp(&packages, true)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(&packages).map_err(|error| io_error(&packages, error))? {
        let entry = entry.map_err(|error| io_error(&packages, error))?;
        let kind = entry
            .file_type()
            .map_err(|error| io_error(&entry.path(), error))?;
        if kind.is_dir() {
            if names.len() == LIMITS.entries {
                return Err(invalid());
            }
            names.push(entry.file_name().into_string().map_err(|_| invalid())?);
        }
    }
    names.sort();
    Ok(names)
}

fn collect_tree(
    root: &Path,
    relative: &str,
) -> Result<BTreeMap<String, Stamp>, ProjectCreateError> {
    let mut tree = BTreeMap::new();
    let path = root.join(relative);
    if !optional_exists(&path)? {
        return Ok(tree);
    }
    let mut pending = vec![(relative.to_owned(), stamp(&path, true)?)];
    while let Some((name, value)) = pending.pop() {
        if value.directory {
            let path = root.join(&name);
            for child in fs::read_dir(&path).map_err(|error| io_error(&path, error))? {
                let child = child.map_err(|error| io_error(&path, error))?;
                let child_name = child.file_name().into_string().map_err(|_| invalid())?;
                if pending.len() + tree.len() >= LIMITS.entries || child_name.contains('\\') {
                    return Err(invalid());
                }
                let name = format!("{name}/{child_name}");
                validate_path(&name)?;
                let metadata = fs::symlink_metadata(child.path())
                    .map_err(|error| io_error(&child.path(), error))?;
                pending.push((name, make_stamp(&child.path(), metadata)?));
            }
        }
        tree.insert(name, value);
    }
    Ok(tree)
}

enum Content {
    Directory,
    Bytes(Vec<u8>),
    File(PathBuf, Stamp),
}

#[derive(Default)]
struct Inventory {
    entries: BTreeMap<String, Content>,
}

impl Inventory {
    fn add_tree(&mut self, root: &Path, name: &str) -> Result<(), ProjectCreateError> {
        self.add_stamps(root, &collect_tree(root, name)?)
    }

    fn add_stamps(
        &mut self,
        root: &Path,
        tree: &BTreeMap<String, Stamp>,
    ) -> Result<(), ProjectCreateError> {
        for (name, value) in tree {
            let content = if value.directory {
                Content::Directory
            } else {
                Content::File(root.join(name), value.clone())
            };
            self.insert(name, content)?;
        }
        Ok(())
    }

    fn add_file(&mut self, root: &Path, name: &str) -> Result<(), ProjectCreateError> {
        let path = root.join(name);
        let identity = stamp(&path, false)?;
        self.insert(name, Content::File(path, identity))
    }

    fn add_bytes(&mut self, name: &str, bytes: Vec<u8>) -> Result<(), ProjectCreateError> {
        let mut child = name;
        while let Some((parent, _)) = child.rsplit_once('/') {
            if self.entries.contains_key(parent) {
                break;
            }
            self.insert(parent, Content::Directory)?;
            child = parent;
        }
        self.insert(name, Content::Bytes(bytes))
    }

    fn insert(&mut self, name: &str, content: Content) -> Result<(), ProjectCreateError> {
        validate_path(name)?;
        if self.entries.len() == LIMITS.entries
            || self.entries.insert(name.to_owned(), content).is_some()
        {
            return Err(invalid());
        }
        Ok(())
    }

    fn write(&mut self, destination: &Path) -> Result<(), ProjectCreateError> {
        // VCC requires an Assets directory even when the archive has no assets.
        if !self.entries.contains_key("Assets") {
            self.insert("Assets", Content::Directory)?;
        }
        let mut archive = zip::ZipWriter::new(new_file(destination)?);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .compression_level(Some(1));
        let mut total = 0u64;
        let mut folded = BTreeSet::new();
        for (name, content) in &self.entries {
            if !folded.insert(name.to_lowercase()) {
                return Err(invalid_at(destination, "template paths collide"));
            }
            let size = match content {
                Content::Directory => {
                    archive
                        .add_directory(format!("{name}/"), options)
                        .map_err(|error| invalid().with_source(error))?;
                    continue;
                }
                Content::Bytes(bytes) => bytes.len() as u64,
                Content::File(_, stamp) => stamp.len,
            };
            total = total
                .checked_add(size)
                .filter(|total| {
                    size <= LIMITS.entry_bytes && *total <= LIMITS.total_uncompressed_bytes
                })
                .ok_or_else(invalid)?;
            archive
                .start_file(name, options)
                .map_err(|error| invalid().with_source(error))?;
            match content {
                Content::Bytes(bytes) => archive
                    .write_all(bytes)
                    .map_err(|error| io_error(destination, error))?,
                Content::File(path, expected) => copy_stamped(path, expected, &mut archive)?,
                Content::Directory => unreachable!(),
            }
        }
        archive
            .finish()
            .map_err(|error| invalid().with_source(error))?
            .sync_all()
            .map_err(|error| io_error(destination, error))
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemplateMetadata {
    name: String,
    display_name: String,
}

fn template_metadata(id: &str, name: &str) -> Result<Vec<u8>, ProjectCreateError> {
    validate_metadata(id, name.trim())?;
    serde_json::to_vec(&TemplateMetadata {
        name: id.to_owned(),
        display_name: name.trim().to_owned(),
    })
    .map_err(|error| invalid().with_source(error))
}

fn validate_metadata(id: &str, name: &str) -> Result<(), ProjectCreateError> {
    if !valid_project_template_id(id)
        || name.trim().is_empty()
        || name.len() > 128
        || name.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_editor(version: &str) -> Result<(), ProjectCreateError> {
    canonical_unity_version(version)
        .map(|_| ())
        .map_err(|error| invalid().with_source(error))
}

fn validate_package_id(id: &str) -> Result<(), ProjectCreateError> {
    if id.len() > 128
        || !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(invalid());
    }
    Ok(())
}

fn project_range(range: &str) -> Result<String, ProjectCreateError> {
    ProjectDependencyRange::parse(range)
        .map(|range| range.canonical())
        .map_err(|error| invalid().with_source(error))
}

fn dependency_map(
    dependencies: Vec<ProjectTemplateDependency>,
) -> Result<BTreeMap<String, String>, ProjectCreateError> {
    if dependencies.len() > 128 {
        return Err(invalid());
    }
    let mut result = BTreeMap::new();
    for dependency in dependencies {
        validate_package_id(&dependency.package_id)?;
        let range = VpmRange::parse(&dependency.version_range)
            .map_err(|error| invalid().with_source(error))?
            .canonical();
        if result.insert(dependency.package_id, range).is_some() {
            return Err(invalid());
        }
    }
    Ok(result)
}

fn make_manifest(
    direct: &BTreeMap<String, String>,
    locked: &BTreeMap<String, String>,
) -> Result<Vec<u8>, ProjectCreateError> {
    let map = |entries: &BTreeMap<String, String>| {
        entries
            .iter()
            .map(|(id, version)| (id.clone(), json!({"version": version})))
            .collect::<serde_json::Map<_, _>>()
    };
    serde_json::to_vec(&json!({"dependencies": map(direct), "locked": map(locked)}))
        .map_err(|error| invalid().with_source(error))
}

type ManifestVersions = (BTreeMap<String, String>, BTreeMap<String, String>, Value);

fn read_vpm(bytes: &[u8]) -> Result<ManifestVersions, ProjectCreateError> {
    let value = parse_json(bytes)?;
    let read_map =
        |key: &str, limit: usize| -> Result<BTreeMap<String, String>, ProjectCreateError> {
            let Some(entries) = value.get(key) else {
                return Ok(BTreeMap::new());
            };
            let entries = entries
                .as_object()
                .filter(|entries| entries.len() <= limit)
                .ok_or_else(invalid)?;
            entries
                .iter()
                .map(|(id, value)| {
                    validate_package_id(id)?;
                    let version = value
                        .get("version")
                        .and_then(Value::as_str)
                        .ok_or_else(invalid)?;
                    let version = if key == "locked" {
                        version
                            .parse::<semver::Version>()
                            .map_err(|error| invalid().with_source(error))?
                            .to_string()
                    } else {
                        project_range(version)?
                    };
                    Ok((id.clone(), version))
                })
                .collect()
        };
    Ok((
        read_map("dependencies", 128)?,
        read_map("locked", 256)?,
        value,
    ))
}

fn verify_lock_closure(
    bytes: Option<&[u8]>,
    locked: &BTreeMap<String, String>,
) -> Result<(), ProjectCreateError> {
    if let Some(bytes) = bytes {
        let value = parse_json(bytes)?;
        if let Some(entries) = value.get("locked").and_then(Value::as_object) {
            for entry in entries.values() {
                if let Some(dependencies) = entry.get("dependencies") {
                    for id in dependencies.as_object().ok_or_else(invalid)?.keys() {
                        validate_package_id(id)?;
                        if !locked.contains_key(id) {
                            return Err(dependency_missing(id));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn read_declaration(
    path: &Path,
    archive_sha256: String,
) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
    let mut archive = open_zip(path)?;
    let metadata = zip_bytes(&mut archive, "package.json", 24 * 1024)?.ok_or_else(invalid)?;
    let metadata: TemplateMetadata =
        serde_json::from_slice(metadata.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&metadata))
            .map_err(|error| invalid().with_source(error))?;
    validate_metadata(&metadata.name, &metadata.display_name)?;
    let version_file = zip_bytes(&mut archive, EDITOR, crate::PROJECT_VERSION_LIMIT)?
        .ok_or_else(|| invalid_at(path, "template requires ProjectSettings/ProjectVersion.txt"))?;
    let (editor_version, _) = crate::parse_project_version(&version_file)
        .map_err(|error| invalid().with_source(error))?;
    validate_editor(&editor_version)?;
    if let Some(upm) = zip_bytes(&mut archive, UPM, crate::PROJECT_MANIFEST_LIMIT)? {
        let upm = parse_json(&upm)?;
        if let Some(dependencies) = upm.get("dependencies") {
            for (package_id, dependency) in dependencies.as_object().ok_or_else(invalid)? {
                let dependency = dependency.as_str().ok_or_else(invalid)?;
                if let Some((_, local)) = dependency
                    .split_once(':')
                    .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("file"))
                {
                    validate_local_dependency(&mut archive, path, package_id, local)?;
                }
            }
        }
    }
    let (dependencies, locked_versions, _) =
        zip_bytes(&mut archive, VPM, crate::PROJECT_MANIFEST_LIMIT)?
            .map(|bytes| read_vpm(&bytes))
            .transpose()?
            .unwrap_or_default();
    let dependencies = dependencies
        .into_iter()
        .map(|(package_id, version_range)| ProjectTemplateDependency {
            package_id,
            version_range,
        })
        .collect();
    let declaration = ProjectTemplateDeclaration {
        id: metadata.name,
        name: metadata.display_name.trim().to_owned(),
        editor_version: Some(editor_version),
        archive_sha256,
        dependencies,
        locked_versions,
    };
    if serde_json::to_vec(&declaration)
        .map_err(|error| invalid().with_source(error))?
        .len()
        > 24 * 1024
    {
        return Err(invalid());
    }
    Ok(declaration)
}

fn validate_local_dependency(
    archive: &mut zip::ZipArchive<File>,
    archive_path: &Path,
    package_id: &str,
    local: &str,
) -> Result<(), ProjectCreateError> {
    let nonportable = || {
        invalid_at(
            archive_path,
            "local UPM dependency must be included in the template",
        )
    };
    let mut relative = vec!["Packages"];
    // Unity resolves file: dependencies relative to Packages/manifest.json.
    let local = local.replace('\\', "/");
    for component in local.split('/') {
        match component {
            "." => {}
            ".." if relative.pop().is_some() => {}
            "" | ".." => return Err(nonportable()),
            name => relative.push(name),
        }
    }
    if relative.first().copied() != Some("Packages") || relative.len() < 2 {
        return Err(nonportable());
    }
    let relative = relative.join("/");
    validate_path(&relative)?;
    if relative.ends_with(".tgz") {
        if archive
            .by_name(&relative)
            .is_ok_and(|entry| !entry.is_dir())
        {
            return Ok(());
        }
        return Err(nonportable());
    }
    let manifest = format!("{relative}/package.json");
    let bytes =
        zip_bytes(archive, &manifest, crate::PROJECT_MANIFEST_LIMIT)?.ok_or_else(nonportable)?;
    if parse_json(&bytes)?.get("name").and_then(Value::as_str) != Some(package_id) {
        return Err(nonportable());
    }
    Ok(())
}

fn unpack(path: &Path, destination: &Path) -> Result<(), ProjectCreateError> {
    extract_archive_checked(path, destination, LIMITS, validate_layout)
        .map_err(|error| invalid().with_source(error))?;
    Ok(())
}

fn validate_layout(planned: &ArchivePreflight) -> Result<(), ArchiveErrorCode> {
    let mut manifest = false;
    let mut directories = BTreeMap::new();
    for entry in &planned.entries {
        let mut components = entry.relative_path.components();
        let root = components
            .next()
            .and_then(|part| part.as_os_str().to_str())
            .ok_or(ArchiveErrorCode::UnsafePath)?;
        if root == "package.json" && components.next().is_none() && !entry.directory {
            manifest = true;
        } else if optional_root_file(root) {
            if components.next().is_some() || entry.directory {
                return Err(ArchiveErrorCode::UnsafePath);
            }
        } else if !ROOTS.contains(&root) || (components.next().is_none() && !entry.directory) {
            return Err(ArchiveErrorCode::UnsafePath);
        }
        let name = entry.relative_path.to_string_lossy().replace('\\', "/");
        validate_path(&name).map_err(|_| ArchiveErrorCode::UnsafePath)?;
        for directory in entry
            .relative_path
            .ancestors()
            .skip(usize::from(!entry.directory))
        {
            let name = directory.to_string_lossy().into_owned();
            if directories
                .insert(name.to_lowercase(), name.clone())
                .is_some_and(|old| old != name)
            {
                return Err(ArchiveErrorCode::PathCollision);
            }
        }
    }
    if !manifest {
        return Err(ArchiveErrorCode::Invalid);
    }
    Ok(())
}

fn optional_root_file(name: &str) -> bool {
    license_root_file(name)
        || ["README.md", ".gitignore"]
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn license_root_file(name: &str) -> bool {
    ["LICENSE", "LICENSE.md", "LICENSE.txt"]
        .iter()
        .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<File>, ProjectCreateError> {
    let file = File::open(path).map_err(|error| io_error(path, error))?;
    zip::ZipArchive::new(file).map_err(|error| invalid().with_source(error))
}

fn zip_bytes(
    archive: &mut zip::ZipArchive<File>,
    name: &str,
    limit: usize,
) -> Result<Option<Vec<u8>>, ProjectCreateError> {
    let entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(invalid().with_source(error)),
    };
    if entry.is_dir() || entry.size() > limit as u64 {
        return Err(invalid());
    }
    let expected = entry.size();
    let mut bytes = Vec::new();
    entry
        .take(expected + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid().with_source(error))?;
    if bytes.len() as u64 != expected {
        return Err(invalid());
    }
    Ok(Some(bytes))
}

fn parse_json(bytes: &[u8]) -> Result<Value, ProjectCreateError> {
    let value: Value = serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
        .map_err(|error| invalid().with_source(error))?;
    if !value.is_object() {
        return Err(invalid());
    }
    Ok(value)
}

fn validate_path(name: &str) -> Result<(), ProjectCreateError> {
    let (normalized, _) =
        normalize_path(name, LIMITS).map_err(|error| invalid().with_source(error))?;
    if normalized.to_string_lossy().replace('\\', "/") != name
        || name.contains(['<', '>', '"', '|', '?', '*'])
    {
        return Err(invalid());
    }
    Ok(())
}

fn template_path(value: &str) -> Result<&Path, ProjectCreateError> {
    let path = Path::new(value);
    if !path.is_absolute()
        || !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case(TEMPLATE_EXTENSION))
    {
        return Err(invalid());
    }
    Ok(path)
}

fn optional_exists(path: &Path) -> Result<bool, ProjectCreateError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

fn stamp(path: &Path, directory: bool) -> Result<Stamp, ProjectCreateError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    let result = make_stamp(path, metadata)?;
    if result.directory != directory {
        return Err(invalid_at(path, "unexpected template input type"));
    }
    Ok(result)
}

fn make_stamp(path: &Path, metadata: Metadata) -> Result<Stamp, ProjectCreateError> {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    if reparse || metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(invalid_at(path, "unsafe template input type"));
    }
    Ok(Stamp {
        directory: metadata.is_dir(),
        len: metadata.len(),
        modified: metadata.modified().map_err(|error| io_error(path, error))?,
        identity: alcomd_platform::file_identity_key(path)
            .map_err(|error| io_error(path, error))?,
    })
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, ProjectCreateError> {
    let expected = stamp(path, false)?;
    if expected.len > limit as u64 {
        return Err(invalid_at(path, "template metadata exceeds its size limit"));
    }
    let mut bytes = Vec::new();
    copy_stamped(path, &expected, &mut bytes)?;
    Ok(bytes)
}

fn copy_stamped(
    path: &Path,
    expected: &Stamp,
    output: &mut impl Write,
) -> Result<(), ProjectCreateError> {
    if &stamp(path, false)? != expected {
        return Err(stale());
    }
    let file = File::open(path).map_err(|error| io_error(path, error))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if !opened.is_file()
        || opened.len() != expected.len
        || opened.modified().map_err(|error| io_error(path, error))? != expected.modified
        || &stamp(path, false)? != expected
    {
        return Err(stale());
    }
    let read = io::copy(&mut file.take(expected.len + 1), output)
        .map_err(|error| io_error(path, error))?;
    if read != expected.len || &stamp(path, false)? != expected {
        return Err(stale());
    }
    Ok(())
}

fn copy_checked_file(source: &Path, target: &Path, limit: u64) -> Result<(), ProjectCreateError> {
    let expected = stamp(source, false)?;
    if expected.len > limit {
        return Err(invalid_at(
            source,
            "template archive exceeds its size limit",
        ));
    }
    let mut output = new_file(target)?;
    copy_stamped(source, &expected, &mut output)?;
    output.sync_all().map_err(|error| io_error(target, error))
}

fn digest_file(path: &Path) -> Result<String, ProjectCreateError> {
    let expected = stamp(path, false)?;
    if expected.len > LIMITS.archive_bytes {
        return Err(invalid_at(path, "template archive exceeds its size limit"));
    }
    struct DigestWriter(Sha256);

    impl Write for DigestWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut hash = DigestWriter(Sha256::new());
    copy_stamped(path, &expected, &mut hash)?;
    Ok(hash
        .0
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn new_file(path: &Path) -> Result<File, ProjectCreateError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error(path, error))
}

struct Scratch {
    path: PathBuf,
    identity: Vec<u8>,
}

impl Scratch {
    fn new(root: &Path) -> Result<Self, ProjectCreateError> {
        let path = root.join(format!(".stage-{}", OperationId::new()));
        fs::create_dir(&path).map_err(|error| io_error(&path, error))?;
        let identity = stamp(&path, true)?.identity;
        let scratch = Self { path, identity };
        let marker = scratch.path.join("identity.json");
        let bytes =
            serde_json::to_vec(&scratch.identity).map_err(|error| invalid().with_source(error))?;
        let mut file = new_file(&marker)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| io_error(&marker, error))?;
        alcomd_platform::sync_directory(&scratch.path)
            .map_err(|error| io_error(&scratch.path, error))?;
        alcomd_platform::sync_directory(root).map_err(|error| io_error(root, error))?;
        Ok(scratch)
    }

    fn clean_interrupted(root: &Path) -> Result<(), ProjectCreateError> {
        for entry in fs::read_dir(root).map_err(|error| io_error(root, error))? {
            let entry = entry.map_err(|error| io_error(root, error))?;
            let name = entry.file_name();
            if !name
                .to_str()
                .and_then(|name| name.strip_prefix(".stage-"))
                .is_some_and(|id| OperationId::parse(id).is_ok())
            {
                continue;
            }
            let path = entry.path();
            let Ok(stamped) = stamp(&path, true) else {
                continue;
            };
            let Ok(marker) = read_bounded(&path.join("identity.json"), 4096) else {
                continue;
            };
            if serde_json::from_slice::<Vec<u8>>(&marker)
                .is_ok_and(|saved| saved == stamped.identity)
                && stamp(&path, true).is_ok_and(|current| current.identity == stamped.identity)
            {
                fs::remove_dir_all(&path).map_err(|error| io_error(&path, error))?;
            }
        }
        Ok(())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if stamp(&self.path, true).is_ok_and(|value| value.identity == self.identity) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn invalid() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::InvalidInput)
}

fn invalid_at(path: &Path, reason: &'static str) -> ProjectCreateError {
    invalid()
        .with_source(io::Error::new(io::ErrorKind::InvalidData, reason))
        .with_path(path.display().to_string())
}

fn io_error(path: &Path, error: io::Error) -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::Internal)
        .with_source(error)
        .with_path(path.display().to_string())
}

fn stale() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::TemplateStale)
}

fn target_exists(path: &Path) -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::TargetExists)
        .with_path(path.display().to_string())
}

fn dependency_missing(id: &str) -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::DependencyUnsatisfied)
        .with_package_id(id.to_owned())
}
