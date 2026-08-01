use crate::activity_log::{
    ActivityDetail, ActivityImportance, ActivityInput, ActivityKind, ActivityLogState,
    ActivitySource, operations, summarize_path,
};
use crate::backend::templates as template_operations;
use crate::commands::prelude::*;
use crate::utils::find_existing_parent_dir_or_home;
use indexmap::IndexMap;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State, Window};
use tauri_plugin_dialog::DialogExt;
use vrc_get_vpm::io::DefaultEnvironmentIo;

#[tauri::command]
#[specta::specta]
pub async fn environment_export_template(
    io: State<'_, DefaultEnvironmentIo>,
    window: Window,
    id: String,
) -> Result<(), RustError> {
    let app = window.app_handle().clone();
    let activity = app.state::<ActivityLogState>();
    let template = template_operations::get_project_template(io.inner(), &id).await?;
    if !template.summary.removable {
        return Err(template_operations::TemplateOperationError::NotRemovable.into());
    }
    let Some(path) = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_file_name(format!("{}.alcomtemplate", template.summary.display_name))
        .add_filter("ALCOMD3 Project Template", &["alcomtemplate"])
        .blocking_save_file()
        .map(|path| path.into_path_buf())
        .transpose()?
    else {
        activity.record_info(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Open,
                ActivityImportance::Secondary,
                operations::TEMPLATE_EXPORT,
                "Template export file picker cancelled",
            )
            .target(template.summary.display_name)
            .details(vec![ActivityDetail::new("template", id)]),
        );
        return Ok(());
    };

    let destination_path = path;
    activity
        .track_result(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::TEMPLATE_EXPORT,
                "Exporting template",
            )
            .target(template.summary.display_name)
            .details(vec![
                ActivityDetail::new("template", id.clone()),
                ActivityDetail::new("destinationPath", summarize_path(&destination_path)),
            ]),
            "Template exported",
            vec![
                ActivityDetail::new("template", id.clone()),
                ActivityDetail::new("destinationPath", summarize_path(&destination_path)),
            ],
            async move {
                template_operations::export_project_template(io.inner(), &id, &destination_path)
                    .await?;
                Ok(())
            },
        )
        .await
}

#[derive(Deserialize, Serialize, specta::Type)]
pub struct TauriAlcomTemplate {
    pub display_name: String,
    pub base: String,
    pub unity_version: Option<String>,
    pub vpm_dependencies: IndexMap<String, String>,
    pub unity_packages: Vec<String>,
}

impl From<template_operations::ProjectTemplateDetails> for TauriAlcomTemplate {
    fn from(value: template_operations::ProjectTemplateDetails) -> Self {
        Self {
            display_name: value.summary.display_name,
            base: value.base_template_id.unwrap_or_default(),
            unity_version: value.unity_version_range,
            vpm_dependencies: value.vpm_dependencies,
            unity_packages: value.unity_package_paths,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn environment_get_alcom_template(
    io: State<'_, DefaultEnvironmentIo>,
    id: String,
) -> Result<TauriAlcomTemplate, RustError> {
    let template = template_operations::get_project_template(io.inner(), &id).await?;
    if !template.summary.editable {
        return Err(template_operations::TemplateOperationError::NotEditable.into());
    }
    Ok(template.into())
}

#[tauri::command]
#[specta::specta]
pub async fn environment_pick_unity_packages(window: Window) -> Result<Vec<String>, RustError> {
    window
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("Unity Package", &["unitypackage"])
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.into_path_buf())
        .map_ok(|path| path.to_string_lossy().into_owned())
        .collect::<Result<Vec<_>, _>>()
}

#[derive(Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum TauriPickUnityPackageResult {
    NoFolderSelected,
    InvalidSelection,
    Successful { new_path: String },
}

#[tauri::command]
#[specta::specta]
pub async fn environment_pick_unity_package(
    window: Window,
    current: String,
) -> Result<TauriPickUnityPackageResult, RustError> {
    let Some(path) = window
        .dialog()
        .file()
        .set_parent(&window)
        .set_directory(find_existing_parent_dir_or_home(current.as_ref()))
        .add_filter("Unity Package", &["unitypackage"])
        .blocking_pick_file()
        .map(|path| path.into_path_buf())
        .transpose()?
    else {
        return Ok(TauriPickUnityPackageResult::NoFolderSelected);
    };

    let Ok(path) = path.into_os_string().into_string() else {
        return Ok(TauriPickUnityPackageResult::InvalidSelection);
    };

    Ok(TauriPickUnityPackageResult::Successful { new_path: path })
}

#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn environment_save_template(
    templates: State<'_, TemplatesState>,
    io: State<'_, DefaultEnvironmentIo>,
    app: AppHandle,
    id: Option<String>,
    base: String,
    name: String,
    unity_range: String,
    vpm_packages: Vec<(String, String)>,
    unity_packages: Vec<String>,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let vpm_package_count = vpm_packages.len();
    let unity_package_count = unity_packages.len();
    let editing_existing = id.is_some();
    let definition = template_operations::ProjectTemplateDefinition {
        display_name: name.clone(),
        base_template_id: base,
        unity_version_range: unity_range,
        vpm_dependencies: vpm_packages.into_iter().collect(),
        unity_package_paths: unity_packages.into_iter().map(PathBuf::from).collect(),
    };
    let template_target = id.clone().unwrap_or_else(|| name.clone());

    activity
        .track_result(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::TEMPLATE_SAVE,
                if editing_existing {
                    "Updating template"
                } else {
                    "Saving template"
                },
            )
            .target(name)
            .details(vec![
                ActivityDetail::new("template", template_target),
                ActivityDetail::new("vpmDependencies", vpm_package_count.to_string()),
                ActivityDetail::new("unityPackages", unity_package_count.to_string()),
            ]),
            if editing_existing {
                "Template updated"
            } else {
                "Template saved"
            },
            vec![
                ActivityDetail::new("vpmDependencies", vpm_package_count.to_string()),
                ActivityDetail::new("unityPackages", unity_package_count.to_string()),
            ],
            async move {
                if let Some(id) = id {
                    template_operations::update_project_template(
                        templates.inner(),
                        io.inner(),
                        &id,
                        definition,
                    )
                    .await?;
                } else {
                    template_operations::create_project_template(
                        templates.inner(),
                        io.inner(),
                        definition,
                    )
                    .await?;
                }
                Ok(())
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_remove_template(
    templates: State<'_, TemplatesState>,
    io: State<'_, DefaultEnvironmentIo>,
    app: AppHandle,
    id: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let template = template_operations::get_project_template(io.inner(), &id).await?;
    let template_name = template.summary.display_name;
    activity
        .track_result(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::TEMPLATE_REMOVE,
                "Removing template",
            )
            .target(template_name)
            .details(vec![ActivityDetail::new("template", id.clone())]),
            "Template removed",
            vec![ActivityDetail::new("template", id.clone())],
            async move {
                template_operations::remove_project_template(templates.inner(), io.inner(), &id)
                    .await?;
                Ok(())
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_import_template(
    window: Window,
    templates_state: State<'_, TemplatesState>,
    io: State<'_, DefaultEnvironmentIo>,
) -> Result<TauriImportTemplateResult, RustError> {
    let app = window.app_handle().clone();
    let activity = app.state::<ActivityLogState>();
    let source_paths = window
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("ALCOMD3 Project Template", &["alcomtemplate"])
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.into_path_buf())
        .collect::<Result<Vec<_>, _>>()?;

    if source_paths.is_empty() {
        activity.record_info(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Open,
                ActivityImportance::Secondary,
                operations::TEMPLATE_IMPORT,
                "Template import file picker cancelled",
            ),
        );
        return Ok(TauriImportTemplateResult::default());
    }

    let tracker = activity.start_activity(
        Some(&app),
        ActivityInput::new(
            ActivitySource::Gui,
            ActivityKind::Write,
            ActivityImportance::Primary,
            operations::TEMPLATE_IMPORT,
            "Importing templates",
        )
        .details(vec![ActivityDetail::new(
            "selectedFiles",
            source_paths.len().to_string(),
        )]),
    );
    let result = import_templates(templates_state.inner(), io.inner(), &source_paths).await;
    let details = vec![
        ActivityDetail::new("selectedFiles", source_paths.len().to_string()),
        ActivityDetail::new("imported", result.imported.to_string()),
        ActivityDetail::new("failed", result.failed.to_string()),
        ActivityDetail::new("duplicates", result.duplicates.len().to_string()),
    ];
    if result.imported == 0 && result.duplicates.is_empty() && result.failed > 0 {
        activity.finish_failed(
            Some(&app),
            &tracker,
            "Template import failed",
            details,
            format!("failed to import {} selected templates", result.failed),
        );
    } else if result.failed > 0 {
        activity.finish_info(
            Some(&app),
            &tracker,
            "Template import partially completed",
            details,
        );
    } else {
        activity.finish_success(Some(&app), &tracker, "Template import completed", details);
    }

    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn environment_import_template_override(
    templates: State<'_, TemplatesState>,
    io: State<'_, DefaultEnvironmentIo>,
    app: AppHandle,
    import_override: Vec<TauriImportDuplicated>,
) -> Result<usize, RustError> {
    let activity = app.state::<ActivityLogState>();
    let override_count = import_override.len();
    let tracker = activity.start_activity(
        Some(&app),
        ActivityInput::new(
            ActivitySource::Gui,
            ActivityKind::Write,
            ActivityImportance::Primary,
            operations::TEMPLATE_IMPORT,
            "Overriding imported templates",
        )
        .details(vec![ActivityDetail::new(
            "duplicates",
            override_count.to_string(),
        )]),
    );
    let overrides = import_override
        .into_iter()
        .map(|value| template_operations::TemplateImportOverride {
            id: value.id,
            source_path: value.source_path,
        })
        .collect::<Vec<_>>();
    let imported = template_operations::override_imported_project_templates(
        templates.inner(),
        io.inner(),
        &overrides,
    )
    .await?;

    let failed = override_count.saturating_sub(imported);
    let details = vec![
        ActivityDetail::new("duplicates", override_count.to_string()),
        ActivityDetail::new("imported", imported.to_string()),
        ActivityDetail::new("failed", failed.to_string()),
    ];
    if imported == override_count {
        activity.finish_success(
            Some(&app),
            &tracker,
            "Imported templates overridden",
            details,
        );
    } else if imported == 0 && override_count > 0 {
        activity.finish_failed(
            Some(&app),
            &tracker,
            "Imported template override failed",
            details,
            format!("failed to override {failed} imported templates"),
        );
    } else {
        activity.finish_info(
            Some(&app),
            &tracker,
            "Imported template override partially completed",
            details,
        );
    }

    Ok(imported)
}

#[derive(Default, Serialize, Deserialize, Clone, specta::Type)]
pub struct TauriImportTemplateResult {
    imported: usize,
    failed: usize,
    duplicates: Vec<TauriImportDuplicated>,
}

#[derive(Serialize, Deserialize, Clone, specta::Type)]
pub struct TauriImportDuplicated {
    id: String,
    source_path: PathBuf,
    existing_name: String,
    existing_update_date: Option<chrono::DateTime<chrono::Utc>>,
    importing_name: String,
    importing_update_date: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<template_operations::TemplateImportConflict> for TauriImportDuplicated {
    fn from(value: template_operations::TemplateImportConflict) -> Self {
        Self {
            id: value.id,
            source_path: value.source_path,
            existing_name: value.existing_name,
            existing_update_date: value.existing_update_date,
            importing_name: value.importing_name,
            importing_update_date: value.importing_update_date,
        }
    }
}

pub async fn import_templates(
    state: &TemplatesState,
    io: &DefaultEnvironmentIo,
    source_paths: &[PathBuf],
) -> TauriImportTemplateResult {
    match template_operations::import_project_templates(state, io, source_paths).await {
        Ok(result) => TauriImportTemplateResult {
            imported: result.imported,
            failed: result.failed,
            duplicates: result.duplicates.into_iter().map(Into::into).collect(),
        },
        Err(error) => {
            log::error!("failed to import project templates: {error}");
            TauriImportTemplateResult {
                imported: 0,
                failed: source_paths.len(),
                duplicates: Vec::new(),
            }
        }
    }
}
