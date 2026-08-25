use alcomd_application::{
    ExtensionApplyOutcome, ExtensionCrashDecision, ExtensionCursor, ExtensionDataDisposition,
    ExtensionDataValue, ExtensionDataWriteResult, ExtensionDesiredState,
    ExtensionFilesystemJournalEntry, ExtensionGrantRecord, ExtensionInstallPlanDraft,
    ExtensionInstanceLease, ExtensionPackageEvidence, ExtensionPage, ExtensionPlanRecord,
    ExtensionProjectSummary, ExtensionQuarantineState, ExtensionRecord, ExtensionRuntimeState,
    ExtensionSourceKind, ExtensionStartContext, ExtensionTrustDecision, ExtensionUiProtocol,
    ExtensionUninstallPlanDraft, IdempotencyKey, M6Error, M6ErrorCode, OperationId, PlanId,
    PrincipalId, Revision,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionEvidence {
    required: Vec<String>,
    optional: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct InterfaceEvidence {
    required: Vec<String>,
    optional: Vec<String>,
}

pub(super) fn list_extensions(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<&ExtensionCursor>,
    limit: u32,
) -> Result<ExtensionPage, M6Error> {
    let mut statement = connection
        .prepare(
            "SELECT e.extension_id, e.version, e.api_major, e.package_digest,
                    e.publisher_fingerprint, e.trust_decision, e.desired_state,
                    e.quarantine_state, COALESCE(i.runtime_state, 'stopped'),
                    e.grant_revision, e.lifecycle_generation, e.revision, e.ui_protocol
             FROM extensions e
             LEFT JOIN extension_instances i ON i.extension_id=e.extension_id
             WHERE e.principal_id=?1 AND e.extension_id>?2
             ORDER BY e.extension_id LIMIT ?3",
        )
        .map_err(store_error)?;
    let rows = statement
        .query_map(
            params![
                owner.as_str(),
                cursor.map_or("", ExtensionCursor::last_extension_id),
                i64::from(limit) + 1
            ],
            load_extension_row,
        )
        .map_err(store_error)?;
    let mut extensions = rows.collect::<Result<Vec<_>, _>>().map_err(store_error)?;
    let next_cursor = if extensions.len() > limit as usize {
        extensions.truncate(limit as usize);
        extensions
            .last()
            .map(|record| ExtensionCursor::new(record.extension_id.clone()))
            .transpose()?
    } else {
        None
    };
    Ok(ExtensionPage {
        extensions,
        next_cursor,
    })
}

pub(super) fn get_extension(
    connection: &Connection,
    owner: &PrincipalId,
    extension_id: &str,
) -> Result<ExtensionRecord, M6Error> {
    connection
        .query_row(
            "SELECT e.extension_id, e.version, e.api_major, e.package_digest,
                    e.publisher_fingerprint, e.trust_decision, e.desired_state,
                    e.quarantine_state, COALESCE(i.runtime_state, 'stopped'),
                    e.grant_revision, e.lifecycle_generation, e.revision, e.ui_protocol
             FROM extensions e
             LEFT JOIN extension_instances i ON i.extension_id=e.extension_id
             WHERE e.principal_id=?1 AND e.extension_id=?2",
            params![owner.as_str(), extension_id],
            load_extension_row,
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::NotInstalled))
}

pub(super) fn live_package_locator(
    connection: &Connection,
    owner: &PrincipalId,
    extension_id: &str,
) -> Result<String, M6Error> {
    connection
        .query_row(
            "SELECT live_package_locator FROM extensions
             WHERE extension_id=?1 AND principal_id=?2",
            params![extension_id, owner.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::NotInstalled))
}

pub(super) fn has_background_authority(
    connection: &Connection,
    owner: &PrincipalId,
    extension_id: &str,
) -> Result<bool, M6Error> {
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM extensions e
                JOIN extension_grants g ON g.extension_id=e.extension_id
                WHERE e.principal_id=?1 AND e.extension_id=?2
                  AND g.permission_name='background.run'
                  AND g.resource_kind='Extension'
                  AND g.resource_id=e.extension_id
                  AND g.state='granted'
            )",
            params![owner.as_str(), extension_id],
            |row| row.get(0),
        )
        .map_err(store_error)
}

pub(super) fn create_install_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: ExtensionInstallPlanDraft,
    now_ms: u64,
) -> Result<ExtensionPlanRecord, M6Error> {
    let permissions = serde_json::to_string(&PermissionEvidence {
        required: draft.evidence.required_permissions.clone(),
        optional: draft.evidence.optional_permissions.clone(),
    })
    .map_err(|_| internal())?;
    let interfaces = serde_json::to_string(&InterfaceEvidence {
        required: draft.evidence.required_interfaces.clone(),
        optional: draft.evidence.optional_interfaces.clone(),
    })
    .map_err(|_| internal())?;
    let transaction = transaction(connection)?;
    let current = extension_revision(&transaction, owner, &draft.evidence.extension_id)?;
    if current.is_some() {
        return Err(error(M6ErrorCode::AlreadyInstalled));
    }
    if current != draft.expected_revision {
        return Err(error(M6ErrorCode::RevisionConflict));
    }
    let plan_id = PlanId::new();
    transaction
        .execute(
            "INSERT INTO extension_plans (
                plan_id, owner_principal_id, action, state, extension_id, version,
                api_major, profile_version, expected_revision,
                source_kind, source_locator, source_identity, package_digest, manifest_digest,
                component_digest, publisher_fingerprint, trust_decision,
                requested_permissions_json, requested_interfaces_json, data_disposition,
                plan_fingerprint, created_at_ms, ui_protocol
             ) VALUES (?1, ?2, 'install', 'unapplied', ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                       ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 'not_applicable', ?18, ?19, ?20)",
            params![
                plan_id.to_string(),
                owner.as_str(),
                draft.evidence.extension_id,
                draft.evidence.version,
                i64::from(draft.evidence.api_major),
                i64::from(draft.evidence.profile_version),
                draft.expected_revision.map(sqlite_revision),
                source_kind(draft.evidence.source_kind),
                draft.evidence.source_locator,
                draft.evidence.source_identity,
                draft.evidence.package_digest.as_slice(),
                draft.evidence.manifest_digest.as_slice(),
                draft.evidence.component_digest.as_slice(),
                draft.evidence.publisher_fingerprint,
                trust(draft.trust_decision),
                permissions,
                interfaces,
                draft.plan_fingerprint.as_slice(),
                sqlite_integer(now_ms)?,
                draft.evidence.ui_protocol.map(ExtensionUiProtocol::as_str)
            ],
        )
        .map_err(store_error)?;
    let record = get_plan_on(&transaction, plan_id)?;
    transaction.commit().map_err(store_error)?;
    Ok(record)
}

pub(super) fn create_uninstall_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    draft: ExtensionUninstallPlanDraft,
    now_ms: u64,
) -> Result<ExtensionPlanRecord, M6Error> {
    let transaction = transaction(connection)?;
    let current = get_extension(&transaction, owner, &draft.extension.extension_id)?;
    if current.revision != draft.extension.revision {
        return Err(error(M6ErrorCode::RevisionConflict));
    }
    let plan_id = PlanId::new();
    transaction
        .execute(
            "INSERT INTO extension_plans (
                plan_id, owner_principal_id, action, state, extension_id, version,
                api_major, profile_version, expected_revision,
                source_kind, package_digest, manifest_digest, component_digest,
                publisher_fingerprint, trust_decision, requested_permissions_json,
                requested_interfaces_json, data_disposition, plan_fingerprint, created_at_ms,
                ui_protocol
             ) SELECT ?1, ?2, 'uninstall', 'unapplied', e.extension_id, e.version,
                      e.api_major, 1, e.revision,
                      'not_applicable', e.package_digest, e.manifest_digest, e.component_digest,
                      e.publisher_fingerprint, e.trust_decision,
                      installed.requested_permissions_json,
                      installed.requested_interfaces_json,
                      ?3, ?4, ?5, e.ui_protocol
               FROM extensions e JOIN extension_plans installed
                 ON installed.plan_id=(
                    SELECT p.plan_id FROM extension_plans p
                    WHERE p.extension_id=e.extension_id AND p.action='install'
                      AND p.state='applied' AND p.package_digest=e.package_digest
                    ORDER BY p.created_at_ms DESC, p.plan_id DESC LIMIT 1
                 )
               WHERE e.extension_id=?6 AND e.principal_id=?2",
            params![
                plan_id.to_string(),
                owner.as_str(),
                data_disposition(draft.data_disposition),
                draft.plan_fingerprint.as_slice(),
                sqlite_integer(now_ms)?,
                current.extension_id
            ],
        )
        .map_err(store_error)?;
    let record = get_plan_on(&transaction, plan_id)?;
    transaction.commit().map_err(store_error)?;
    Ok(record)
}

pub(super) fn get_plan(
    connection: &Connection,
    plan_id: PlanId,
) -> Result<ExtensionPlanRecord, M6Error> {
    get_plan_on(connection, plan_id)
}

pub(super) fn begin_apply(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<ExtensionPlanRecord, M6Error> {
    let transaction = transaction(connection)?;
    let plan = plan_for_operation(&transaction, operation_id)?;
    let now = sqlite_integer(now_ms)?;
    let changed = transaction
        .execute(
            "UPDATE operations SET state='running', revision=revision+1,
             started_at_ms=COALESCE(started_at_ms, ?1), updated_at_ms=?1
             WHERE operation_id=?2 AND kind IN ('extensions.install', 'extensions.uninstall')
             AND state IN ('queued', 'recovering')",
            params![now, operation_id.to_string()],
        )
        .map_err(store_error)?;
    if changed != 1 {
        let state: String = transaction
            .query_row(
                "SELECT state FROM operations WHERE operation_id=?1",
                [operation_id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(store_error)?
            .ok_or_else(|| error(M6ErrorCode::RecoveryRequired))?;
        if state != "running" {
            return Err(error(M6ErrorCode::RecoveryRequired));
        }
    } else {
        let revision = operation_revision(&transaction, operation_id)?;
        insert_operation_event(
            &transaction,
            &plan.owner,
            operation_id,
            revision,
            "operation.state_changed",
            now,
        )?;
    }
    if plan.action == "uninstall"
        && !journal_has_phase(&transaction, operation_id, "grants_revoked")?
        && extension_revision(&transaction, &plan.owner, &plan.evidence.extension_id)?.is_some()
    {
        let changed = transaction
            .execute(
                "UPDATE extensions SET desired_state='uninstalling',
                 grant_revision=grant_revision+1, lifecycle_generation=lifecycle_generation+1,
                 revision=revision+1, updated_at_ms=?1
                 WHERE extension_id=?2 AND principal_id=?3 AND revision=?4",
                params![
                    now,
                    plan.evidence.extension_id,
                    plan.owner.as_str(),
                    plan.expected_revision.map(sqlite_revision)
                ],
            )
            .map_err(store_error)?;
        if changed != 1 {
            return Err(error(M6ErrorCode::RecoveryRequired));
        }
        transaction
            .execute(
                "DELETE FROM extension_grants WHERE extension_id=?1",
                [&plan.evidence.extension_id],
            )
            .map_err(store_error)?;
        append_journal(&transaction, &plan, operation_id, "grants_revoked", now)?;
        transaction
            .execute(
                "UPDATE extension_instances SET lease_cancelled=1,
                 runtime_state='stopping', updated_at_ms=?1 WHERE extension_id=?2",
                params![now, plan.evidence.extension_id],
            )
            .map_err(store_error)?;
        append_journal(&transaction, &plan, operation_id, "lease_revoked", now)?;
    }
    transaction.commit().map_err(store_error)?;
    Ok(plan)
}

pub(super) fn append_filesystem_journal(
    connection: &mut Connection,
    entry: ExtensionFilesystemJournalEntry,
) -> Result<(), M6Error> {
    let evidence: serde_json::Value =
        serde_json::from_str(&entry.evidence_json).map_err(|_| error(M6ErrorCode::InvalidInput))?;
    if !evidence.is_object() || entry.evidence_json.len() > 1_048_576 {
        return Err(error(M6ErrorCode::InvalidInput));
    }
    connection
        .execute(
            "INSERT INTO extension_filesystem_journal
             (operation_id, step, plan_id, extension_id, action, phase, state,
              evidence_json, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.operation_id.to_string(),
                sqlite_integer(entry.step)?,
                entry.plan_id.to_string(),
                entry.extension_id,
                entry.action,
                entry.phase.as_str(),
                entry.state.as_str(),
                entry.evidence_json,
                sqlite_integer(entry.updated_at_ms)?
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

pub(super) fn next_filesystem_journal_step(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<u64, M6Error> {
    let step: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(step), 0)+1 FROM extension_filesystem_journal
             WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    nonnegative(step).map_err(|_| internal())
}

pub(super) fn filesystem_journal_has_phase(
    connection: &Connection,
    operation_id: OperationId,
    phase: alcomd_application::ExtensionJournalPhase,
) -> Result<bool, M6Error> {
    journal_has_phase(connection, operation_id, phase.as_str())
}

pub(super) fn recover_operations(
    connection: &mut Connection,
    now_ms: u64,
) -> Result<Vec<OperationId>, M6Error> {
    let transaction = transaction(connection)?;
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "UPDATE operations SET state='interrupted', revision=revision+1, updated_at_ms=?1
             WHERE kind IN ('extensions.install', 'extensions.uninstall')
             AND state IN ('running', 'cancelling')",
            [now],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE operations SET state='recovering', revision=revision+1, updated_at_ms=?1
             WHERE kind IN ('extensions.install', 'extensions.uninstall')
             AND state='interrupted'",
            [now],
        )
        .map_err(store_error)?;
    let mut statement = transaction
        .prepare(
            "SELECT operation_id FROM operations
             WHERE kind IN ('extensions.install', 'extensions.uninstall')
             AND state IN ('queued', 'recovering')
             ORDER BY created_at_ms, operation_id",
        )
        .map_err(store_error)?;
    let operations = statement
        .query_map([], |row| {
            OperationId::parse(&row.get::<_, String>(0)?).map_err(|_| invalid_query())
        })
        .map_err(store_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_error)?;
    drop(statement);
    transaction.commit().map_err(store_error)?;
    Ok(operations)
}

pub(super) fn accept_plan(
    connection: &mut Connection,
    owner: &PrincipalId,
    plan_id: PlanId,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<ExtensionApplyOutcome, M6Error> {
    let transaction = transaction(connection)?;
    let plan = get_plan_on(&transaction, plan_id)?;
    if plan.owner != *owner {
        return Err(error(M6ErrorCode::PermissionDenied));
    }
    let method = if plan.action == "install" {
        "extensions.applyInstall"
    } else {
        "extensions.applyUninstall"
    };
    let fingerprint = format!("{{\"planId\":\"{plan_id}\",\"version\":1}}");
    if let Some((saved, response)) = transaction
        .query_row(
            "SELECT request_fingerprint, response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(store_error)?
    {
        if saved != fingerprint {
            return Err(error(M6ErrorCode::IdempotencyConflict));
        }
        let mut outcome: ExtensionApplyOutcome =
            serde_json::from_str(&response).map_err(|_| internal())?;
        outcome.replayed = true;
        outcome.schedule = false;
        transaction.commit().map_err(store_error)?;
        return Ok(outcome);
    }
    if plan.state != "unapplied" {
        return Err(error(M6ErrorCode::PlanStale));
    }
    let current = extension_revision(&transaction, owner, &plan.evidence.extension_id)?;
    if current != plan.expected_revision {
        return Err(error(M6ErrorCode::PlanStale));
    }
    let operation_id = OperationId::new();
    let operation_kind = if plan.action == "install" {
        "extensions.install"
    } else {
        "extensions.uninstall"
    };
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "INSERT INTO operations (operation_id, kind, state, revision, owner_principal_id,
                 request_json, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, 'queued', 1, ?3, ?4, ?5, ?5)",
            params![
                operation_id.to_string(),
                operation_kind,
                owner.as_str(),
                fingerprint,
                now
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE extension_plans SET state='applied', apply_operation_id=?1
             WHERE plan_id=?2 AND state='unapplied'",
            params![operation_id.to_string(), plan_id.to_string()],
        )
        .map_err(store_error)?;
    insert_operation_event(
        &transaction,
        owner,
        operation_id,
        Revision::new(1).ok_or_else(internal)?,
        "operation.created",
        now,
    )?;
    transaction
        .execute(
            "INSERT INTO operation_journal (operation_id, step, kind, state, payload_json, updated_at_ms)
             VALUES (?1, 1, ?2, 'prepared', ?3, ?4)",
            params![operation_id.to_string(), operation_kind, fingerprint, now],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "INSERT INTO extension_filesystem_journal
             (operation_id, step, plan_id, extension_id, action, phase, state, evidence_json, updated_at_ms)
             VALUES (?1, 1, ?2, ?3, ?4, 'accepted', 'completed', '{}', ?5)",
            params![
                operation_id.to_string(), plan_id.to_string(), plan.evidence.extension_id,
                plan.action, now
            ],
        )
        .map_err(store_error)?;
    let saved = ExtensionApplyOutcome {
        operation_id,
        replayed: false,
        schedule: true,
    };
    let response = serde_json::to_string(&saved).map_err(|_| internal())?;
    transaction
        .execute(
            "INSERT INTO idempotency_records
             (principal_id, method, idempotency_key, request_fingerprint, state,
              operation_id, response_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, 'completed', ?5, ?6, ?7)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                operation_id.to_string(),
                response,
                now
            ],
        )
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)?;
    Ok(saved)
}

pub(super) fn finish_install(
    connection: &mut Connection,
    operation_id: OperationId,
    live_locator: &str,
    now_ms: u64,
) -> Result<(), M6Error> {
    let transaction = transaction(connection)?;
    let plan = plan_for_operation(&transaction, operation_id)?;
    if plan.action != "install" {
        return Err(internal());
    }
    let p = &plan.evidence;
    let now = sqlite_integer(now_ms)?;
    let existing = transaction
        .query_row(
            "SELECT version, api_major, package_digest, manifest_digest, component_digest,
                    publisher_fingerprint, trust_decision, principal_id, live_package_locator,
                    ui_protocol
             FROM extensions WHERE extension_id=?1",
            [&p.extension_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            },
        )
        .optional()
        .map_err(store_error)?;
    if let Some(existing) = existing {
        if existing.0 != p.version
            || existing.1 != i64::from(p.api_major)
            || existing.2 != p.package_digest
            || existing.3 != p.manifest_digest
            || existing.4 != p.component_digest
            || existing.5 != p.publisher_fingerprint
            || existing.6 != trust(plan.trust_decision)
            || existing.7 != plan.owner.as_str()
            || existing.8 != live_locator
            || existing.9.as_deref() != p.ui_protocol.map(ExtensionUiProtocol::as_str)
        {
            return Err(error(M6ErrorCode::RecoveryRequired));
        }
    } else {
        transaction
            .execute(
                "INSERT INTO extensions (
                extension_id, version, api_major, package_digest, manifest_digest,
                component_digest, publisher_fingerprint, trust_decision, principal_id,
                live_package_locator, desired_state, quarantine_state, grant_revision,
                lifecycle_generation, revision, ui_protocol, installed_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                       'installed_disabled', 'clear', 1, 1, 1, ?11, ?12, ?12)",
                params![
                    p.extension_id,
                    p.version,
                    i64::from(p.api_major),
                    p.package_digest.as_slice(),
                    p.manifest_digest.as_slice(),
                    p.component_digest.as_slice(),
                    p.publisher_fingerprint,
                    trust(plan.trust_decision),
                    plan.owner.as_str(),
                    live_locator,
                    p.ui_protocol.map(ExtensionUiProtocol::as_str),
                    now
                ],
            )
            .map_err(store_error)?;
    }
    transaction
        .execute(
            "INSERT INTO extension_data_namespaces
             (extension_id, publisher_fingerprint, revision, key_count, total_value_bytes, updated_at_ms)
             VALUES (?1, ?2, 1, 0, 0, ?3) ON CONFLICT DO NOTHING",
            params![p.extension_id, p.publisher_fingerprint, now],
        )
        .map_err(store_error)?;
    append_journal(&transaction, &plan, operation_id, "state_committed", now)?;
    transaction.commit().map_err(store_error)
}

pub(super) fn finish_uninstall(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M6Error> {
    let transaction = transaction(connection)?;
    let plan = plan_for_operation(&transaction, operation_id)?;
    if plan.action != "uninstall" {
        return Err(internal());
    }
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "DELETE FROM extension_grants WHERE extension_id=?1",
            [&plan.evidence.extension_id],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "DELETE FROM extension_instances WHERE extension_id=?1",
            [&plan.evidence.extension_id],
        )
        .map_err(store_error)?;
    if plan.data_disposition == Some(ExtensionDataDisposition::DeleteData) {
        transaction
            .execute(
                "DELETE FROM extension_data_namespaces
                 WHERE extension_id=?1 AND publisher_fingerprint=?2",
                params![
                    plan.evidence.extension_id,
                    plan.evidence.publisher_fingerprint
                ],
            )
            .map_err(store_error)?;
        append_journal(&transaction, &plan, operation_id, "data_deleted", now)?;
    }
    transaction
        .execute(
            "DELETE FROM extensions WHERE extension_id=?1",
            [&plan.evidence.extension_id],
        )
        .map_err(store_error)?;
    append_journal(&transaction, &plan, operation_id, "state_committed", now)?;
    transaction.commit().map_err(store_error)
}

pub(super) fn complete_operation(
    connection: &mut Connection,
    operation_id: OperationId,
    now_ms: u64,
) -> Result<(), M6Error> {
    let transaction = transaction(connection)?;
    if let Some((owner, revision)) = finish_operation(
        &transaction,
        operation_id,
        "succeeded",
        None,
        sqlite_integer(now_ms)?,
    )? {
        insert_operation_event(
            &transaction,
            &owner,
            operation_id,
            revision,
            "operation.completed",
            sqlite_integer(now_ms)?,
        )?;
    }
    transaction.commit().map_err(store_error)
}

pub(super) fn fail_operation(
    connection: &mut Connection,
    operation_id: OperationId,
    code: M6ErrorCode,
    now_ms: u64,
) -> Result<(), M6Error> {
    let transaction = transaction(connection)?;
    if let Some((owner, revision)) = finish_operation(
        &transaction,
        operation_id,
        "failed",
        Some(error_code(code)),
        sqlite_integer(now_ms)?,
    )? {
        insert_operation_event(
            &transaction,
            &owner,
            operation_id,
            revision,
            "operation.completed",
            sqlite_integer(now_ms)?,
        )?;
    }
    transaction.commit().map_err(store_error)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn set_grant(
    connection: &mut Connection,
    owner: &PrincipalId,
    extension_id: &str,
    permission: &str,
    resource_kind: &str,
    resource_id: &str,
    expected: Revision,
    key: &IdempotencyKey,
    grant: bool,
    now_ms: u64,
) -> Result<ExtensionGrantRecord, M6Error> {
    if !valid_scope(extension_id, permission, resource_kind, resource_id) {
        return Err(error(M6ErrorCode::ScopeDenied));
    }
    let transaction = transaction(connection)?;
    let method = if grant {
        "extensions.setGrant"
    } else {
        "extensions.revokeGrant"
    };
    let fingerprint = format!(
        "{{\"extensionId\":\"{extension_id}\",\"permission\":\"{permission}\",\"resourceId\":\"{resource_id}\",\"resourceKind\":\"{resource_kind}\",\"version\":1}}"
    );
    if let Some((saved, response)) = idempotent(&transaction, owner, method, key)? {
        if saved != fingerprint {
            return Err(error(M6ErrorCode::IdempotencyConflict));
        }
        let mut result: ExtensionGrantRecord =
            serde_json::from_str(&response).map_err(|_| internal())?;
        result.replayed = true;
        transaction.commit().map_err(store_error)?;
        return Ok(result);
    }
    let extension = get_extension(&transaction, owner, extension_id)?;
    let requested: bool = transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM extension_plans p, json_each(p.requested_permissions_json, '$.required') r
                WHERE p.extension_id=?1 AND p.action='install' AND p.state='applied'
                  AND r.value=?2
                UNION ALL
                SELECT 1 FROM extension_plans p, json_each(p.requested_permissions_json, '$.optional') o
                WHERE p.extension_id=?1 AND p.action='install' AND p.state='applied'
                  AND o.value=?2
             )",
            params![extension_id, permission],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    if !requested {
        return Err(error(M6ErrorCode::PermissionDenied));
    }
    if extension.grant_revision != expected {
        return Err(error(M6ErrorCode::RevisionConflict));
    }
    let next = expected.checked_next().ok_or_else(internal)?;
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "INSERT INTO extension_grants
             (extension_id, permission_name, resource_kind, resource_id, state, grant_revision, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(extension_id, permission_name, resource_kind, resource_id)
             DO UPDATE SET state=excluded.state, grant_revision=excluded.grant_revision,
                           updated_at_ms=excluded.updated_at_ms",
            params![extension_id, permission, resource_kind, resource_id,
                    if grant { "granted" } else { "revoked" }, sqlite_revision(next), now],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE extensions SET grant_revision=?1, revision=revision+1, updated_at_ms=?2
             WHERE extension_id=?3 AND principal_id=?4 AND grant_revision=?5",
            params![
                sqlite_revision(next),
                now,
                extension_id,
                owner.as_str(),
                sqlite_revision(expected)
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE extension_instances SET lease_cancelled=1, updated_at_ms=?1
             WHERE extension_id=?2",
            params![now, extension_id],
        )
        .map_err(store_error)?;
    let result = ExtensionGrantRecord {
        extension_id: extension_id.to_owned(),
        permission: permission.to_owned(),
        resource_kind: resource_kind.to_owned(),
        resource_id: resource_id.to_owned(),
        granted: grant,
        grant_revision: next,
        replayed: false,
    };
    save_idempotent(&transaction, owner, method, key, &fingerprint, &result, now)?;
    transaction.commit().map_err(store_error)?;
    Ok(result)
}

pub(super) fn set_desired(
    connection: &mut Connection,
    owner: &PrincipalId,
    extension_id: &str,
    expected: Revision,
    key: &IdempotencyKey,
    enable: bool,
    now_ms: u64,
) -> Result<ExtensionRecord, M6Error> {
    let transaction = transaction(connection)?;
    let method = if enable {
        "extensions.enable"
    } else {
        "extensions.disable"
    };
    let fingerprint = format!(
        "{{\"extensionId\":\"{extension_id}\",\"expectedRevision\":{},\"version\":1}}",
        expected.get()
    );
    if let Some((saved, response)) = idempotent(&transaction, owner, method, key)? {
        if saved != fingerprint {
            return Err(error(M6ErrorCode::IdempotencyConflict));
        }
        let result = serde_json::from_str(&response).map_err(|_| internal())?;
        transaction.commit().map_err(store_error)?;
        return Ok(result);
    }
    let current = get_extension(&transaction, owner, extension_id)?;
    if current.revision != expected {
        return Err(error(M6ErrorCode::RevisionConflict));
    }
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "UPDATE extensions SET desired_state=?1,
             quarantine_state=CASE WHEN ?1='enabled' THEN 'clear' ELSE quarantine_state END,
             lifecycle_generation=lifecycle_generation+1,
             revision=revision+1, updated_at_ms=?2 WHERE extension_id=?3 AND principal_id=?4",
            params![
                if enable {
                    "enabled"
                } else {
                    "installed_disabled"
                },
                now,
                extension_id,
                owner.as_str()
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE extension_instances SET lease_cancelled=1, runtime_state='stopping', updated_at_ms=?1
             WHERE extension_id=?2",
            params![now, extension_id],
        )
        .map_err(store_error)?;
    let result = get_extension(&transaction, owner, extension_id)?;
    save_idempotent(&transaction, owner, method, key, &fingerprint, &result, now)?;
    transaction.commit().map_err(store_error)?;
    Ok(result)
}

pub(super) fn data_get(
    connection: &Connection,
    lease: &ExtensionInstanceLease,
    key: &str,
    now_ms: u64,
) -> Result<Option<ExtensionDataValue>, M6Error> {
    if !valid_data_key(key) {
        return Err(error(M6ErrorCode::InvalidInput));
    }
    validate_lease(connection, lease, now_ms)?;
    ensure_data_owner(connection, lease)?;
    connection
        .query_row(
            "SELECT i.value, i.key_revision, n.revision
             FROM extension_data_items i JOIN extension_data_namespaces n
             ON n.extension_id=i.extension_id AND n.publisher_fingerprint=i.publisher_fingerprint
             WHERE i.extension_id=?1 AND i.publisher_fingerprint=?2 AND i.key=?3",
            params![lease.extension_id, lease.publisher_fingerprint, key],
            |row| {
                Ok(ExtensionDataValue {
                    value: row.get(0)?,
                    key_revision: row_revision(row, 1)?,
                    namespace_revision: row_revision(row, 2)?,
                })
            },
        )
        .optional()
        .map_err(store_error)
}

pub(super) fn data_set(
    connection: &mut Connection,
    lease: &ExtensionInstanceLease,
    key: &str,
    value: &[u8],
    expected: Option<Revision>,
    now_ms: u64,
) -> Result<ExtensionDataWriteResult, M6Error> {
    if !valid_data_key(key) || value.len() > 65_536 {
        return Err(error(M6ErrorCode::InvalidInput));
    }
    let transaction = transaction(connection)?;
    validate_lease(&transaction, lease, now_ms)?;
    ensure_data_owner(&transaction, lease)?;
    let existing = transaction
        .query_row(
            "SELECT key_revision, length(value) FROM extension_data_items
             WHERE extension_id=?1 AND publisher_fingerprint=?2 AND key=?3",
            params![lease.extension_id, lease.publisher_fingerprint, key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(store_error)?;
    match (existing, expected) {
        (None, None) => {}
        (Some((revision, _)), Some(expected)) if revision == sqlite_revision(expected) => {}
        _ => return Err(error(M6ErrorCode::RevisionConflict)),
    }
    let (old_bytes, next_key, key_delta) = match existing {
        Some((revision, bytes)) => (bytes, revision.checked_add(1).ok_or_else(internal)?, 0),
        None => (0, 1, 1),
    };
    let delta = i64::try_from(value.len()).map_err(|_| internal())? - old_bytes;
    let (count, total, namespace_revision) = transaction
        .query_row(
            "SELECT key_count, total_value_bytes, revision FROM extension_data_namespaces
             WHERE extension_id=?1 AND publisher_fingerprint=?2",
            params![lease.extension_id, lease.publisher_fingerprint],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(store_error)?;
    if count + key_delta > 1_024 || total + delta > 4_194_304 {
        return Err(error(M6ErrorCode::DataQuotaExceeded));
    }
    let next_namespace = namespace_revision.checked_add(1).ok_or_else(internal)?;
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "INSERT INTO extension_data_items
             (extension_id, publisher_fingerprint, key, value, key_revision)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(extension_id, publisher_fingerprint, key)
             DO UPDATE SET value=excluded.value, key_revision=excluded.key_revision",
            params![
                lease.extension_id,
                lease.publisher_fingerprint,
                key,
                value,
                next_key
            ],
        )
        .map_err(store_error)?;
    transaction
        .execute(
            "UPDATE extension_data_namespaces SET key_count=key_count+?1,
             total_value_bytes=total_value_bytes+?2, revision=?3, updated_at_ms=?4
             WHERE extension_id=?5 AND publisher_fingerprint=?6",
            params![
                key_delta,
                delta,
                next_namespace,
                now,
                lease.extension_id,
                lease.publisher_fingerprint
            ],
        )
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)?;
    Ok(ExtensionDataWriteResult {
        key_revision: Revision::new(u64::try_from(next_key).map_err(|_| internal())?)
            .ok_or_else(internal)?,
        namespace_revision: Revision::new(u64::try_from(next_namespace).map_err(|_| internal())?)
            .ok_or_else(internal)?,
    })
}

pub(super) fn data_delete(
    connection: &mut Connection,
    lease: &ExtensionInstanceLease,
    key: &str,
    expected: Revision,
    now_ms: u64,
) -> Result<ExtensionDataWriteResult, M6Error> {
    if !valid_data_key(key) {
        return Err(error(M6ErrorCode::InvalidInput));
    }
    let transaction = transaction(connection)?;
    validate_lease(&transaction, lease, now_ms)?;
    ensure_data_owner(&transaction, lease)?;
    let bytes: i64 = transaction
        .query_row(
            "SELECT length(value) FROM extension_data_items WHERE extension_id=?1
             AND publisher_fingerprint=?2 AND key=?3 AND key_revision=?4",
            params![
                lease.extension_id,
                lease.publisher_fingerprint,
                key,
                sqlite_revision(expected)
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::RevisionConflict))?;
    transaction
        .execute(
            "DELETE FROM extension_data_items WHERE extension_id=?1
             AND publisher_fingerprint=?2 AND key=?3",
            params![lease.extension_id, lease.publisher_fingerprint, key],
        )
        .map_err(store_error)?;
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "UPDATE extension_data_namespaces SET key_count=key_count-1,
             total_value_bytes=total_value_bytes-?1, revision=revision+1, updated_at_ms=?2
             WHERE extension_id=?3 AND publisher_fingerprint=?4",
            params![bytes, now, lease.extension_id, lease.publisher_fingerprint],
        )
        .map_err(store_error)?;
    let namespace_revision: i64 = transaction
        .query_row(
            "SELECT revision FROM extension_data_namespaces WHERE extension_id=?1
             AND publisher_fingerprint=?2",
            params![lease.extension_id, lease.publisher_fingerprint],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)?;
    Ok(ExtensionDataWriteResult {
        key_revision: expected.checked_next().ok_or_else(internal)?,
        namespace_revision: Revision::new(
            u64::try_from(namespace_revision).map_err(|_| internal())?,
        )
        .ok_or_else(internal)?,
    })
}

pub(super) fn prepare_instance(
    connection: &mut Connection,
    owner: &PrincipalId,
    extension_id: &str,
    daemon_epoch: &str,
    now_ms: u64,
) -> Result<ExtensionStartContext, M6Error> {
    let transaction = transaction(connection)?;
    let (publisher, grant_revision, lifecycle_generation, locator): (String, i64, i64, String) =
        transaction
            .query_row(
                "SELECT publisher_fingerprint, grant_revision, lifecycle_generation,
                        live_package_locator
                 FROM extensions WHERE extension_id=?1 AND principal_id=?2
                 AND desired_state='enabled' AND quarantine_state='clear'
                 AND EXISTS (
                    SELECT 1 FROM extension_data_namespaces n
                    WHERE n.extension_id=extensions.extension_id
                      AND n.publisher_fingerprint=extensions.publisher_fingerprint
                 )",
                params![extension_id, owner.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(store_error)?
            .ok_or_else(|| error(M6ErrorCode::Quarantined))?;
    let required_grants_present: bool = transaction
        .query_row(
            "SELECT NOT EXISTS(
                SELECT 1
                FROM extension_plans p, json_each(p.requested_permissions_json, '$.required') r
                WHERE p.extension_id=?1 AND p.action='install' AND p.state='applied'
                  AND p.package_digest=(SELECT package_digest FROM extensions WHERE extension_id=?1)
                  AND NOT EXISTS (
                    SELECT 1 FROM extension_grants g
                    WHERE g.extension_id=?1 AND g.permission_name=r.value AND g.state='granted'
                      AND ((r.value='background.run' AND g.resource_kind='Extension'
                            AND g.resource_id=?1)
                           OR (r.value='projects.read' AND g.resource_kind='Project'))
                  )
             )",
            [extension_id],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    if !required_grants_present {
        return Err(error(M6ErrorCode::PermissionDenied));
    }
    let instance_id = OperationId::new().to_string();
    let lease_id = OperationId::new().to_string();
    let principal_id =
        PrincipalId::parse(format!("extension-instance:{instance_id}")).map_err(|_| internal())?;
    let expires_at_ms = now_ms.checked_add(60_000).ok_or_else(internal)?;
    let now = sqlite_integer(now_ms)?;
    transaction
        .execute(
            "INSERT INTO extension_instances (
                extension_id, instance_id, principal_id, bound_grant_revision,
                lifecycle_generation, daemon_epoch, runtime_state, lease_expires_at_ms,
                lease_cancelled, started_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'starting', ?7, 0, NULL, ?8)
             ON CONFLICT(extension_id) DO UPDATE SET
                instance_id=excluded.instance_id, principal_id=excluded.principal_id,
                bound_grant_revision=excluded.bound_grant_revision,
                lifecycle_generation=excluded.lifecycle_generation,
                daemon_epoch=excluded.daemon_epoch, runtime_state='starting',
                lease_expires_at_ms=excluded.lease_expires_at_ms, lease_cancelled=0,
                started_at_ms=NULL, updated_at_ms=excluded.updated_at_ms",
            params![
                extension_id,
                instance_id,
                principal_id.as_str(),
                grant_revision,
                lifecycle_generation,
                daemon_epoch,
                sqlite_integer(expires_at_ms)?,
                now
            ],
        )
        .map_err(store_error)?;
    transaction.commit().map_err(store_error)?;
    let component_path = std::path::PathBuf::from(locator)
        .join("component")
        .join("extension.wasm")
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| error(M6ErrorCode::RecoveryRequired))?;
    Ok(ExtensionStartContext {
        lease: ExtensionInstanceLease {
            lease_id,
            extension_id: extension_id.to_owned(),
            instance_id,
            principal_id,
            publisher_fingerprint: publisher,
            grant_revision: Revision::new(nonnegative(grant_revision).map_err(|_| internal())?)
                .ok_or_else(internal)?,
            lifecycle_generation: Revision::new(
                nonnegative(lifecycle_generation).map_err(|_| internal())?,
            )
            .ok_or_else(internal)?,
            daemon_epoch: daemon_epoch.to_owned(),
            expires_at_ms,
        },
        component_path,
        activation_kind: alcomd_application::ExtensionActivationKind::Background,
    })
}

pub(super) fn mark_instance_running(
    connection: &mut Connection,
    lease: &ExtensionInstanceLease,
    now_ms: u64,
) -> Result<(), M6Error> {
    let now = sqlite_integer(now_ms)?;
    let changed = connection
        .execute(
            "UPDATE extension_instances SET runtime_state='running', started_at_ms=?1,
             updated_at_ms=?1 WHERE extension_id=?2 AND instance_id=?3
             AND principal_id=?4 AND bound_grant_revision=?5 AND lifecycle_generation=?6
             AND daemon_epoch=?7 AND lease_expires_at_ms=?8 AND lease_cancelled=0
             AND runtime_state='starting'",
            params![
                now,
                lease.extension_id,
                lease.instance_id,
                lease.principal_id.as_str(),
                sqlite_revision(lease.grant_revision),
                sqlite_revision(lease.lifecycle_generation),
                lease.daemon_epoch,
                sqlite_integer(lease.expires_at_ms)?
            ],
        )
        .map_err(store_error)?;
    if changed == 1 {
        Ok(())
    } else {
        Err(error(M6ErrorCode::InstanceStale))
    }
}

pub(super) fn mark_instance_stopped(
    connection: &mut Connection,
    extension_id: &str,
) -> Result<(), M6Error> {
    connection
        .execute(
            "DELETE FROM extension_instances WHERE extension_id=?1",
            [extension_id],
        )
        .map_err(store_error)?;
    Ok(())
}

pub(super) fn renew_instance(
    connection: &mut Connection,
    lease: &ExtensionInstanceLease,
    now_ms: u64,
) -> Result<ExtensionInstanceLease, M6Error> {
    let expires_at_ms = now_ms.checked_add(60_000).ok_or_else(internal)?;
    let changed = connection
        .execute(
            "UPDATE extension_instances SET lease_expires_at_ms=?1, updated_at_ms=?2
             WHERE extension_id=?3 AND instance_id=?4 AND principal_id=?5
             AND bound_grant_revision=?6 AND lifecycle_generation=?7 AND daemon_epoch=?8
             AND lease_expires_at_ms=?9 AND lease_cancelled=0 AND runtime_state='running'",
            params![
                sqlite_integer(expires_at_ms)?,
                sqlite_integer(now_ms)?,
                lease.extension_id,
                lease.instance_id,
                lease.principal_id.as_str(),
                sqlite_revision(lease.grant_revision),
                sqlite_revision(lease.lifecycle_generation),
                lease.daemon_epoch,
                sqlite_integer(lease.expires_at_ms)?
            ],
        )
        .map_err(store_error)?;
    if changed != 1 {
        return Err(error(M6ErrorCode::InstanceStale));
    }
    let mut renewed = lease.clone();
    renewed.expires_at_ms = expires_at_ms;
    Ok(renewed)
}

pub(super) fn record_instance_crash(
    connection: &mut Connection,
    lease: &ExtensionInstanceLease,
    reason: &str,
    now_ms: u64,
) -> Result<ExtensionCrashDecision, M6Error> {
    if reason.is_empty()
        || reason.len() > 64
        || !reason
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
    {
        return Err(error(M6ErrorCode::InvalidInput));
    }
    let transaction = transaction(connection)?;
    let current: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM extension_instances
             WHERE extension_id=?1 AND instance_id=?2 AND principal_id=?3
             AND bound_grant_revision=?4 AND lifecycle_generation=?5 AND daemon_epoch=?6)",
            params![
                lease.extension_id,
                lease.instance_id,
                lease.principal_id.as_str(),
                sqlite_revision(lease.grant_revision),
                sqlite_revision(lease.lifecycle_generation),
                lease.daemon_epoch
            ],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    if !current {
        return Err(error(M6ErrorCode::InstanceStale));
    }
    transaction
        .execute(
            "UPDATE extension_instances SET runtime_state='crashed', lease_cancelled=1,
             updated_at_ms=?1 WHERE extension_id=?2 AND instance_id=?3",
            params![
                sqlite_integer(now_ms)?,
                lease.extension_id,
                lease.instance_id
            ],
        )
        .map_err(store_error)?;
    let mut evidence = {
        let mut statement = transaction
            .prepare(
                "SELECT occurred_at_ms, reason_code FROM extension_crashes
                 WHERE extension_id=?1 ORDER BY sequence",
            )
            .map_err(store_error)?;
        statement
            .query_map([&lease.extension_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(store_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(store_error)?
    };
    evidence.push((sqlite_integer(now_ms)?, reason.to_owned()));
    if evidence.len() > 16 {
        evidence.remove(0);
    }
    transaction
        .execute(
            "DELETE FROM extension_crashes WHERE extension_id=?1",
            [&lease.extension_id],
        )
        .map_err(store_error)?;
    for (index, (occurred_at_ms, reason_code)) in evidence.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO extension_crashes
                 (extension_id, sequence, occurred_at_ms, reason_code)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    lease.extension_id,
                    i64::try_from(index + 1).map_err(|_| internal())?,
                    occurred_at_ms,
                    reason_code
                ],
            )
            .map_err(store_error)?;
    }
    let window_start = now_ms.saturating_sub(300_000);
    let crash_count = evidence
        .iter()
        .filter(|(occurred_at_ms, _)| {
            u64::try_from(*occurred_at_ms).is_ok_and(|value| value >= window_start)
        })
        .count();
    let (desired_state, quarantine_state): (String, String) = transaction
        .query_row(
            "SELECT desired_state, quarantine_state FROM extensions WHERE extension_id=?1",
            [&lease.extension_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(store_error)?;
    let should_quarantine = crash_count >= 3 && quarantine_state == "clear";
    if should_quarantine {
        transaction
            .execute(
                "UPDATE extensions SET quarantine_state='quarantined',
                 lifecycle_generation=lifecycle_generation+1, revision=revision+1,
                 updated_at_ms=?1 WHERE extension_id=?2 AND quarantine_state='clear'",
                params![sqlite_integer(now_ms)?, lease.extension_id],
            )
            .map_err(store_error)?;
        transaction
            .execute(
                "DELETE FROM extension_instances WHERE extension_id=?1",
                [&lease.extension_id],
            )
            .map_err(store_error)?;
    }
    transaction.commit().map_err(store_error)?;
    let restart_delay_ms =
        if desired_state != "enabled" || quarantine_state != "clear" || should_quarantine {
            None
        } else if crash_count == 1 {
            Some(1_000)
        } else {
            Some(5_000)
        };
    Ok(ExtensionCrashDecision {
        extension_id: lease.extension_id.clone(),
        restart_delay_ms,
        quarantined: should_quarantine || quarantine_state == "quarantined",
    })
}

pub(super) fn recover_instances(
    connection: &mut Connection,
    daemon_epoch: &str,
    now_ms: u64,
) -> Result<Vec<String>, M6Error> {
    let transaction = transaction(connection)?;
    transaction
        .execute(
            "UPDATE extension_instances SET runtime_state='crashed', lease_cancelled=1,
             updated_at_ms=?1 WHERE daemon_epoch<>?2
             AND runtime_state IN ('starting', 'running', 'stopping')",
            params![sqlite_integer(now_ms)?, daemon_epoch],
        )
        .map_err(store_error)?;
    let mut statement = transaction
        .prepare(
            "SELECT e.extension_id FROM extensions e
             WHERE e.principal_id=?1 AND e.desired_state='enabled'
             AND e.quarantine_state='clear'
             AND EXISTS(SELECT 1 FROM extension_grants g WHERE g.extension_id=e.extension_id
                 AND g.permission_name='background.run' AND g.resource_kind='Extension'
                 AND g.resource_id=e.extension_id AND g.state='granted')
             ORDER BY e.extension_id",
        )
        .map_err(store_error)?;
    let extensions = statement
        .query_map([PrincipalId::LOCAL_OWNER], |row| row.get::<_, String>(0))
        .map_err(store_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(store_error)?;
    drop(statement);
    transaction.commit().map_err(store_error)?;
    Ok(extensions)
}

pub(super) fn project_summary(
    connection: &Connection,
    lease: &ExtensionInstanceLease,
    project_id: &str,
    now_ms: u64,
) -> Result<ExtensionProjectSummary, M6Error> {
    validate_lease(connection, lease, now_ms)?;
    let granted: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM extension_grants WHERE extension_id=?1
             AND permission_name='projects.read' AND resource_kind='Project'
             AND resource_id=?2 AND state='granted')",
            params![lease.extension_id, project_id],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    if !granted {
        return Err(error(M6ErrorCode::ScopeDenied));
    }
    connection
        .query_row(
            "SELECT root_path, project_type, unity_version, revision FROM projects
             WHERE project_id=?1 AND owner_principal_id=?2",
            params![project_id, PrincipalId::LOCAL_OWNER],
            |row| {
                let root: String = row.get(0)?;
                let project_type: String = row.get(1)?;
                Ok(ExtensionProjectSummary {
                    project_id: project_id.to_owned(),
                    display_name: std::path::Path::new(&root)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("Project")
                        .chars()
                        .take(120)
                        .collect(),
                    kind: if project_type.starts_with("upm-") {
                        "upm"
                    } else if matches!(project_type.as_str(), "avatars" | "worlds" | "vpm-starter")
                    {
                        "vpm"
                    } else {
                        "unknown"
                    }
                    .to_owned(),
                    unity_version: Some(row.get(2)?),
                    revision: row_revision(row, 3)?,
                })
            },
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::ProjectNotFound))
}

fn get_plan_on(connection: &Connection, plan_id: PlanId) -> Result<ExtensionPlanRecord, M6Error> {
    connection
        .query_row(
            "SELECT plan_id, owner_principal_id, action, state, extension_id, version,
                    api_major, profile_version, expected_revision, source_kind,
                    COALESCE(source_locator, ''),
                    COALESCE(source_identity, X''), package_digest, manifest_digest,
                    component_digest, publisher_fingerprint, trust_decision,
                    requested_permissions_json, requested_interfaces_json, data_disposition,
                    plan_fingerprint, apply_operation_id, created_at_ms, ui_protocol
             FROM extension_plans WHERE plan_id=?1",
            [plan_id.to_string()],
            load_plan_row,
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::PlanStale))
}

fn load_plan_row(row: &Row<'_>) -> rusqlite::Result<ExtensionPlanRecord> {
    let permissions: PermissionEvidence = json_row(row, 17)?;
    let interfaces: InterfaceEvidence = json_row(row, 18)?;
    Ok(ExtensionPlanRecord {
        plan_id: PlanId::parse(&row.get::<_, String>(0)?).map_err(|_| invalid_query())?,
        owner: PrincipalId::parse(row.get::<_, String>(1)?).map_err(|_| invalid_query())?,
        action: row.get(2)?,
        state: row.get(3)?,
        evidence: ExtensionPackageEvidence {
            extension_id: row.get(4)?,
            version: row.get(5)?,
            api_major: u32::try_from(row.get::<_, i64>(6)?).map_err(|_| invalid_query())?,
            profile_version: u32::try_from(row.get::<_, i64>(7)?).map_err(|_| invalid_query())?,
            source_kind: parse_source_kind(&row.get::<_, String>(9)?)?,
            source_locator: row.get(10)?,
            source_identity: row.get(11)?,
            package_digest: digest(row.get(12)?)?,
            manifest_digest: digest(row.get(13)?)?,
            component_digest: digest(row.get(14)?)?,
            publisher_fingerprint: row.get(15)?,
            required_permissions: permissions.required,
            optional_permissions: permissions.optional,
            required_interfaces: interfaces.required,
            optional_interfaces: interfaces.optional,
            ui_protocol: parse_ui_protocol(row.get(23)?)?,
        },
        trust_decision: parse_trust(&row.get::<_, String>(16)?)?,
        expected_revision: optional_revision(row.get(8)?)?,
        data_disposition: parse_disposition(&row.get::<_, String>(19)?)?,
        plan_fingerprint: digest(row.get(20)?)?,
        apply_operation_id: row
            .get::<_, Option<String>>(21)?
            .map(|value| OperationId::parse(&value).map_err(|_| invalid_query()))
            .transpose()?,
        created_at_ms: nonnegative(row.get(22)?)?,
    })
}

fn load_extension_row(row: &Row<'_>) -> rusqlite::Result<ExtensionRecord> {
    Ok(ExtensionRecord {
        extension_id: row.get(0)?,
        version: row.get(1)?,
        api_major: u32::try_from(row.get::<_, i64>(2)?).map_err(|_| invalid_query())?,
        package_digest: digest(row.get(3)?)?,
        publisher_fingerprint: row.get(4)?,
        trust_decision: parse_trust(&row.get::<_, String>(5)?)?,
        desired_state: parse_desired(&row.get::<_, String>(6)?)?,
        quarantine_state: parse_quarantine(&row.get::<_, String>(7)?)?,
        runtime_state: parse_runtime(&row.get::<_, String>(8)?)?,
        grant_revision: row_revision(row, 9)?,
        lifecycle_generation: row_revision(row, 10)?,
        revision: row_revision(row, 11)?,
        ui_protocol: parse_ui_protocol(row.get(12)?)?,
    })
}

fn extension_revision(
    connection: &Connection,
    owner: &PrincipalId,
    extension_id: &str,
) -> Result<Option<Revision>, M6Error> {
    connection
        .query_row(
            "SELECT revision FROM extensions WHERE principal_id=?1 AND extension_id=?2",
            params![owner.as_str(), extension_id],
            |row| row_revision(row, 0),
        )
        .optional()
        .map_err(store_error)
}

fn plan_for_operation(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<ExtensionPlanRecord, M6Error> {
    let plan_id = connection
        .query_row(
            "SELECT plan_id FROM extension_plans WHERE apply_operation_id=?1",
            [operation_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::RecoveryRequired))?;
    get_plan_on(connection, PlanId::parse(&plan_id).map_err(|_| internal())?)
}

fn append_journal(
    transaction: &Transaction<'_>,
    plan: &ExtensionPlanRecord,
    operation_id: OperationId,
    phase: &str,
    now: i64,
) -> Result<(), M6Error> {
    transaction
        .execute(
            "INSERT INTO extension_filesystem_journal
             (operation_id, step, plan_id, extension_id, action, phase, state, evidence_json, updated_at_ms)
             SELECT ?1, COALESCE(MAX(step), 0)+1, ?2, ?3, ?4, ?5, 'completed', '{}', ?6
             FROM extension_filesystem_journal WHERE operation_id=?1",
            params![operation_id.to_string(), plan.plan_id.to_string(), plan.evidence.extension_id,
                    plan.action, phase, now],
        )
        .map_err(store_error)?;
    Ok(())
}

fn journal_has_phase(
    connection: &Connection,
    operation_id: OperationId,
    phase: &str,
) -> Result<bool, M6Error> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM extension_filesystem_journal
             WHERE operation_id=?1 AND phase=?2 AND state='completed')",
            params![operation_id.to_string(), phase],
            |row| row.get::<_, bool>(0),
        )
        .map_err(store_error)
}

fn finish_operation(
    transaction: &Transaction<'_>,
    operation_id: OperationId,
    state: &str,
    operation_error: Option<&str>,
    now: i64,
) -> Result<Option<(PrincipalId, Revision)>, M6Error> {
    let (current_state, owner): (String, String) = transaction
        .query_row(
            "SELECT state, owner_principal_id FROM operations WHERE operation_id=?1
             AND kind IN ('extensions.install', 'extensions.uninstall')",
            [operation_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(store_error)?
        .ok_or_else(|| error(M6ErrorCode::RecoveryRequired))?;
    if matches!(current_state.as_str(), "succeeded" | "failed" | "cancelled") {
        return if current_state == state {
            Ok(None)
        } else {
            Err(error(M6ErrorCode::RecoveryRequired))
        };
    }
    let changed = transaction
        .execute(
            "UPDATE operations SET state=?1, revision=revision+1, error_code=?2,
             result_json=CASE WHEN ?1='succeeded' THEN '{}' ELSE NULL END,
             completed_at_ms=?3, updated_at_ms=?3 WHERE operation_id=?4",
            params![state, operation_error, now, operation_id.to_string()],
        )
        .map_err(store_error)?;
    if changed != 1 {
        return Err(error(M6ErrorCode::RecoveryRequired));
    }
    Ok(Some((
        PrincipalId::parse(owner).map_err(|_| internal())?,
        operation_revision(transaction, operation_id)?,
    )))
}

fn operation_revision(
    connection: &Connection,
    operation_id: OperationId,
) -> Result<Revision, M6Error> {
    let revision: i64 = connection
        .query_row(
            "SELECT revision FROM operations WHERE operation_id=?1",
            [operation_id.to_string()],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    Revision::new(nonnegative(revision).map_err(|_| internal())?).ok_or_else(internal)
}

fn insert_operation_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    operation_id: OperationId,
    revision: Revision,
    kind: &str,
    now: i64,
) -> Result<(), M6Error> {
    transaction
        .execute(
            "INSERT INTO events
             (event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
              principal_id, occurred_at_ms, payload_json)
             VALUES (?1, ?2, 'operation', ?3, ?4, ?5, ?6, '{}')",
            params![
                OperationId::new().to_string(),
                kind,
                operation_id.to_string(),
                sqlite_revision(revision),
                owner.as_str(),
                now
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

fn validate_lease(
    connection: &Connection,
    lease: &ExtensionInstanceLease,
    now_ms: u64,
) -> Result<(), M6Error> {
    let valid: i64 = connection
        .query_row(
            "SELECT count(*) FROM extension_instances i JOIN extensions e USING(extension_id)
             WHERE i.extension_id=?1 AND i.instance_id=?2 AND i.principal_id=?3
             AND e.publisher_fingerprint=?4 AND i.bound_grant_revision=?5
             AND i.lifecycle_generation=?6 AND i.daemon_epoch=?7
             AND i.lease_expires_at_ms=?8 AND i.lease_expires_at_ms>?9
             AND i.lease_cancelled=0 AND i.runtime_state='running'",
            params![
                lease.extension_id,
                lease.instance_id,
                lease.principal_id.as_str(),
                lease.publisher_fingerprint,
                sqlite_revision(lease.grant_revision),
                sqlite_revision(lease.lifecycle_generation),
                lease.daemon_epoch,
                sqlite_integer(lease.expires_at_ms)?,
                sqlite_integer(now_ms)?
            ],
            |row| row.get(0),
        )
        .map_err(store_error)?;
    if valid == 1 {
        Ok(())
    } else {
        Err(error(M6ErrorCode::InstanceStale))
    }
}

fn ensure_data_owner(
    connection: &Connection,
    lease: &ExtensionInstanceLease,
) -> Result<(), M6Error> {
    let fingerprints = connection
        .query_row(
            "SELECT count(*), SUM(CASE WHEN publisher_fingerprint=?2 THEN 1 ELSE 0 END)
             FROM extension_data_namespaces WHERE extension_id=?1",
            params![lease.extension_id, lease.publisher_fingerprint],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(store_error)?;
    if fingerprints.0 > 0 && fingerprints.1 == 0 {
        Err(error(M6ErrorCode::DataOwnerMismatch))
    } else {
        Ok(())
    }
}

fn idempotent(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
) -> Result<Option<(String, String)>, M6Error> {
    transaction
        .query_row(
            "SELECT request_fingerprint, response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(store_error)
}

fn save_idempotent<T: Serialize>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
    response: &T,
    now: i64,
) -> Result<(), M6Error> {
    transaction
        .execute(
            "INSERT INTO idempotency_records
             (principal_id, method, idempotency_key, request_fingerprint, state,
              response_json, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, 'completed', ?5, ?6)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                serde_json::to_string(response).map_err(|_| internal())?,
                now
            ],
        )
        .map_err(store_error)?;
    Ok(())
}

fn valid_scope(extension_id: &str, permission: &str, kind: &str, id: &str) -> bool {
    (permission == "background.run" && kind == "Extension" && id == extension_id)
        || (permission == "projects.read" && kind == "Project" && uuid::Uuid::parse_str(id).is_ok())
}

fn valid_data_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key.is_ascii()
        && key == key.to_ascii_lowercase()
        && !key.contains("//")
        && !key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        && key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-/".contains(&byte)
        })
}

fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M6Error> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(store_error)
}

fn json_row<T: for<'de> Deserialize<'de>>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    serde_json::from_str(&row.get::<_, String>(index)?).map_err(|_| invalid_query())
}

fn digest(value: Vec<u8>) -> rusqlite::Result<[u8; 32]> {
    value.try_into().map_err(|_| invalid_query())
}

fn row_revision(row: &Row<'_>, index: usize) -> rusqlite::Result<Revision> {
    Revision::new(nonnegative(row.get(index)?)?).ok_or_else(invalid_query)
}

fn optional_revision(value: Option<i64>) -> rusqlite::Result<Option<Revision>> {
    value
        .map(|value| Revision::new(nonnegative(value)?).ok_or_else(invalid_query))
        .transpose()
}

fn nonnegative(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| invalid_query())
}

fn sqlite_integer(value: u64) -> Result<i64, M6Error> {
    i64::try_from(value).map_err(|_| error(M6ErrorCode::InvalidInput))
}

fn sqlite_revision(value: Revision) -> i64 {
    i64::try_from(value.get()).expect("revision is bounded by i64")
}

fn source_kind(value: ExtensionSourceKind) -> &'static str {
    match value {
        ExtensionSourceKind::NotApplicable => "not_applicable",
        ExtensionSourceKind::LocalOwnerSelected => "local_owner_selected",
        ExtensionSourceKind::FirstPartyPackaged => "first_party_packaged",
    }
}

fn parse_source_kind(value: &str) -> rusqlite::Result<ExtensionSourceKind> {
    match value {
        "not_applicable" => Ok(ExtensionSourceKind::NotApplicable),
        "local_owner_selected" => Ok(ExtensionSourceKind::LocalOwnerSelected),
        "first_party_packaged" => Ok(ExtensionSourceKind::FirstPartyPackaged),
        _ => Err(invalid_query()),
    }
}

fn trust(value: ExtensionTrustDecision) -> &'static str {
    match value {
        ExtensionTrustDecision::Official => "official",
        ExtensionTrustDecision::UserApprovedForExtension => "user_approved_for_extension",
    }
}

fn parse_trust(value: &str) -> rusqlite::Result<ExtensionTrustDecision> {
    match value {
        "official" => Ok(ExtensionTrustDecision::Official),
        "user_approved_for_extension" => Ok(ExtensionTrustDecision::UserApprovedForExtension),
        _ => Err(invalid_query()),
    }
}

fn parse_ui_protocol(value: Option<String>) -> rusqlite::Result<Option<ExtensionUiProtocol>> {
    match value.as_deref() {
        None => Ok(None),
        Some("portable-v1") => Ok(Some(ExtensionUiProtocol::PortableV1)),
        Some(_) => Err(invalid_query()),
    }
}

fn data_disposition(value: ExtensionDataDisposition) -> &'static str {
    match value {
        ExtensionDataDisposition::RetainData => "retain_data",
        ExtensionDataDisposition::DeleteData => "delete_data",
    }
}

fn parse_disposition(value: &str) -> rusqlite::Result<Option<ExtensionDataDisposition>> {
    match value {
        "not_applicable" => Ok(None),
        "retain_data" => Ok(Some(ExtensionDataDisposition::RetainData)),
        "delete_data" => Ok(Some(ExtensionDataDisposition::DeleteData)),
        _ => Err(invalid_query()),
    }
}

fn parse_desired(value: &str) -> rusqlite::Result<ExtensionDesiredState> {
    match value {
        "installed_disabled" => Ok(ExtensionDesiredState::InstalledDisabled),
        "enabled" => Ok(ExtensionDesiredState::Enabled),
        "uninstalling" => Ok(ExtensionDesiredState::Uninstalling),
        _ => Err(invalid_query()),
    }
}

fn parse_quarantine(value: &str) -> rusqlite::Result<ExtensionQuarantineState> {
    match value {
        "clear" => Ok(ExtensionQuarantineState::Clear),
        "quarantined" => Ok(ExtensionQuarantineState::Quarantined),
        _ => Err(invalid_query()),
    }
}

fn parse_runtime(value: &str) -> rusqlite::Result<ExtensionRuntimeState> {
    match value {
        "stopped" => Ok(ExtensionRuntimeState::Stopped),
        "starting" => Ok(ExtensionRuntimeState::Starting),
        "running" => Ok(ExtensionRuntimeState::Running),
        "stopping" => Ok(ExtensionRuntimeState::Stopping),
        "crashed" => Ok(ExtensionRuntimeState::Crashed),
        _ => Err(invalid_query()),
    }
}

fn error(code: M6ErrorCode) -> M6Error {
    M6Error::new(code)
}

fn internal() -> M6Error {
    error(M6ErrorCode::Internal)
}

fn store_error(_: rusqlite::Error) -> M6Error {
    error(M6ErrorCode::StoreUnavailable)
}

pub(super) fn unavailable() -> M6Error {
    error(M6ErrorCode::StoreUnavailable)
}

fn invalid_query() -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

fn error_code(code: M6ErrorCode) -> &'static str {
    match code {
        M6ErrorCode::InvalidInput => "invalid_request",
        M6ErrorCode::PermissionDenied => "permission_denied",
        M6ErrorCode::ManifestInvalid => "extension_manifest_invalid",
        M6ErrorCode::PackageInvalid => "extension_package_invalid",
        M6ErrorCode::PackageUntrusted => "extension_package_untrusted",
        M6ErrorCode::PublisherConfirmationRequired => "extension_publisher_confirmation_required",
        M6ErrorCode::SignatureInvalid => "extension_signature_invalid",
        M6ErrorCode::AlreadyInstalled => "extension_already_installed",
        M6ErrorCode::NotInstalled => "extension_not_installed",
        M6ErrorCode::ProjectNotFound => "project_not_found",
        M6ErrorCode::ScopeDenied => "extension_scope_denied",
        M6ErrorCode::ApiUnsupported => "extension_api_unsupported",
        M6ErrorCode::InstanceStale => "extension_instance_stale",
        M6ErrorCode::ResourceLimit => "extension_resource_limit",
        M6ErrorCode::Crashed => "extension_crashed",
        M6ErrorCode::Quarantined => "extension_quarantined",
        M6ErrorCode::PlanStale => "extension_plan_stale",
        M6ErrorCode::DataQuotaExceeded => "extension_data_quota_exceeded",
        M6ErrorCode::DataOwnerMismatch => "extension_data_owner_mismatch",
        M6ErrorCode::RecoveryRequired => "extension_recovery_required",
        M6ErrorCode::RevisionConflict => "revision_conflict",
        M6ErrorCode::IdempotencyConflict => "idempotency_conflict",
        M6ErrorCode::StoreUnavailable => "store_unavailable",
        M6ErrorCode::Internal => "internal_error",
    }
}
