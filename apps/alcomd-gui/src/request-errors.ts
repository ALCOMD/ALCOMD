import type { RpcError } from "@alcomd/sdk";

/** Stable error codes only; remote messages and payloads never become UI text. */
export function normalizeRequestError(caught: unknown): RpcError {
    const fallback: RpcError = { code: "internal_error", message: "" };
    if (typeof caught !== "object" || caught === null) return fallback;
    try {
        const value = caught as Record<string, unknown>;
        const code = typeof value.code === "string" && /^[a-z][a-z0-9_]{0,127}$/.test(value.code) ? value.code : "internal_error";
        const diagnosticId = typeof value.diagnosticId === "string" && /^[A-Za-z0-9][A-Za-z0-9._:-]{0,4095}$/.test(value.diagnosticId)
            ? value.diagnosticId : undefined;
        // RpcError requires a message field; discard its original contents.
        return { code, message: "", ...(diagnosticId === undefined ? {} : { diagnosticId }) };
    } catch {
        return fallback;
    }
}

/** Finite mappings to authored translation keys, without guessing from prefixes. */
export function requestErrorMessage(code: string): string {
    switch (code) {
        case "invalid_request":
        case "invalid_params":
        case "invalid_input":
            return "Some information is missing or invalid. Check the inputs before submitting again.";
        case "daemon_unavailable":
            return "Core is unavailable. Reconnect and check any submitted task before repeating a change.";
        case "internal_error":
            return "The result could not be confirmed because of an internal error. Check Tasks before repeating a change.";
        case "permission_denied":
            return "You do not have permission for this action. Review the required access.";
        case "revision_conflict":
        case "resource_revision_conflict":
        case "project_revision_conflict":
        case "repository_revision_conflict":
            return "The data changed after it was loaded. Refresh it before making another change.";
        case "idempotency_conflict":
            return "This request conflicts with an earlier submission. Check the existing task instead of submitting it again.";
        case "store_unavailable":
            return "Core could not read or save its state. Check Core status and any current task before continuing.";
        case "handshake_required":
        case "handshake_already_completed":
            return "The Core connection needs to be renewed. Reconnect to continue.";
        case "method_not_found":
        case "rpc_version_unsupported":
        case "component_upgrade_required":
        case "capability_required":
        case "data_schema_unsupported":
            return "This app, Core, or data version is not compatible with this action. Use matching versions.";
        case "operation_not_found":
            return "This task is unavailable or no longer visible. Refresh the task list.";
        case "operation_not_cancellable":
            return "This task can no longer be cancelled. Refresh to see its current result.";
        case "path_encoding_unsupported":
        case "file_selection_failed":
        case "directory_selection_failed":
            return "The selected location could not be used. Check the path and access permissions, or choose another location.";

        case "extension_manifest_invalid":
        case "extension_package_invalid":
            return "The extension package or manifest is invalid. Choose a valid extension package.";
        case "extension_package_untrusted":
            return "This extension package is not trusted for this install. Check its publisher and use an approved package.";
        case "extension_publisher_confirmation_required":
            return "The publisher has not been approved for this extension. If you trust it, select the publisher approval checkbox and create the install plan again.";
        case "extension_signature_invalid":
            return "The extension signature could not be verified. Obtain a valid package from its publisher before installing.";
        case "extension_already_installed":
            return "This extension is already installed. Use its existing management page.";
        case "extension_not_installed":
            return "This extension is not installed. Refresh the extension list.";
        case "extension_not_enabled":
            return "This extension is not enabled. Check its state in extension management.";
        case "extension_permission_denied":
        case "extension_scope_denied":
            return "This action is outside the extension's allowed permissions or scope. Review its access to the selected resource.";
        case "extension_api_unsupported":
        case "extension_ui_protocol_unsupported":
            return "This extension uses an unsupported API or interface version. Use a compatible extension and app.";
        case "extension_instance_stale":
        case "extension_ui_session_not_found":
        case "extension_ui_session_stale":
            return "The extension session has ended or changed. Refresh its state and reopen its page.";
        case "extension_ui_snapshot_stale":
            return "The extension page has changed. Refresh it before making another change.";
        case "extension_ui_not_available":
            return "This extension does not provide a supported function page. Use its management page instead.";
        case "extension_ui_document_invalid":
            return "The extension returned an invalid page. Check its status before reopening it.";
        case "extension_ui_action_invalid":
            return "This extension action is invalid. Check the inputs or refresh its page before continuing.";
        case "extension_ui_limit_exceeded":
        case "extension_resource_limit":
            return "The extension exceeded supported limits. Reduce the input or check its runtime status.";
        case "extension_crashed":
            return "The extension stopped unexpectedly. Check its current state before continuing.";
        case "extension_quarantined":
            return "The extension is quarantined. Review its status before choosing Recover extension.";
        case "extension_data_quota_exceeded":
            return "The extension's data limit was reached. Check its status or contact its publisher.";
        case "extension_data_owner_mismatch":
            return "The extension data ownership check failed. Stop this action and report it to the extension publisher.";

        case "repository_not_found":
        case "repository_not_registered":
            return "This repository is no longer available in the list. Refresh the repository list.";
        case "repository_already_registered":
        case "duplicate_source":
            return "The same repository source is already listed. Keep one entry.";
        case "repository_source_invalid":
            return "The repository source is invalid. Check the value you entered.";
        case "source_invalid":
            return "This entry does not contain a valid repository address. Check the URL.";
        case "repository_inaccessible":
            return "The repository source could not be accessed. Check its location and access permissions.";
        case "repository_unavailable":
            return "The repository could not be reached. Check its source and the network connection.";
        case "repository_document_invalid":
            return "The repository document is invalid. Check that the source is a VPM repository manifest.";
        case "repository_document_too_large":
            return "The repository manifest exceeds the size limit. Check the source or contact its publisher.";
        case "repository_credentials_unsupported":
        case "authentication_not_supported":
            return "This action cannot preserve the source's embedded credentials or authentication headers. Handle this source separately.";
        case "repository_refresh_required":
            return "The repository metadata needs refreshing. Refresh it before reviewing package changes.";
        case "repository_list_invalid":
            return "The repository list is invalid. Use UTF-8 and stay within 1 MiB and 4,096 entries.";
        case "repository_list_configuration_failed":
        case "repository_list_import_incomplete":
            return "Some repository configuration could not be applied. Completed changes are kept. Review the repository list before importing again.";
        case "repository_list_changed":
            return "The repository list changed after preview. Preview it again before exporting.";
        case "repository_list_target_exists":
            return "The destination already exists and was kept. Choose a new file name.";
        case "repository_list_target_invalid":
            return "This save location is not allowed. Choose a new file outside Core data and registered projects.";
        case "repository_list_io_error":
            return "The list file could not be read or written. Check its location and access permissions.";
        case "vcc_link_not_supported":
            return "This vcc link is not a supported repository link. Use a supported repository URL.";
        case "scheme_not_supported":
            return "This list supports only HTTP and HTTPS repository addresses. Use a supported URL.";
        case "local_source":
        case "unsupported_source":
            return "This source cannot be represented in the repository URL list. Handle it separately.";
        case "user_package_not_found":
            return "This user package is no longer registered. Refresh the user package list.";
        case "user_package_already_enrolled":
            return "This package source is already registered. Use the existing user package.";
        case "user_package_source_unavailable":
            return "The user package folder is unavailable. Check its location and access permissions.";
        case "user_package_source_unsafe":
            return "The package folder did not pass safety checks. Choose a safe package source.";
        case "user_package_source_changed":
            return "The package source changed during the operation. Refresh it before continuing.";
        case "user_package_manifest_invalid":
            return "The package manifest is invalid. Check the selected package folder.";
        case "user_package_limit_exceeded":
            return "The user package exceeds supported limits. Reduce its contents before checking it again.";
        case "template_builtin_immutable":
            return "Built-in templates cannot be edited or removed.";
        case "template_conflict":
            return "A template uses this ID. Choose a different ID or replace the existing user template.";
        case "template_dependency_unsatisfied":
            return "The template packages could not be resolved. Check the package ranges, repositories and Unity version.";
        case "package_integrity_mismatch":
        case "backup_integrity_mismatch":
            return "The content failed its integrity check. Use a verified source before continuing.";

        case "plan_not_found":
        case "plan_stale":
        case "extension_plan_stale":
        case "backup_restore_plan_not_found":
        case "backup_restore_plan_stale":
        case "project_copy_plan_not_found":
        case "project_copy_plan_stale":
        case "project_delete_plan_not_found":
        case "project_delete_plan_stale":
        case "project_unity_migration_plan_not_found":
        case "project_unity_migration_plan_stale":
            return "This plan is unavailable or out of date. Discard it and review a new plan before applying.";
        case "extension_recovery_required":
        case "recovery_required":
        case "project_transaction_recovery_required":
        case "backup_restore_recovery_required":
        case "project_copy_recovery_required":
        case "project_delete_recovery_required":
        case "project_unity_migration_recovery_required":
            return "The previous operation needs recovery. Check its task before starting another change.";
        case "project_not_found":
        case "project_not_registered":
            return "This project is not available in the registry. Refresh the project list.";
        case "project_already_registered":
            return "This project is already registered. Use its existing entry.";
        case "project_inaccessible":
        case "project_delete_source_missing":
            return "The project folder is unavailable. Check its location and access permissions.";
        case "project_version_missing":
        case "project_version_invalid":
            return "The project's Unity version cannot be read. Check its version file before choosing an Editor.";
        case "project_manifest_invalid":
            return "A package manifest in the project could not be read. Check its format and contents.";
        case "project_changed_during_apply":
        case "project_copy_source_changed":
        case "project_delete_source_changed":
        case "project_unity_migration_source_changed":
            return "The project changed after review. Refresh it and review a new plan before continuing.";
        case "project_create_destination_changed":
            return "The destination folder changed during creation. Check the location and try again.";
        case "project_create_target_exists":
        case "backup_target_exists":
        case "project_copy_target_exists":
            return "The target already exists. Choose a new name or location; existing files were kept.";
        case "backup_target_invalid":
        case "project_copy_target_unsafe":
        case "project_copy_source_unsafe":
        case "project_delete_source_unsafe":
        case "backup_source_unsafe":
            return "The selected location did not pass safety checks. Review it or choose another valid location.";
        case "project_copy_source_missing":
            return "The source project folder does not exist. Check the source project's path before copying.";
        case "backup_not_found":
        case "backup_unavailable":
            return "This backup is unavailable. Refresh the backup list and check that its archive is accessible.";
        case "project_changed_during_backup":
            return "The project changed while backing up. Check the task result before creating another backup.";
        case "backup_archive_limit_exceeded":
        case "plan_too_large":
        case "project_copy_limit_exceeded":
        case "package_candidate_limit_exceeded":
            return "The request exceeds supported limits. Reduce the selection or input size.";
        case "unity_installation_not_found":
            return "That Unity installation is not registered. Refresh the installed Editor list.";
        case "unity_installation_invalid":
        case "unity_version_unverified":
            return "The Unity Editor could not be verified. Check its executable and version, then refresh installations.";
        case "unity_version_mismatch":
            return "The installed Editor does not exactly match the project's Unity version. Choose a matching installation.";
        case "unity_editor_selection_required":
            return "More than one matching Editor is available. Choose the installation for this launch.";
        case "unity_architecture_unsupported":
            return "This Unity Editor architecture is not supported. Choose a supported installation.";
        case "unity_installation_in_use":
        case "unity_project_running":
            return "Unity is using this installation or project. Finish the active work before changing it.";
        case "unity_launch_state_uncertain":
            return "This project may already be open in Unity. Check its Editor before launching again.";
        case "unity_launch_failed":
        case "unity_launch_not_found":
            return "The Unity launch could not be completed or found. Check the Editor's current state before launching again.";
        case "hub_path_changed":
        case "hub_path_invalid":
        case "hub_unavailable":
        case "hub_open_failed":
            return "Unity Hub could not be opened at the selected location. Check its path in Settings.";
        case "hub_open_busy":
            return "A Unity Hub launch is already being processed. Check its current state before launching again.";
        default:
            return "The request did not complete for an unrecognized reason.";
    }
}
