//! Selective template import through the existing native template operations.
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_import::TemplateSource;
use alcomd_protocol::{
    CAPABILITY_PROJECT_TEMPLATES_V1, CAPABILITY_TEMPLATE_EDIT_V1, OperationState,
    ProjectTemplateDependency, ProjectTemplateGetParams, ProjectTemplateRef, ProjectTemplateSource,
    ProjectTemplatesSaveParams,
};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::selection::Source;
use crate::templates::{DirectoryTemplate, Inventory};

#[derive(Serialize)]
enum Selected {
    Vcc(Vec<DirectoryTemplate>),
    Legacy(Inventory),
}

impl Selected {
    fn archives(&self) -> Vec<&str> {
        match self {
            Self::Vcc(items) => items.iter().map(DirectoryTemplate::relative).collect(),
            Self::Legacy(items) => items
                .attachments
                .iter()
                .map(crate::templates::Archive::relative)
                .collect(),
        }
    }

    fn count(&self) -> usize {
        match self {
            Self::Vcc(items) => items.len(),
            Self::Legacy(items) => items.templates.len(),
        }
    }

    fn skipped(&self) -> usize {
        match self {
            Self::Vcc(_) => 0,
            Self::Legacy(items) => items.skipped_derived,
        }
    }
}

pub(super) fn run(
    root: &Path,
    identity: &[u8],
    source: Source,
    approved: Option<&str>,
    staging: Option<&Path>,
) -> Result<Value> {
    let selected = match source {
        Source::Vcc => Selected::Vcc(crate::templates::inventory_vcc(root)?),
        Source::Alcom | Source::Alcomd3Beta => {
            Selected::Legacy(crate::templates::inventory_legacy(root)?)
        }
    };
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(root)?.1 == identity,
        "selected source was replaced"
    );
    let plan = serde_json::to_vec(&(
        "alcomd-selective-templates-v1",
        source,
        root,
        identity,
        &selected,
    ))?;
    let digest = crate::hex_digest(&Sha256::digest(plan));
    let archives = selected.archives();
    let Some(approved) = approved else {
        ensure!(staging.is_none(), "only apply accepts template staging");
        return Ok(json!({
            "source": source, "category": "templates", "planDigest": digest,
            "templates": selected.count(), "archives": archives.len(), "skippedDerived": selected.skipped(),
        }));
    };
    ensure!(
        approved == digest,
        "source no longer matches the approved selection"
    );
    let stage = if archives.is_empty() {
        ensure!(staging.is_none(), "this selection has no template archives");
        None
    } else {
        let staging = staging.context("template archives require a staging directory")?;
        Some(stage_archives(
            root, identity, staging, &selected, &archives,
        )?)
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let count = runtime.block_on(async {
        let config = ClientConfig::for_installed_core()
            .without_daemon_start()
            .with_client_name("alcomd-selective-import");
        let mut core = AlcomdClient::connect(config).await?;
        apply(&mut core, &selected, &digest, stage.as_deref()).await
    })?;
    Ok(json!({"planDigest": digest, "merged": {"templatesConfirmed": count}}))
}

fn stage_archives(
    source: &Path,
    source_identity: &[u8],
    output: &Path,
    selected: &Selected,
    archives: &[&str],
) -> Result<PathBuf> {
    ensure!(
        output.is_absolute()
            && !output
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir)),
        "invalid absolute template staging destination"
    );
    let (parent, parent_id) = alcomd_platform::resolve_plain_storage_directory(
        output
            .parent()
            .context("template staging parent is missing")?,
    )?;
    let output = parent.join(
        output
            .file_name()
            .context("template staging name is missing")?,
    );
    ensure!(
        !output.starts_with(source),
        "template staging cannot be inside the source profile"
    );
    let mut stage = crate::Stage::create(&parent)?;
    let folder = stage.path.join("templates");
    fs::create_dir(&folder)?;
    match selected {
        Selected::Vcc(items) => {
            for template in items {
                template.convert(source, &stage.path)?;
            }
        }
        Selected::Legacy(items) => {
            for archive in &items.attachments {
                archive.convert(source, &stage.path)?;
            }
        }
    }
    let mut bytes = 0_u64;
    for name in archives {
        bytes = bytes
            .checked_add(crate::file_summary(&stage.path, name)?.size)
            .context("template staging is too large")?;
        ensure!(
            bytes <= crate::ATTACHMENTS_LIMIT,
            "template staging is too large"
        );
    }
    alcomd_platform::sync_directory(&folder)?;
    alcomd_platform::sync_directory(&stage.path)?;
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(source)?.1 == source_identity
            && alcomd_platform::resolve_plain_storage_directory(&parent)?.1 == parent_id
            && stage.current(),
        "template source or staging was replaced"
    );
    match alcomd_platform::publish_path_no_replace(&stage.path, &output) {
        Ok(()) => {
            stage.published = true;
            alcomd_platform::sync_directory(&parent)?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            verify_prior_stage(&stage.path, &output, archives)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(output)
}

fn verify_prior_stage(expected: &Path, observed: &Path, archives: &[&str]) -> Result<()> {
    let (_, identity) = alcomd_platform::resolve_plain_storage_directory(observed)?;
    let folder = observed.join("templates");
    let (_, folder_id) = alcomd_platform::resolve_plain_storage_directory(&folder)?;
    ensure!(
        fs::read_dir(observed)?.count() == 1 && fs::read_dir(&folder)?.count() == archives.len(),
        "template staging contains other files"
    );
    for name in archives {
        let expected = crate::file_summary(expected, name)?;
        let path = observed.join(name);
        let (_, file_id) = alcomd_platform::open_plain_file(&path)?;
        let observed = crate::file_summary(observed, name)?;
        ensure!(
            observed.size == expected.size
                && observed.sha256 == expected.sha256
                && alcomd_platform::open_plain_file(&path)?.1 == file_id,
            "template staging differs from the approved source"
        );
    }
    ensure!(
        alcomd_platform::resolve_plain_storage_directory(observed)?.1 == identity
            && alcomd_platform::resolve_plain_storage_directory(&folder)?.1 == folder_id,
        "template staging was replaced"
    );
    Ok(())
}

async fn apply(
    core: &mut AlcomdClient,
    selected: &Selected,
    digest: &str,
    stage: Option<&Path>,
) -> Result<usize> {
    ensure!(
        core.supports_capability(CAPABILITY_PROJECT_TEMPLATES_V1),
        "Core lacks the project template registry"
    );
    match selected {
        Selected::Vcc(items) => {
            for (index, item) in items.iter().enumerate() {
                save(core, archive_source(stage, item.relative())?, digest, index).await?;
            }
        }
        Selected::Legacy(items) => {
            ensure!(
                items
                    .templates
                    .iter()
                    .all(|item| matches!(item.source, TemplateSource::ProjectArchive { .. }))
                    || core.supports_capability(CAPABILITY_TEMPLATE_EDIT_V1),
                "Core lacks template base lookup"
            );
            for (index, template) in items.templates.iter().enumerate() {
                let source = match &template.source {
                    TemplateSource::ProjectArchive { archive_path, .. } => {
                        archive_source(stage, archive_path)?
                    }
                    TemplateSource::Manual {
                        base_template,
                        unity_range,
                        dependencies,
                        unity_packages,
                    } => {
                        ensure!(
                            unity_packages.is_empty(),
                            "template includes unsupported Unity packages"
                        );
                        let base = core
                            .template_get(ProjectTemplateGetParams {
                                id: base_template.clone(),
                                cursor: None,
                                limit: Some(1),
                            })
                            .await?;
                        ProjectTemplateSource::Manual {
                            id: template.id.clone().context("missing template identifier")?,
                            name: template.name.clone(),
                            unity_version: unity_range.clone(),
                            dependencies: dependencies
                                .iter()
                                .map(|(package_id, version_range)| ProjectTemplateDependency {
                                    package_id: package_id.clone(),
                                    version_range: version_range.clone(),
                                })
                                .collect(),
                            base_template: ProjectTemplateRef {
                                id: base_template.clone(),
                                revision: base.template.revision,
                            },
                        }
                    }
                };
                save(core, source, digest, index).await?;
            }
        }
    }
    Ok(selected.count())
}

fn archive_source(stage: Option<&Path>, relative: &str) -> Result<ProjectTemplateSource> {
    let path = stage
        .context("template archive staging is unavailable")?
        .join(relative);
    Ok(ProjectTemplateSource::File {
        source_file: path
            .into_os_string()
            .into_string()
            .map_err(|_| anyhow::anyhow!("template archive path must be Unicode"))?,
    })
}

async fn save(
    core: &mut AlcomdClient,
    source: ProjectTemplateSource,
    digest: &str,
    index: usize,
) -> Result<()> {
    let receipt = core
        .project_templates_save(ProjectTemplatesSaveParams {
            source,
            expected_revision: None,
            idempotency_key: format!("select:{digest}:template:{index}"),
        })
        .await?;
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let operation = core.operation_get(receipt.operation_id.clone()).await?;
            match operation.state {
                OperationState::Succeeded => return Ok(()),
                OperationState::Failed | OperationState::Cancelled => {
                    anyhow::bail!("template import operation ended without saving")
                }
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
    })
    .await
    .context("template import is still pending; retry with the same plan and staging directory")?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vcc_project(source: &Path) -> Result<PathBuf> {
        let project = source.join("Templates/示例 Project");
        for name in ["Assets", "Packages", "ProjectSettings"] {
            fs::create_dir_all(project.join(name))?;
        }
        fs::write(project.join("Assets/example.txt"), "payload")?;
        fs::write(
            project.join("Packages/manifest.json"),
            r#"{"dependencies":{}}"#,
        )?;
        fs::write(
            project.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )?;
        Ok(project)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn vcc_template_is_imported_through_core_without_replacing_an_existing_record()
    -> Result<()> {
        #[cfg(windows)]
        let _ipc_guard = crate::SELECTION_IPC_LOCK.lock().await;
        // Use the same short root as the other real IPC fixtures on macOS.
        #[cfg(target_os = "macos")]
        let temporary_root = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let temporary_root = std::env::temp_dir();
        let fixture = crate::Stage::create(&temporary_root.canonicalize()?)?;
        let root = &fixture.path;
        let source = root.join("source");
        let project = vcc_project(&source)?;
        let identity = alcomd_platform::resolve_plain_storage_directory(&source)?.1;
        let selected = Selected::Vcc(crate::templates::inventory_vcc(&source)?);
        let archives = selected.archives();
        let destination = root.join("prepared");
        let stage = stage_archives(&source, &identity, &destination, &selected, &archives)?;
        stage_archives(&source, &identity, &destination, &selected, &archives)?;
        let mut archive = zip::ZipArchive::new(fs::File::open(stage.join(archives[0]))?)?;
        assert!(archive.by_name("Assets/example.txt").is_ok());
        drop(archive);

        let mut config = ClientConfig::default().without_daemon_start();
        let ipc = if cfg!(unix) {
            config = config.with_runtime_directory(root.join("run"));
            alcomd_platform::IpcConfig::isolated(root.join("run"))
        } else {
            alcomd_platform::IpcConfig::default()
        };
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let daemon = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            alcomd_platform::DataConfig::isolated(root.join("data")),
            async {
                let _ = stopped.await;
            },
        )
        .await?;
        let mut core = AlcomdClient::connect(config).await?;
        assert_eq!(
            apply(&mut core, &selected, "vcc-fixture", Some(&stage)).await?,
            1
        );
        assert_eq!(
            apply(&mut core, &selected, "vcc-fixture", Some(&stage)).await?,
            1
        );
        let listing = core.project_templates_list(Default::default()).await?;
        let saved = listing
            .templates
            .iter()
            .filter(|item| item.id.starts_with("com.vrclearn.alcomd.migration.vcc."))
            .collect::<Vec<_>>();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].name, "示例 Project");
        assert_eq!(
            fs::read_to_string(project.join("Assets/example.txt"))?,
            "payload"
        );

        fs::write(project.join("Assets/example.txt"), "changed")?;
        assert!(stage_archives(&source, &identity, &destination, &selected, &archives).is_err());
        let changed = Selected::Vcc(crate::templates::inventory_vcc(&source)?);
        assert!(
            stage_archives(
                &source,
                &identity,
                &destination,
                &changed,
                &changed.archives()
            )
            .is_err()
        );
        let changed_stage = stage_archives(
            &source,
            &identity,
            &root.join("different"),
            &changed,
            &changed.archives(),
        )?;
        assert!(
            apply(
                &mut core,
                &changed,
                "different-fixture",
                Some(&changed_stage)
            )
            .await
            .is_err()
        );
        let after = core.project_templates_list(Default::default()).await?;
        assert_eq!(
            after.templates.iter().find(|item| item.id == saved[0].id),
            Some(saved[0])
        );
        drop(core);
        let _ = stop.send(());
        daemon.await??;
        Ok(())
    }
}
