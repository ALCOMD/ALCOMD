use crate::commands::RustError;
use crate::state::{GuiConfigState, TemplatesState};
use crate::templates::{
    self, AlcomTemplate, ProjectTemplateInfo, alcom_template_project_archive_payload,
    new_user_template_id, parse_alcom_template, sanitize_template_file_stem,
    serialize_alcom_project_archive_template, serialize_alcom_template, template_id_can_be_base,
};
use crate::utils::trash_delete;
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tokio::sync::Mutex;
use vrc_get_vpm::environment::VccDatabaseConnection;
use vrc_get_vpm::io::{DefaultEnvironmentIo, IoTrait};
use vrc_get_vpm::is_valid_package_name;
use vrc_get_vpm::version::VersionRange;

static TEMPLATE_MUTATION_LOCK: Mutex<()> = Mutex::const_new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProjectTemplateKind {
    BuiltIn,
    Derived,
    ProjectArchive,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectTemplateSummary {
    pub(crate) display_name: String,
    pub(crate) id: String,
    pub(crate) unity_versions: Vec<String>,
    pub(crate) update_date: Option<String>,
    pub(crate) has_unity_packages: bool,
    pub(crate) has_project_archive: bool,
    pub(crate) available: bool,
    pub(crate) kind: ProjectTemplateKind,
    pub(crate) editable: bool,
    pub(crate) removable: bool,
    pub(crate) usable_as_base: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectTemplateDetails {
    #[serde(flatten)]
    pub(crate) summary: ProjectTemplateSummary,
    pub(crate) base_template_id: Option<String>,
    pub(crate) unity_version_range: Option<String>,
    pub(crate) vpm_dependencies: IndexMap<String, String>,
    pub(crate) unity_package_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectTemplateDefinition {
    pub(crate) display_name: String,
    pub(crate) base_template_id: String,
    pub(crate) unity_version_range: String,
    pub(crate) vpm_dependencies: IndexMap<String, String>,
    pub(crate) unity_package_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemovedProjectTemplate {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) kind: ProjectTemplateKind,
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateImportConflict {
    pub(crate) id: String,
    pub(crate) source_path: PathBuf,
    pub(crate) existing_name: String,
    pub(crate) existing_update_date: Option<chrono::DateTime<chrono::Utc>>,
    pub(crate) importing_name: String,
    pub(crate) importing_update_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateImportResult {
    pub(crate) imported: usize,
    pub(crate) failed: usize,
    pub(crate) duplicates: Vec<TemplateImportConflict>,
}

#[derive(Debug, Clone)]
pub(crate) struct TemplateImportOverride {
    pub(crate) id: String,
    pub(crate) source_path: PathBuf,
}

#[derive(Debug)]
pub(crate) enum TemplateOperationError {
    NotFound,
    NotEditable,
    NotRemovable,
    InvalidDefinition(String),
    InvalidUnityPackagePath { path: PathBuf, reason: String },
    Storage(io::Error),
    Trash(String),
}

impl TemplateOperationError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "template_not_found",
            Self::NotEditable => "template_not_editable",
            Self::NotRemovable => "template_not_removable",
            Self::InvalidDefinition(_) => "invalid_template_definition",
            Self::InvalidUnityPackagePath { .. } => "invalid_unity_package_path",
            Self::Storage(_) | Self::Trash(_) => "template_storage_error",
        }
    }
}

impl fmt::Display for TemplateOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("project template was not found"),
            Self::NotEditable => f.write_str("project template is not editable"),
            Self::NotRemovable => f.write_str("project template is not removable"),
            Self::InvalidDefinition(message) => f.write_str(message),
            Self::InvalidUnityPackagePath { path, reason } => {
                write!(f, "invalid Unity package path {}: {reason}", path.display())
            }
            Self::Storage(error) => write!(f, "project template storage error: {error}"),
            Self::Trash(error) => write!(f, "failed to move project template to trash: {error}"),
        }
    }
}

impl std::error::Error for TemplateOperationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for TemplateOperationError {
    fn from(value: io::Error) -> Self {
        Self::Storage(value)
    }
}

impl From<TemplateOperationError> for RustError {
    fn from(value: TemplateOperationError) -> Self {
        RustError::unrecoverable_str(value.to_string())
    }
}

pub(crate) async fn load_project_templates(
    io: &DefaultEnvironmentIo,
) -> Result<Vec<ProjectTemplateInfo>, TemplateOperationError> {
    let connection = VccDatabaseConnection::connect(io).await?;
    let unity_versions = connection
        .get_unity_installations()
        .iter()
        .filter_map(|unity| unity.version())
        .collect::<Vec<_>>();

    Ok(templates::load_resolve_all_templates(io, &unity_versions).await?)
}

pub(crate) fn project_template_summary(template: &ProjectTemplateInfo) -> ProjectTemplateSummary {
    let mut unity_versions = template.unity_versions.clone();
    unity_versions.sort_unstable_by(|left, right| right.cmp(left));
    unity_versions.dedup();
    let unity_versions = unity_versions
        .into_iter()
        .map(|version| version.to_string())
        .collect();
    let kind = project_template_kind(template);
    let editable = kind == ProjectTemplateKind::Derived && template.source_path.is_some();
    let removable = kind != ProjectTemplateKind::BuiltIn && template.source_path.is_some();

    ProjectTemplateSummary {
        display_name: template.display_name.clone(),
        id: template.id.clone(),
        unity_versions,
        update_date: template.update_date.map(|date| date.to_rfc3339()),
        has_unity_packages: template
            .alcom_template
            .as_ref()
            .is_some_and(|value| !value.unity_packages.is_empty()),
        has_project_archive: kind == ProjectTemplateKind::ProjectArchive,
        available: template.available,
        kind,
        editable,
        removable,
        usable_as_base: template.available && template_id_can_be_base(&template.id),
    }
}

pub(crate) fn project_template_details(template: &ProjectTemplateInfo) -> ProjectTemplateDetails {
    let alcom_template = template.alcom_template.as_ref();
    ProjectTemplateDetails {
        summary: project_template_summary(template),
        base_template_id: alcom_template.and_then(|value| value.base.clone()),
        unity_version_range: alcom_template
            .and_then(|value| value.unity_version.as_ref())
            .map(ToString::to_string),
        vpm_dependencies: alcom_template
            .map(|value| {
                value
                    .vpm_dependencies
                    .iter()
                    .map(|(name, range)| (name.clone(), range.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        unity_package_paths: alcom_template
            .map(|value| {
                value
                    .unity_packages
                    .iter()
                    .map(|path| path.to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub(crate) async fn get_project_template(
    io: &DefaultEnvironmentIo,
    template_id: &str,
) -> Result<ProjectTemplateDetails, TemplateOperationError> {
    let templates = load_project_templates(io).await?;
    templates
        .iter()
        .find(|template| template.id == template_id)
        .map(project_template_details)
        .ok_or(TemplateOperationError::NotFound)
}

pub(crate) async fn create_project_template(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    definition: ProjectTemplateDefinition,
) -> Result<ProjectTemplateDetails, TemplateOperationError> {
    let _guard = TEMPLATE_MUTATION_LOCK.lock().await;
    let templates = load_project_templates(io).await?;
    let template_id = new_user_template_id();
    let template = validated_template(&templates, Some(&template_id), definition).await?;
    let display_name = template.display_name.clone();
    let serialized = serialize_alcom_template(template).map_err(|error| {
        TemplateOperationError::InvalidDefinition(format!(
            "failed to serialize project template: {error}"
        ))
    })?;
    save_new_template_file(io, &display_name, &serialized).await?;
    refresh_templates(state, io).await?;
    get_project_template(io, &template_id).await
}

pub(crate) async fn update_project_template(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    template_id: &str,
    definition: ProjectTemplateDefinition,
) -> Result<ProjectTemplateDetails, TemplateOperationError> {
    let _guard = TEMPLATE_MUTATION_LOCK.lock().await;
    let templates = load_project_templates(io).await?;
    let existing = templates
        .iter()
        .find(|template| template.id == template_id)
        .ok_or(TemplateOperationError::NotFound)?;
    if project_template_kind(existing) != ProjectTemplateKind::Derived {
        return Err(TemplateOperationError::NotEditable);
    }
    let source_path = existing
        .source_path
        .clone()
        .ok_or(TemplateOperationError::NotEditable)?;
    let template = validated_template(&templates, Some(template_id), definition).await?;
    let serialized = serialize_alcom_template(template).map_err(|error| {
        TemplateOperationError::InvalidDefinition(format!(
            "failed to serialize project template: {error}"
        ))
    })?;
    io.write_atomic(&source_path, &serialized).await?;
    refresh_templates(state, io).await?;
    get_project_template(io, template_id).await
}

pub(crate) async fn remove_project_template(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    template_id: &str,
) -> Result<RemovedProjectTemplate, TemplateOperationError> {
    let _guard = TEMPLATE_MUTATION_LOCK.lock().await;
    let templates = load_project_templates(io).await?;
    let existing = templates
        .iter()
        .find(|template| template.id == template_id)
        .ok_or(TemplateOperationError::NotFound)?;
    let kind = project_template_kind(existing);
    if kind == ProjectTemplateKind::BuiltIn {
        return Err(TemplateOperationError::NotRemovable);
    }
    let source_path = existing
        .source_path
        .clone()
        .ok_or(TemplateOperationError::NotRemovable)?;
    let removed = RemovedProjectTemplate {
        id: existing.id.clone(),
        display_name: existing.display_name.clone(),
        kind,
    };

    trash_delete(io.resolve(&source_path))
        .await
        .map_err(|error| TemplateOperationError::Trash(error.to_string()))?;
    refresh_templates(state, io).await?;
    Ok(removed)
}

pub(crate) async fn export_project_template(
    io: &DefaultEnvironmentIo,
    template_id: &str,
    destination: &Path,
) -> Result<ProjectTemplateSummary, TemplateOperationError> {
    let templates = load_project_templates(io).await?;
    let template = templates
        .iter()
        .find(|template| template.id == template_id)
        .ok_or(TemplateOperationError::NotFound)?;
    let source_path = template
        .source_path
        .as_ref()
        .ok_or(TemplateOperationError::NotRemovable)?;
    tokio::fs::copy(io.resolve(source_path), destination).await?;
    Ok(project_template_summary(template))
}

pub(crate) async fn import_project_templates(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    source_paths: &[PathBuf],
) -> Result<TemplateImportResult, TemplateOperationError> {
    let _guard = TEMPLATE_MUTATION_LOCK.lock().await;
    let mut installed_ids = templates::load_alcom_templates(io)
        .await
        .into_iter()
        .filter_map(|(path, template)| template.id.clone().map(|id| (id, (path, template))))
        .collect::<HashMap<_, _>>();
    let mut result = TemplateImportResult {
        imported: 0,
        failed: 0,
        duplicates: Vec::new(),
    };

    for source_path in source_paths {
        let bytes = match tokio::fs::read(source_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                log::error!(
                    "failed to read project template {}: {error}",
                    source_path.display()
                );
                result.failed += 1;
                continue;
            }
        };
        let mut parsed = match parse_alcom_template(&bytes) {
            Ok(template) => template,
            Err(error) => {
                log::error!(
                    "invalid project template {}: {error}",
                    source_path.display()
                );
                result.failed += 1;
                continue;
            }
        };
        if let Some(id) = &parsed.id
            && let Some((_, existing)) = installed_ids.get(id)
        {
            result.duplicates.push(TemplateImportConflict {
                id: id.clone(),
                source_path: source_path.clone(),
                existing_name: existing.display_name.clone(),
                existing_update_date: existing.update_date,
                importing_name: parsed.display_name.clone(),
                importing_update_date: parsed.update_date,
            });
            continue;
        }

        let import_bytes = match ensure_imported_template_id(&mut parsed, &bytes) {
            Ok(bytes) => bytes,
            Err(error) => {
                log::error!(
                    "failed to prepare imported template {}: {error}",
                    source_path.display()
                );
                result.failed += 1;
                continue;
            }
        };
        match save_new_template_file(io, &parsed.display_name, &import_bytes).await {
            Ok(path) => {
                if let Some(id) = parsed.id.clone() {
                    installed_ids.insert(id, (path, parsed));
                }
                result.imported += 1;
            }
            Err(error) => {
                log::error!(
                    "failed to save imported template {}: {error}",
                    source_path.display()
                );
                result.failed += 1;
            }
        }
    }

    refresh_templates(state, io).await?;
    Ok(result)
}

pub(crate) async fn override_imported_project_templates(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    overrides: &[TemplateImportOverride],
) -> Result<usize, TemplateOperationError> {
    let _guard = TEMPLATE_MUTATION_LOCK.lock().await;
    let templates = load_project_templates(io).await?;
    let mut imported = 0;

    for import_override in overrides {
        let Some(existing) = templates
            .iter()
            .find(|template| template.id == import_override.id)
        else {
            log::error!("cannot override missing template {}", import_override.id);
            continue;
        };
        let Some(target_path) = existing.source_path.as_ref() else {
            log::error!("cannot override built-in template {}", import_override.id);
            continue;
        };
        let bytes = match tokio::fs::read(&import_override.source_path).await {
            Ok(bytes) => bytes,
            Err(error) => {
                log::error!(
                    "failed to read project template {}: {error}",
                    import_override.source_path.display()
                );
                continue;
            }
        };
        let parsed = match parse_alcom_template(&bytes) {
            Ok(parsed) => parsed,
            Err(error) => {
                log::error!(
                    "invalid project template {}: {error}",
                    import_override.source_path.display()
                );
                continue;
            }
        };
        if parsed.id.as_deref() != Some(import_override.id.as_str()) {
            log::error!(
                "template override id mismatch: expected {}, found {:?}",
                import_override.id,
                parsed.id
            );
            continue;
        }
        match io.write_atomic(target_path, &bytes).await {
            Ok(()) => imported += 1,
            Err(error) => log::error!(
                "failed to override project template {}: {error}",
                import_override.id
            ),
        }
    }

    refresh_templates(state, io).await?;
    Ok(imported)
}

pub(crate) async fn set_template_favorite(
    config: &GuiConfigState,
    template_id: String,
    favorite: bool,
) -> Result<(), RustError> {
    let mut config = config.load_mut().await?;
    if favorite {
        if !config.favorite_templates.contains(&template_id) {
            config.favorite_templates.push(template_id);
        }
    } else {
        config.favorite_templates.retain(|id| id != &template_id);
    }
    config.save().await?;
    Ok(())
}

async fn validated_template(
    templates: &[ProjectTemplateInfo],
    template_id: Option<&str>,
    definition: ProjectTemplateDefinition,
) -> Result<AlcomTemplate, TemplateOperationError> {
    let display_name = definition.display_name.trim().to_string();
    if display_name.is_empty() {
        return Err(TemplateOperationError::InvalidDefinition(
            "display_name must not be empty".to_string(),
        ));
    }
    validate_base_template(templates, &definition.base_template_id, template_id)?;
    let unity_version =
        VersionRange::from_str(definition.unity_version_range.trim()).map_err(|error| {
            TemplateOperationError::InvalidDefinition(format!(
                "unity_version_range is invalid: {error}"
            ))
        })?;
    let mut vpm_dependencies = IndexMap::new();
    for (package_name, version_range) in definition.vpm_dependencies {
        if !is_valid_package_name(&package_name) {
            return Err(TemplateOperationError::InvalidDefinition(format!(
                "invalid VPM package name: {package_name}"
            )));
        }
        let version_range = VersionRange::from_str(version_range.trim()).map_err(|error| {
            TemplateOperationError::InvalidDefinition(format!(
                "invalid version range for {package_name}: {error}"
            ))
        })?;
        vpm_dependencies.insert(package_name, version_range);
    }
    let mut unity_packages = Vec::with_capacity(definition.unity_package_paths.len());
    for path in definition.unity_package_paths {
        if !path.is_absolute() {
            return Err(TemplateOperationError::InvalidUnityPackagePath {
                path,
                reason: "path must be absolute".to_string(),
            });
        }
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("unitypackage"))
        {
            return Err(TemplateOperationError::InvalidUnityPackagePath {
                path,
                reason: "file extension must be .unitypackage".to_string(),
            });
        }
        let canonical = tokio::fs::canonicalize(&path).await.map_err(|error| {
            TemplateOperationError::InvalidUnityPackagePath {
                path: path.clone(),
                reason: error.to_string(),
            }
        })?;
        let metadata = tokio::fs::metadata(&canonical).await.map_err(|error| {
            TemplateOperationError::InvalidUnityPackagePath {
                path: path.clone(),
                reason: error.to_string(),
            }
        })?;
        if !metadata.is_file() {
            return Err(TemplateOperationError::InvalidUnityPackagePath {
                path,
                reason: "path must identify a regular file".to_string(),
            });
        }
        unity_packages.push(canonical);
    }

    Ok(AlcomTemplate {
        display_name,
        update_date: Some(chrono::Utc::now()),
        id: template_id.map(str::to_string),
        base: Some(definition.base_template_id),
        unity_version: Some(unity_version),
        vpm_dependencies,
        unity_packages,
        archive: None,
    })
}

fn validate_base_template(
    templates: &[ProjectTemplateInfo],
    base_template_id: &str,
    editing_template_id: Option<&str>,
) -> Result<(), TemplateOperationError> {
    let base = templates
        .iter()
        .find(|template| template.id == base_template_id)
        .ok_or_else(|| {
            TemplateOperationError::InvalidDefinition(format!(
                "base template was not found: {base_template_id}"
            ))
        })?;
    if !project_template_summary(base).usable_as_base {
        return Err(TemplateOperationError::InvalidDefinition(format!(
            "template cannot be used as a base: {base_template_id}"
        )));
    }

    let Some(editing_template_id) = editing_template_id else {
        return Ok(());
    };
    let by_id = templates
        .iter()
        .map(|template| (template.id.as_str(), template))
        .collect::<HashMap<_, _>>();
    let mut current = Some(base_template_id);
    let mut visited = HashSet::new();
    while let Some(id) = current {
        if id == editing_template_id {
            return Err(TemplateOperationError::InvalidDefinition(
                "base template would create a dependency cycle".to_string(),
            ));
        }
        if !visited.insert(id) {
            return Err(TemplateOperationError::InvalidDefinition(
                "base template dependency chain contains a cycle".to_string(),
            ));
        }
        current = by_id
            .get(id)
            .and_then(|template| template.alcom_template.as_ref())
            .and_then(|template| template.base.as_deref());
    }
    Ok(())
}

fn ensure_imported_template_id(
    parsed: &mut AlcomTemplate,
    original: &[u8],
) -> Result<Vec<u8>, TemplateOperationError> {
    if parsed.id.is_some() {
        return Ok(original.to_vec());
    }
    parsed.id = Some(new_user_template_id());
    if parsed.is_derived() {
        serialize_alcom_template(parsed.clone()).map_err(|error| {
            TemplateOperationError::InvalidDefinition(format!(
                "failed to assign imported template id: {error}"
            ))
        })
    } else if parsed.is_project_archive() {
        let payload = alcom_template_project_archive_payload(original).ok_or_else(|| {
            TemplateOperationError::InvalidDefinition(
                "project archive template has no payload".to_string(),
            )
        })?;
        serialize_alcom_project_archive_template(parsed.clone(), payload).map_err(|error| {
            TemplateOperationError::InvalidDefinition(format!(
                "failed to assign imported archive template id: {error}"
            ))
        })
    } else {
        Err(TemplateOperationError::InvalidDefinition(
            "unsupported project template kind".to_string(),
        ))
    }
}

async fn save_new_template_file(
    io: &DefaultEnvironmentIo,
    display_name: &str,
    serialized: &[u8],
) -> Result<PathBuf, TemplateOperationError> {
    let template_dir = Path::new(crate::storage::TEMPLATE_DIR);
    io.create_dir_all(template_dir).await?;
    let file_name = sanitize_template_file_stem(display_name);
    let mut candidates = Vec::with_capacity(12);
    candidates.push(
        template_dir
            .join(&file_name)
            .with_extension("alcomtemplate"),
    );
    for suffix in 1..=10 {
        candidates.push(
            template_dir
                .join(format!("{file_name}_{suffix}"))
                .with_extension("alcomtemplate"),
        );
    }
    candidates.push(
        template_dir
            .join(uuid::Uuid::new_v4().simple().to_string())
            .with_extension("alcomtemplate"),
    );

    for candidate in candidates {
        match io.metadata(&candidate).await {
            Ok(_) => continue,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                io.write_atomic(&candidate, serialized).await?;
                return Ok(candidate);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(TemplateOperationError::Storage(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to allocate a unique project template file name",
    )))
}

async fn refresh_templates(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
) -> Result<(), TemplateOperationError> {
    state.save(load_project_templates(io).await?);
    Ok(())
}

fn project_template_kind(template: &ProjectTemplateInfo) -> ProjectTemplateKind {
    match template.alcom_template.as_ref() {
        None => ProjectTemplateKind::BuiltIn,
        Some(template) if template.is_project_archive() => ProjectTemplateKind::ProjectArchive,
        Some(_) => ProjectTemplateKind::Derived,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use vrc_get_vpm::version::UnityVersion;

    const BLANK_TEMPLATE_ID: &str = "com.anatawa12.vrc-get.blank";

    fn definition(
        display_name: &str,
        unity_package_paths: Vec<PathBuf>,
    ) -> ProjectTemplateDefinition {
        ProjectTemplateDefinition {
            display_name: display_name.to_string(),
            base_template_id: BLANK_TEMPLATE_ID.to_string(),
            unity_version_range: "2022.x.x".to_string(),
            vpm_dependencies: IndexMap::new(),
            unity_package_paths,
        }
    }

    fn template(id: &str, base: Option<&str>, kind: ProjectTemplateKind) -> ProjectTemplateInfo {
        let alcom_template = match kind {
            ProjectTemplateKind::BuiltIn => None,
            ProjectTemplateKind::Derived => Some(AlcomTemplate {
                display_name: "Example".to_string(),
                update_date: None,
                id: Some(id.to_string()),
                base: base.map(str::to_string),
                unity_version: None,
                vpm_dependencies: IndexMap::new(),
                unity_packages: Vec::new(),
                archive: None,
            }),
            ProjectTemplateKind::ProjectArchive => None,
        };
        ProjectTemplateInfo {
            display_name: "Example".to_string(),
            id: id.to_string(),
            unity_versions: vec![
                UnityVersion::new_f1(2022, 3, 6),
                UnityVersion::new_f1(2022, 3, 22),
                UnityVersion::new_f1(2022, 3, 6),
            ],
            update_date: None,
            alcom_template,
            source_path: (kind != ProjectTemplateKind::BuiltIn)
                .then(|| PathBuf::from("Templates/example.alcomtemplate")),
            available: true,
        }
    }

    #[test]
    fn project_template_summary_is_discovery_focused() {
        let template = template(
            "com.example.template",
            Some("com.anatawa12.vrc-get.blank"),
            ProjectTemplateKind::Derived,
        );
        let summary = serde_json::to_value(project_template_summary(&template)).unwrap();

        assert_eq!(summary["displayName"], "Example");
        assert_eq!(summary["id"], "com.example.template");
        assert_eq!(
            summary["unityVersions"],
            serde_json::json!(["2022.3.22f1", "2022.3.6f1"])
        );
        assert_eq!(summary["kind"], "derived");
        assert_eq!(summary["editable"], true);
        assert_eq!(summary["removable"], true);
        assert_eq!(summary["usableAsBase"], true);
        assert_eq!(summary.get("sourcePath"), None);
        assert_eq!(summary["updateDate"], Value::Null);
    }

    #[test]
    fn base_template_validation_rejects_cycles() {
        let built_in = template(
            "com.anatawa12.vrc-get.blank",
            None,
            ProjectTemplateKind::BuiltIn,
        );
        let first = template(
            "com.example.first",
            Some("com.example.second"),
            ProjectTemplateKind::Derived,
        );
        let second = template(
            "com.example.second",
            Some("com.anatawa12.vrc-get.blank"),
            ProjectTemplateKind::Derived,
        );

        let error = validate_base_template(
            &[built_in, first, second],
            "com.example.first",
            Some("com.example.second"),
        )
        .unwrap_err();
        assert_eq!(error.code(), "invalid_template_definition");

        let first = template(
            "com.example.first",
            Some("com.example.second"),
            ProjectTemplateKind::Derived,
        );
        let second = template(
            "com.example.second",
            Some("com.example.first"),
            ProjectTemplateKind::Derived,
        );
        let error = validate_base_template(
            &[first, second],
            "com.example.first",
            Some("com.example.third"),
        )
        .unwrap_err();
        assert_eq!(error.code(), "invalid_template_definition");
    }

    #[test]
    fn project_template_create_update_round_trip_preserves_id_and_storage_path() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let temp = tempfile::tempdir().unwrap();
                let environment = temp.path().join("environment");
                tokio::fs::create_dir_all(&environment).await.unwrap();
                let attachment = temp.path().join("example.unitypackage");
                tokio::fs::write(&attachment, b"unity package")
                    .await
                    .unwrap();
                let attachment = tokio::fs::canonicalize(attachment).await.unwrap();
                let io = DefaultEnvironmentIo::new(environment.into_boxed_path());
                let state = TemplatesState::new();

                let error = update_project_template(
                    &state,
                    &io,
                    BLANK_TEMPLATE_ID,
                    definition("Built-in", vec![attachment.clone()]),
                )
                .await
                .unwrap_err();
                assert!(matches!(error, TemplateOperationError::NotEditable));

                let created = create_project_template(
                    &state,
                    &io,
                    definition("Created Template", vec![attachment.clone()]),
                )
                .await
                .unwrap();
                assert_eq!(created.summary.display_name, "Created Template");
                assert_eq!(created.base_template_id.as_deref(), Some(BLANK_TEMPLATE_ID));
                assert_eq!(
                    created.unity_package_paths,
                    vec![attachment.to_string_lossy()]
                );
                let template_id = created.summary.id.clone();
                let created_path = load_project_templates(&io)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|template| template.id == template_id)
                    .unwrap()
                    .source_path
                    .unwrap();

                let updated = update_project_template(
                    &state,
                    &io,
                    &template_id,
                    definition("Updated Template", vec![attachment.clone()]),
                )
                .await
                .unwrap();
                assert_eq!(updated.summary.id, template_id);
                assert_eq!(updated.summary.display_name, "Updated Template");
                let updated_path = load_project_templates(&io)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|template| template.id == updated.summary.id)
                    .unwrap()
                    .source_path
                    .unwrap();

                assert_eq!(updated_path, created_path);
                assert!(attachment.is_file());
            });
    }

    #[test]
    fn template_definition_rejects_relative_unity_package_path() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let templates = vec![template(
                    BLANK_TEMPLATE_ID,
                    None,
                    ProjectTemplateKind::BuiltIn,
                )];
                let result = validated_template(
                    &templates,
                    Some("com.example.template"),
                    definition(
                        "Example",
                        vec![PathBuf::from("relative/example.unitypackage")],
                    ),
                )
                .await;
                let Err(error) = result else {
                    panic!("relative Unity package path should be rejected");
                };

                assert!(matches!(
                    error,
                    TemplateOperationError::InvalidUnityPackagePath { .. }
                ));
            });
    }
}
