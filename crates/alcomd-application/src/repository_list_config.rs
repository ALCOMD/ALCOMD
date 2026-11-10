//! Configuration exchange uses existing repository registration and settings authority.
use super::*;
use crate::{ConfigPackageSettingsUpdate, ConfigRepositoryPackages, ConfigUpdate, RepositoryId};
use std::collections::{BTreeMap, HashSet};

impl<S: RepositoryListStore, F: RepositoryListFiles> RepositoryListApplication<S, F> {
    pub async fn preview_format(
        &self,
        access: &AccessContext,
        format: RepositoryListFormat,
    ) -> Result<RepositoryListPreview, RepositoryListError> {
        if format == RepositoryListFormat::Legacy {
            return self.preview(access).await;
        }
        require(access, true)?;
        let records = self.configured_records(access).await?;
        Ok(self.files.prepare_configuration(records).await?.preview)
    }

    pub async fn export_format(
        &self,
        access: &AccessContext,
        path: String,
        expected_fingerprint: String,
        format: RepositoryListFormat,
    ) -> Result<RepositoryListExport, RepositoryListError> {
        if format == RepositoryListFormat::Legacy {
            return self.export(access, path, expected_fingerprint).await;
        }
        require(access, true)?;
        let snapshot = self.snapshot(access).await?;
        let records = self
            .attach_configuration(access, snapshot.repositories)
            .await?;
        let prepared = self.files.prepare_configuration(records).await?;
        if prepared.preview.fingerprint != expected_fingerprint {
            return Err(RepositoryListError("repository_list_changed"));
        }
        let result = RepositoryListExport {
            exported_count: prepared.preview.exportable_count,
            byte_count: prepared.preview.byte_count,
            sha256: prepared.sha256,
        };
        crate::log_execution_step(
            "repositories.exportListFile",
            "write_repository_configuration",
            Some(crate::log_path("file", path.clone())),
            self.files
                .create_new(path, prepared.bytes, snapshot.project_roots),
            |_| None,
        )
        .await?;
        crate::flush_log_steps().await;
        Ok(result)
    }

    async fn configured_records(
        &self,
        access: &AccessContext,
    ) -> Result<Vec<RepositoryListRecord>, RepositoryListError> {
        self.attach_configuration(access, self.snapshot(access).await?.repositories)
            .await
    }

    async fn attach_configuration(
        &self,
        access: &AccessContext,
        mut records: Vec<RepositoryListRecord>,
    ) -> Result<Vec<RepositoryListRecord>, RepositoryListError> {
        let settings = self
            .settings
            .as_ref()
            .ok_or(RepositoryListError("internal_error"))?
            .get_settings(access)
            .await
            .map_err(config_error)?;
        for record in &mut records {
            record.configuration.enabled = !settings
                .settings
                .packages
                .hidden_repository_ids
                .contains(&record.repository_id);
            record.configuration.disabled_package_ids = settings
                .settings
                .packages
                .hidden_repository_packages
                .iter()
                .find(|group| group.repository_id == record.repository_id)
                .map(|group| group.package_ids.clone())
                .unwrap_or_default();
        }
        Ok(records)
    }

    pub(super) async fn import_configuration<R: M3ReadAdapter>(
        &self,
        access: &AccessContext,
        registry: &M3Application<S, R>,
        inspection: RepositoryListInspection,
        key: IdempotencyKey,
        mut stop: tokio::sync::watch::Receiver<bool>,
        overwrite_existing: bool,
    ) -> Result<Vec<(u32, RepositoryBatchItem)>, RepositoryListError>
    where
        S: M3RegistryStore,
    {
        access
            .require(Permission::SettingsRead)
            .map_err(|_| RepositoryListError("permission_denied"))?;
        access
            .require(Permission::SettingsManage)
            .map_err(|_| RepositoryListError("permission_denied"))?;
        let official = self
            .settings
            .as_ref()
            .ok_or(RepositoryListError("internal_error"))?;
        let _import_guard = tokio::select! {
            biased;
            _ = stop.wait_for(|stopped| *stopped) => return Ok(inspection.entries.iter().map(|entry| (entry.line,RepositoryBatchItem::Unprocessed)).collect()),
            guard = registry.repository_import.lock() => guard,
        };
        let before = self.snapshot(access).await?;
        let mut jobs = tokio::task::JoinSet::new();
        let permits = Arc::new(tokio::sync::Semaphore::new(DOWNLOAD_CONCURRENCY));
        // Reads may overlap; serialize admission to the store and stop further
        // registrations if a write's outcome cannot be determined.
        let commits = Arc::new(tokio::sync::Mutex::new(false));
        for entry in inspection.entries {
            let Some(configuration) = entry.configuration else {
                continue;
            };
            let Some(source) = entry.source else { continue };
            let existing = before
                .repositories
                .iter()
                .find(|record| record.source == source)
                .cloned();
            let registry = registry.clone();
            let access = access.clone();
            let permits = Arc::clone(&permits);
            let commits = Arc::clone(&commits);
            let mut stopped = stop.clone();
            let operation_key = key.as_str().to_owned();
            jobs.spawn(async move {
                if existing.is_some() && !overwrite_existing {
                    return (entry.line, RepositoryBatchItem::Existing, None);
                }
                let permit = tokio::select! {
                    biased;
                    _ = stopped.wait_for(|value| *value) => None,
                    permit = permits.acquire_owned() => permit.ok(),
                };
                let Some(_permit) = permit else {
                    return (entry.line, RepositoryBatchItem::Unprocessed, None);
                };
                if *stopped.borrow() || *commits.lock().await {
                    return (entry.line, RepositoryBatchItem::Unprocessed, None);
                }
                let admission_key =
                    IdempotencyKey::parse(format!("config:{operation_key}:{}", entry.line))
                        .expect("bounded import key");
                let observation = if existing.is_none() {
                    match crate::log_execution_step(
                        "repositories.register",
                        "fetch_repository",
                        None,
                        registry.reader.inspect_repository_authenticated(
                            source,
                            None,
                            entry.credentials.clone(),
                        ),
                        |error| error.technical_path().map(str::to_owned),
                    )
                    .await
                    {
                        Ok(crate::RepositoryReadOutcome::Fresh(observation)) => Some(observation),
                        Ok(crate::RepositoryReadOutcome::NotModified(_)) => {
                            return (
                                entry.line,
                                RepositoryBatchItem::Failed(M3ErrorCode::RepositoryUnavailable),
                                None,
                            );
                        }
                        Err(error) => {
                            return (entry.line, RepositoryBatchItem::Failed(error.code()), None);
                        }
                    }
                } else {
                    None
                };
                let mut uncertain = commits.lock().await;
                if *uncertain {
                    return (entry.line, RepositoryBatchItem::Unprocessed, None);
                }
                let result = match existing {
                    Some(record) => Ok((record.repository_id, false)),
                    None => {
                        let observation = observation.expect("new source was read");
                        let result = match entry.credentials.clone() {
                            Some(credentials) => {
                                registry
                                    .persist_authenticated_repository(
                                        &access,
                                        observation,
                                        credentials,
                                        admission_key,
                                    )
                                    .await
                            }
                            None => {
                                registry
                                    .persist_repository(&access, observation, admission_key)
                                    .await
                            }
                        };
                        result.map(|saved| (saved.value.repository_id.to_string(), true))
                    }
                };
                let (id, added) = match result {
                    Ok(result) => result,
                    Err(error) => {
                        *uncertain = matches!(
                            error.code(),
                            M3ErrorCode::Internal | M3ErrorCode::StoreUnavailable
                        );
                        return (
                            entry.line,
                            if *uncertain {
                                RepositoryBatchItem::Unknown(error.code())
                            } else {
                                RepositoryBatchItem::Failed(error.code())
                            },
                            None,
                        );
                    }
                };
                let mut authentication_changed = false;
                if !added {
                    let id_value = RepositoryId::parse(&id).expect("registered repository ID");
                    let auth = registry
                        .store
                        .repository_credential_reference(access.principal().clone(), id_value)
                        .await;
                    let update = match auth {
                        Ok(Some(reference)) => registry
                            .reader
                            .load_repository_credentials(reference)
                            .await
                            .map(|current| Some(current) != entry.credentials),
                        Ok(None) => Ok(entry.credentials.is_some()),
                        Err(error) => Err(error),
                    };
                    match update {
                        Ok(false) => {}
                        Ok(true) => {
                            let current = registry.get_repository(&access, id_value).await;
                            let result = match current {
                                Ok(current) => registry
                                    .set_repository_credentials(
                                        &access,
                                        id_value,
                                        current.revision,
                                        entry.credentials,
                                        IdempotencyKey::parse(format!(
                                            "config-auth:{operation_key}:{}",
                                            entry.line
                                        ))
                                        .expect("bounded import key"),
                                    )
                                    .await
                                    .map(|_| ()),
                                Err(error) => Err(error),
                            };
                            if let Err(error) = result {
                                *uncertain = matches!(
                                    error.code(),
                                    M3ErrorCode::Internal | M3ErrorCode::StoreUnavailable
                                );
                                return (
                                    entry.line,
                                    if *uncertain {
                                        RepositoryBatchItem::Unknown(error.code())
                                    } else {
                                        RepositoryBatchItem::Failed(error.code())
                                    },
                                    None,
                                );
                            }
                            authentication_changed = true;
                        }
                        Err(error) => {
                            return (entry.line, RepositoryBatchItem::Failed(error.code()), None);
                        }
                    }
                }
                (
                    entry.line,
                    if added {
                        RepositoryBatchItem::Added
                    } else if authentication_changed {
                        RepositoryBatchItem::Updated
                    } else {
                        RepositoryBatchItem::Existing
                    },
                    Some((id, configuration)),
                )
            });
        }
        let mut results = Vec::new();
        let mut imported = BTreeMap::new();
        while let Some(result) = jobs.join_next().await {
            let (line, outcome, configuration) =
                result.map_err(|_| RepositoryListError("repository_list_import_incomplete"))?;
            if let Some((id, configuration)) = configuration {
                imported.insert(id, (line, configuration));
            }
            results.push((line, outcome));
        }
        results.sort_by_key(|(line, _)| *line);
        if imported.is_empty() {
            return Ok(results);
        }
        let current = self.snapshot(access).await?;
        let config = official.get_settings(access).await.map_err(config_error)?;
        let mut hidden = config.settings.packages.hidden_repository_ids.clone();
        let mut package_groups = config.settings.packages.hidden_repository_packages.clone();
        let ids: HashSet<_> = imported.keys().cloned().collect();
        hidden.retain(|id| !ids.contains(id));
        package_groups.retain(|group| !ids.contains(&group.repository_id));
        for (id, (_, entry)) in &imported {
            if !entry.enabled {
                hidden.push(id.clone());
            }
            if !entry.disabled_package_ids.is_empty() {
                package_groups.push(ConfigRepositoryPackages {
                    repository_id: id.clone(),
                    package_ids: entry.disabled_package_ids.clone(),
                });
            }
        }
        hidden.sort();
        package_groups.sort_by(|a, b| a.repository_id.cmp(&b.repository_id));
        // Validate the merged preferences before committing any metadata. The
        // settings use case remains responsible for publication and its CAS.
        let mut merged = config.settings.clone();
        merged.packages.hidden_repository_ids = hidden.clone();
        merged.packages.hidden_repository_packages = package_groups.clone();
        merged.validate().map_err(config_error)?;
        let mut ordered: Vec<_> = current.repositories.iter().collect();
        ordered.sort_by_key(|record| match imported.get(&record.repository_id) {
            Some((line, entry)) => (false, entry.priority, *line as u64),
            None => (true, record.configuration.priority, 0),
        });
        let max_imported = imported
            .values()
            .map(|(_, entry)| entry.priority)
            .max()
            .unwrap_or(0);
        let preserve_priorities = overwrite_existing
            && max_imported
                .checked_add(current.repositories.len() as u64)
                .is_some_and(|maximum| maximum <= i64::MAX as u64);
        let mut rank = if overwrite_existing {
            0
        } else {
            current
                .repositories
                .iter()
                .filter(|record| !ids.contains(&record.repository_id))
                .map(|record| record.configuration.priority)
                .max()
                .unwrap_or(0)
        };
        let mut previous = None;
        let mut changes = Vec::new();
        for record in ordered {
            let imported_configuration = imported.get(&record.repository_id);
            if !overwrite_existing && imported_configuration.is_none() {
                continue;
            }
            let group = match imported_configuration {
                Some((_, entry)) => (false, entry.priority),
                None => (true, record.configuration.priority),
            };
            if previous != Some(group) {
                rank = if preserve_priorities {
                    imported_configuration
                        .map(|(_, entry)| entry.priority)
                        .unwrap_or_else(|| (rank + 1).max(record.configuration.priority))
                } else {
                    rank + 1
                };
                previous = Some(group);
            }
            let name = imported_configuration
                .map(|(_, entry)| entry.display_name.clone())
                .unwrap_or_else(|| record.configuration.display_name.clone());
            let metadata_changed =
                name != record.configuration.display_name || rank != record.configuration.priority;
            let switches_changed = imported_configuration.is_some_and(|(_, entry)| {
                entry.enabled
                    == config
                        .settings
                        .packages
                        .hidden_repository_ids
                        .contains(&record.repository_id)
                    || entry.disabled_package_ids
                        != config
                            .settings
                            .packages
                            .hidden_repository_packages
                            .iter()
                            .find(|group| group.repository_id == record.repository_id)
                            .map(|group| group.package_ids.clone())
                            .unwrap_or_default()
            });
            if (metadata_changed || switches_changed)
                && let Some((line, _)) = imported_configuration
                && let Some((_, result)) =
                    results.iter_mut().find(|(candidate, _)| candidate == line)
                && *result == RepositoryBatchItem::Existing
            {
                *result = RepositoryBatchItem::Updated;
            }
            if metadata_changed {
                changes.push(RepositoryListMetadata {
                    repository_id: record.repository_id.clone(),
                    expected_display_name: record.configuration.display_name.clone(),
                    expected_priority: record.configuration.priority,
                    display_name: name,
                    priority: rank,
                });
            }
        }
        let repository_keys = changes
            .iter()
            .map(|entry| RepositoryId::parse(&entry.repository_id).expect("registered ID"))
            .map(crate::ResourceKey::Repository)
            .collect();
        let _guard = registry.locks.acquire(repository_keys).await;
        crate::log_execution_step(
            "repositories.importListFile",
            "restore_repository_configuration",
            None,
            self.store.apply_repository_list_metadata(
                access.principal().clone(),
                changes,
                super::super::m3_time_ms().map_err(|_| RepositoryListError("internal_error"))?,
            ),
            |_| None,
        )
        .await
        .map_err(|error| {
            RepositoryListError(match error.code() {
                M3ErrorCode::RevisionConflict => "repository_list_changed",
                M3ErrorCode::StoreUnavailable => "store_unavailable",
                _ => "repository_list_configuration_failed",
            })
        })?;
        if hidden != config.settings.packages.hidden_repository_ids
            || package_groups != config.settings.packages.hidden_repository_packages
        {
            official
                .update_settings(
                    access,
                    config.revision,
                    ConfigUpdate {
                        packages: Some(ConfigPackageSettingsUpdate {
                            show_prerelease: None,
                            hide_local_user_packages: None,
                            hidden_repository_ids: Some(hidden),
                            hidden_repository_packages: Some(package_groups),
                        }),
                        ..ConfigUpdate::default()
                    },
                )
                .await
                .map_err(config_error)?;
        }
        Ok(results)
    }
}

fn config_error(error: crate::OfficialGuiError) -> RepositoryListError {
    RepositoryListError(if error == crate::OfficialGuiError::PermissionDenied {
        "permission_denied"
    } else {
        "repository_list_configuration_failed"
    })
}
