use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{Failure, failure, invalid, ipc_fields, read, snake, success, write};

pub(super) fn recognized(name: &str) -> bool {
    matches!(
        name,
        "alcomd_register_project"
            | "alcomd_list_project_templates"
            | "alcomd_create_project"
            | "alcomd_plan_project_copy"
            | "alcomd_apply_project_copy"
            | "alcomd_list_backups"
            | "alcomd_get_backup"
            | "alcomd_create_backup"
            | "alcomd_plan_backup_restore"
            | "alcomd_apply_backup_restore"
    )
}

fn params<T: DeserializeOwned>(value: Value) -> Result<T, ErrorData> {
    serde_json::from_value(value).map_err(|_| invalid())
}

fn reply<T: Serialize>(value: Result<T, ClientError>, writing: bool) -> Result<Value, Failure> {
    let value = value.map_err(if writing { write } else { read })?;
    serde_json::to_value(value)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}

pub(super) fn operation_kind(kind: &str) -> bool {
    matches!(
        kind,
        "projects.create" | "projects.copy" | "backups.create" | "backups.restore"
    )
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreatedProject {
    project_id: String,
    revision: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopiedProject {
    target_project_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredBackup {
    backup_id: String,
    source_project_id: String,
    archive_sha256: [u8; 32],
    archive_bytes: u64,
    format_version: u32,
    created_at_ms: u64,
    compression_mode: rpc::BackupCompression,
    exclude_vpm_packages: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoredProject {
    backup_id: String,
    project_id: String,
    target_name: String,
    packages_require_resolve: bool,
    excluded_packages: Vec<rpc::BackupRestoreExcludedPackage>,
}

fn project_receipt<T: Serialize + DeserializeOwned>(result: Value) -> Result<Value, Failure> {
    let safe: T = serde_json::from_value(result).map_err(|_| Failure::InvalidCore)?;
    serde_json::to_value(safe).map_err(|_| Failure::InvalidCore)
}

pub(super) fn operation_result(kind: &str, result: Value) -> Result<Value, Failure> {
    match kind {
        "projects.create" => project_receipt::<CreatedProject>(result),
        "projects.copy" => project_receipt::<CopiedProject>(result),
        "backups.restore" => project_receipt::<RestoredProject>(result),
        "backups.create" => {
            let backup: StoredBackup =
                serde_json::from_value(result).map_err(|_| Failure::InvalidCore)?;
            serde_json::to_value(rpc::BackupRecord {
                backup_id: backup.backup_id,
                source_project_id: backup.source_project_id,
                archive_sha256: backup
                    .archive_sha256
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                archive_bytes: backup.archive_bytes,
                format_version: backup.format_version,
                created_at_ms: backup.created_at_ms,
                compression_mode: backup.compression_mode,
                exclude_vpm_packages: backup.exclude_vpm_packages,
            })
            .map_err(|_| Failure::InvalidCore)
        }
        _ => Err(Failure::InvalidCore),
    }
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    request: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    if request.input_responses.is_some() || request.request_state.is_some() {
        return Err(invalid());
    }
    let value = ipc_fields(Value::Object(request.arguments.unwrap_or_default()))?;
    let (result, summary, plan) = match request.name.as_ref() {
        "alcomd_register_project" => (
            reply(client.mcp_project_register(params(value)?).await, true),
            "Project registered.",
            false,
        ),
        "alcomd_list_project_templates" => (
            reply(client.project_templates_list(params(value)?).await, false),
            "Project templates available for creation.",
            false,
        ),
        "alcomd_create_project" => (
            reply(client.mcp_project_create(params(value)?).await, true),
            "Project creation accepted. Check the operation.",
            false,
        ),
        "alcomd_plan_project_copy" => (
            reply(client.mcp_project_plan_copy(params(value)?).await, true),
            "Review the project copy plan before confirming.",
            true,
        ),
        "alcomd_apply_project_copy" => {
            let input: rpc::McpProjectsApplyCopyParams = params(value)?;
            if !input.confirm {
                return Err(invalid());
            }
            (
                reply(client.mcp_project_apply_copy(input).await, true),
                "Project copy accepted. Check the operation.",
                false,
            )
        }
        "alcomd_list_backups" => (
            reply(client.backups_list(params(value)?).await, false),
            "Managed project backups.",
            false,
        ),
        "alcomd_get_backup" => {
            let input: rpc::BackupGetParams = params(value)?;
            (
                reply(client.backup_get(input.backup_id).await, false),
                "Managed backup details.",
                false,
            )
        }
        "alcomd_create_backup" => (
            reply(client.backup_create(params(value)?).await, true),
            "Backup creation accepted. Check the operation.",
            false,
        ),
        "alcomd_plan_backup_restore" => (
            reply(client.mcp_backup_plan_restore(params(value)?).await, false),
            "Review the restore plan before confirming.",
            true,
        ),
        "alcomd_apply_backup_restore" => {
            let input: rpc::McpBackupApplyRestoreParams = params(value)?;
            if !input.confirm {
                return Err(invalid());
            }
            (
                reply(client.mcp_backup_apply_restore(input).await, true),
                "Backup restore accepted. Check the operation.",
                false,
            )
        }
        _ => return Err(invalid()),
    };
    Ok(match result {
        Ok(value) => success(value, summary.into(), plan),
        Err(error) => failure(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn restore_result_does_not_accept_the_native_absolute_target() {
        let mut result = json!({
            "backupId": "00000000-0000-4000-8000-000000000001",
            "projectId": "00000000-0000-4000-8000-000000000002",
            "targetName": "Restored project",
            "packagesRequireResolve": true,
            "excludedPackages": []
        });
        assert!(matches!(
            operation_result("backups.restore", result.clone()),
            Ok(projected) if projected == result
        ));
        result["target"] = json!({"parent": "/private/path", "leaf": "Restored project"});
        assert!(operation_result("backups.restore", result).is_err());
    }
}
