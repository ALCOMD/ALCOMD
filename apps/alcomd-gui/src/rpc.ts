import type {
    ExtensionResult,
    ExtensionUiCloseParams,
    ExtensionUiCloseResult,
    ExtensionUiDispatchParams,
    ExtensionUiDispatchResult,
    ExtensionUiOpenParams,
    ExtensionUiOpenResult,
    ExtensionUiRefreshParams,
    ExtensionUiSnapshotResult,
    RpcError
} from "@alcomd/sdk";
import { invoke } from "@tauri-apps/api/core";

import type {
    ActivityCursor,
    ActivityListResult,
    BackupApplyRestoreResult,
    BackupCreateResult,
    BackupRecord,
    BackupRestorePlan,
    BackupsListResult,
    DiagnosticCursor,
    DiagnosticsListResult,
    ExtensionGrantResult,
    ExtensionOperationResult,
    ExtensionPlanResult,
    ExtensionsListResult,
    Operation,
    OperationAccepted,
    OperationWriteResult,
    OperationsListResult,
    PackageApplyPlanParams,
    PackagePlan,
    PackagePlanDowngradeParams,
    PackagePlanInstallParams,
    PackagePlanRemoveParams,
    PackagePlanResolveParams,
    ProjectEditorResult,
    ProjectEditorSelectionResult,
    ProjectEditorClearResult,
    ProjectResult,
    ProjectsApplyCopyResult,
    ProjectsPlanCopyResult,
    ProjectUnregisterResult,
    ProjectWriteResult,
    ProjectsListResult,
    RegistryCursor,
    RepositoriesListResult,
    RepositoryPackagesResult,
    RepositoryResult,
    RepositorySource,
    RepositoryUnregisterResult,
    RepositoryWriteResult,
    SettingsGetResult,
    SettingsUpdate,
    SystemStatus,
    TemplateApplyResult,
    TemplateBundleInspection,
    TemplateExportResult,
    TemplatePlan,
    TemplateRecordResult,
    TemplateRemoveResult,
    TemplatesListResult,
    UnityInstallationResult,
    UnityInstallationRemoveResult,
    UnityInstallationsListResult,
    UnityLaunchResult,
    UnityWriterState
} from "./core-models";

export interface GuiRpcClient {
    systemStatus(): Promise<SystemStatus>;
    stateCheck(): Promise<OperationAccepted>;
    operationsList(): Promise<OperationsListResult>;
    operationGet(operationId: string): Promise<Operation>;
    operationCancel(operationId: string, expectedRevision: number): Promise<OperationWriteResult>;
    settingsGet(): Promise<SettingsGetResult>;
    settingsUpdate(expectedRevision: number, update: SettingsUpdate): Promise<SettingsGetResult>;
    activityList(cursor?: ActivityCursor): Promise<ActivityListResult>;
    diagnosticsList(cursor?: DiagnosticCursor): Promise<DiagnosticsListResult>;
    projectsInspect(path: string, discoveryMode: "exact-root" | "search-parents"): Promise<ProjectResult>;
    projectsList(cursor?: RegistryCursor): Promise<ProjectsListResult>;
    projectGet(projectId: string): Promise<ProjectResult>;
    openProjectDirectory(projectId: string): Promise<void>;
    selectDirectory(): Promise<string | undefined>;
    projectRegister(path: string): Promise<ProjectWriteResult>;
    projectRefresh(projectId: string, expectedRevision: number): Promise<ProjectWriteResult>;
    projectSetFavorite(projectId: string, favorite: boolean, expectedRevision: number): Promise<ProjectWriteResult>;
    projectUnregister(projectId: string, expectedRevision: number): Promise<ProjectUnregisterResult>;
    projectPlanCopy(sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string): Promise<ProjectsPlanCopyResult>;
    projectApplyCopy(planId: string, expectedRevision: number): Promise<ProjectsApplyCopyResult>;
    repositoriesInspect(source: RepositorySource): Promise<RepositoryResult>;
    repositoriesList(): Promise<RepositoriesListResult>;
    repositoryGet(repositoryId: string): Promise<RepositoryResult>;
    repositoryPackages(repositoryId: string): Promise<RepositoryPackagesResult>;
    repositoryRegister(source: RepositorySource): Promise<RepositoryWriteResult>;
    repositoryRefresh(repositoryId: string, expectedRevision: number): Promise<RepositoryWriteResult>;
    repositoryUnregister(repositoryId: string, expectedRevision: number): Promise<RepositoryUnregisterResult>;
    packagePlanInstall(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanRemove(params: PackagePlanRemoveParams): Promise<PackagePlan>;
    packagePlanUpgrade(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanDowngrade(params: PackagePlanDowngradeParams): Promise<PackagePlan>;
    packagePlanResolve(params: PackagePlanResolveParams): Promise<PackagePlan>;
    packageApplyPlan(params: Omit<PackageApplyPlanParams, "idempotencyKey">): Promise<{ operationId: string; replayed: boolean }>;
    unityInstallationsList(cursor?: string): Promise<UnityInstallationsListResult>;
    unityInstallationGet(installationId: string): Promise<UnityInstallationResult>;
    unityInstallationRegister(executablePath: string): Promise<UnityInstallationResult>;
    unityInstallationRemove(installationId: string, expectedRevision: number): Promise<UnityInstallationRemoveResult>;
    unityInstallationsRefresh(): Promise<UnityInstallationsListResult>;
    unityProjectEditorGet(projectId: string): Promise<ProjectEditorResult>;
    unityProjectEditorSet(projectId: string, installationId: string, arguments_: string[], expectedRevision: number): Promise<ProjectEditorResult>;
    unityProjectEditorSelectionGet(projectId: string): Promise<ProjectEditorSelectionResult>;
    unityProjectEditorClear(projectId: string, expectedRevision: number): Promise<ProjectEditorClearResult>;
    unityWriterState(projectId: string): Promise<UnityWriterState>;
    unityLaunch(projectId: string, expectedProjectRevision: number): Promise<UnityLaunchResult>;
    unityLaunchStatus(launchId: string): Promise<UnityLaunchResult>;
    templatesList(): Promise<TemplatesListResult>;
    templateGet(templateId: string): Promise<TemplateRecordResult>;
    templateInspectBundle(bundlePath: string): Promise<TemplateBundleInspection>;
    templatePlanImport(bundlePath: string, overrideExisting: boolean, expectedRevision: number): Promise<TemplatePlan>;
    templateApplyImport(planId: string): Promise<TemplateApplyResult>;
    templatePlanDerive(params: { projectId: string; expectedProjectRevision: number; templateId: string; templateVersion: string; displayName: string; description?: string }): Promise<TemplatePlan>;
    templateApplyDerive(planId: string): Promise<TemplateApplyResult>;
    templateExport(templateId: string, expectedRevision: number, targetPath: string): Promise<TemplateExportResult>;
    templateSetFavorite(templateId: string, favorite: boolean, expectedRevision: number): Promise<TemplateRecordResult>;
    templateRemove(templateId: string, expectedRevision: number): Promise<TemplateRemoveResult>;
    templatePlanCreateProject(templateId: string, expectedTemplateRevision: number, targetParent: string, targetLeaf: string): Promise<TemplatePlan>;
    templateApplyCreateProject(planId: string): Promise<TemplateApplyResult>;
    backupsList(projectId?: string): Promise<BackupsListResult>;
    backupGet(backupId: string): Promise<BackupRecord>;
    backupCreate(projectId: string, expectedRevision: number, compressionMode: "store" | "fast" | "maximum", excludeVpmPackages: boolean): Promise<BackupCreateResult>;
    backupPlanRestore(backupId: string, targetParent: string, targetLeaf: string): Promise<BackupRestorePlan>;
    backupApplyRestore(planId: string): Promise<BackupApplyRestoreResult>;
    extensionsList(): Promise<ExtensionsListResult>;
    extensionGet(extensionId: string): Promise<ExtensionResult>;
    extensionPlanInstall(packagePath: string, expectedRevision: number, publisherApproval: "none" | "approve_for_extension"): Promise<ExtensionPlanResult>;
    extensionApplyInstall(planId: string): Promise<ExtensionOperationResult>;
    extensionEnable(extensionId: string, expectedRevision: number): Promise<ExtensionResult>;
    extensionDisable(extensionId: string, expectedRevision: number): Promise<ExtensionResult>;
    extensionPlanUninstall(extensionId: string, expectedRevision: number, dataDisposition: "retain_data" | "delete_data"): Promise<ExtensionPlanResult>;
    extensionApplyUninstall(planId: string): Promise<ExtensionOperationResult>;
    extensionSetGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number): Promise<ExtensionGrantResult>;
    extensionRevokeGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number): Promise<ExtensionGrantResult>;
    extensionUiOpen(params: ExtensionUiOpenParams): Promise<ExtensionUiOpenResult>;
    extensionUiRefresh(params: ExtensionUiRefreshParams): Promise<ExtensionUiSnapshotResult>;
    extensionUiDispatch(params: ExtensionUiDispatchParams): Promise<ExtensionUiDispatchResult>;
    extensionUiClose(params: ExtensionUiCloseParams): Promise<ExtensionUiCloseResult>;
}

class TauriGuiRpcClient implements GuiRpcClient {
    systemStatus(): Promise<SystemStatus> {
        return invokeTyped("gui_system_status", {});
    }

    stateCheck(): Promise<OperationAccepted> {
        return invokeTyped("gui_state_check", { idempotencyKey: crypto.randomUUID() });
    }

    operationsList(): Promise<OperationsListResult> {
        return invokeTyped("gui_operations_list", { params: { limit: 100 } });
    }

    operationGet(operationId: string): Promise<Operation> {
        return invokeTyped("gui_operation_get", { operationId });
    }

    operationCancel(operationId: string, expectedRevision: number): Promise<OperationWriteResult> {
        return invokeTyped("gui_operation_cancel", { operationId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    settingsGet(): Promise<SettingsGetResult> {
        return invokeTyped("gui_settings_get", {});
    }

    settingsUpdate(expectedRevision: number, update: SettingsUpdate): Promise<SettingsGetResult> {
        return invokeTyped("gui_settings_update", { params: { expectedRevision, update } });
    }

    activityList(cursor?: ActivityCursor): Promise<ActivityListResult> {
        return invokeTyped("gui_activity_list", { params: { cursor, limit: 100 } });
    }

    diagnosticsList(cursor?: DiagnosticCursor): Promise<DiagnosticsListResult> {
        return invokeTyped("gui_diagnostics_list", { params: { cursor, limit: 100 } });
    }

    projectsInspect(path: string, discoveryMode: "exact-root" | "search-parents"): Promise<ProjectResult> {
        return invokeTyped("gui_projects_inspect", { params: { path, discoveryMode } });
    }

    projectsList(cursor?: RegistryCursor): Promise<ProjectsListResult> {
        return invokeTyped("gui_projects_list", { params: { cursor, limit: 100 } });
    }

    projectGet(projectId: string): Promise<ProjectResult> {
        return invokeTyped("gui_project_get", { projectId });
    }

    openProjectDirectory(projectId: string): Promise<void> {
        return invokeTyped("gui_open_project_directory", { projectId });
    }

    selectDirectory(): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_directory", {}).then((path) => path ?? undefined);
    }

    projectRegister(path: string): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_register", { path, idempotencyKey: crypto.randomUUID() });
    }

    projectRefresh(projectId: string, expectedRevision: number): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_refresh", { projectId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    projectSetFavorite(projectId: string, favorite: boolean, expectedRevision: number): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_set_favorite", { params: { projectId, favorite, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    projectUnregister(projectId: string, expectedRevision: number): Promise<ProjectUnregisterResult> {
        return invokeTyped("gui_project_unregister", { projectId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    projectPlanCopy(sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string): Promise<ProjectsPlanCopyResult> {
        return invokeTyped("gui_project_plan_copy", { params: { sourceProjectId, expectedRevision, targetParentPath, targetLeaf, idempotencyKey: crypto.randomUUID() } });
    }

    projectApplyCopy(planId: string, expectedRevision: number): Promise<ProjectsApplyCopyResult> {
        return invokeTyped("gui_project_apply_copy", { params: { planId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    repositoriesInspect(source: RepositorySource): Promise<RepositoryResult> {
        return invokeTyped("gui_repositories_inspect", { params: { source } });
    }

    repositoriesList(): Promise<RepositoriesListResult> {
        return invokeTyped("gui_repositories_list", { params: { limit: 100 } });
    }

    repositoryGet(repositoryId: string): Promise<RepositoryResult> {
        return invokeTyped("gui_repository_get", { repositoryId });
    }

    repositoryPackages(repositoryId: string): Promise<RepositoryPackagesResult> {
        return invokeTyped("gui_repository_packages", { params: { repositoryId, limit: 100 } });
    }

    repositoryRegister(source: RepositorySource): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_register", { source, idempotencyKey: crypto.randomUUID() });
    }

    repositoryRefresh(repositoryId: string, expectedRevision: number): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_refresh", { repositoryId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    repositoryUnregister(repositoryId: string, expectedRevision: number): Promise<RepositoryUnregisterResult> {
        return invokeTyped("gui_repository_unregister", { repositoryId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    packagePlanInstall(params: PackagePlanInstallParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_install", { params });
    }

    packagePlanRemove(params: PackagePlanRemoveParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_remove", { params });
    }

    packagePlanUpgrade(params: PackagePlanInstallParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_upgrade", { params });
    }

    packagePlanDowngrade(params: PackagePlanDowngradeParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_downgrade", { params });
    }

    packagePlanResolve(params: PackagePlanResolveParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_resolve", { params });
    }

    packageApplyPlan(params: Omit<PackageApplyPlanParams, "idempotencyKey">): Promise<{ operationId: string; replayed: boolean }> {
        return invokeTyped("gui_package_apply_plan", { params: { ...params, idempotencyKey: crypto.randomUUID() } });
    }

    unityInstallationsList(cursor?: string): Promise<UnityInstallationsListResult> {
        return invokeTyped("gui_unity_installations_list", { params: { cursor, limit: 100 } });
    }

    unityInstallationGet(installationId: string): Promise<UnityInstallationResult> {
        return invokeTyped("gui_unity_installation_get", { installationId });
    }

    unityInstallationRegister(executablePath: string): Promise<UnityInstallationResult> {
        return invokeTyped("gui_unity_installation_register", { executablePath, idempotencyKey: crypto.randomUUID() });
    }

    unityInstallationRemove(installationId: string, expectedRevision: number): Promise<UnityInstallationRemoveResult> {
        return invokeTyped("gui_unity_installation_remove", { installationId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    unityInstallationsRefresh(): Promise<UnityInstallationsListResult> {
        return invokeTyped("gui_unity_installations_refresh", { idempotencyKey: crypto.randomUUID() });
    }

    unityProjectEditorGet(projectId: string): Promise<ProjectEditorResult> {
        return invokeTyped("gui_unity_project_editor_get", { projectId });
    }

    unityProjectEditorSet(projectId: string, installationId: string, arguments_: string[], expectedRevision: number): Promise<ProjectEditorResult> {
        return invokeTyped("gui_unity_project_editor_set", { params: { projectId, installationId, arguments: arguments_, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    unityProjectEditorSelectionGet(projectId: string): Promise<ProjectEditorSelectionResult> {
        return invokeTyped("gui_unity_project_editor_selection_get", { projectId });
    }

    unityProjectEditorClear(projectId: string, expectedRevision: number): Promise<ProjectEditorClearResult> {
        return invokeTyped("gui_unity_project_editor_clear", { params: { projectId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    unityWriterState(projectId: string): Promise<UnityWriterState> {
        return invokeTyped("gui_unity_writer_state", { projectId });
    }

    unityLaunch(projectId: string, expectedProjectRevision: number): Promise<UnityLaunchResult> {
        return invokeTyped("gui_unity_launch", { params: { projectId, expectedProjectRevision, idempotencyKey: crypto.randomUUID() } });
    }

    unityLaunchStatus(launchId: string): Promise<UnityLaunchResult> {
        return invokeTyped("gui_unity_launch_status", { launchId });
    }

    templatesList(): Promise<TemplatesListResult> {
        return invokeTyped("gui_templates_list", { params: { limit: 100 } });
    }

    templateGet(templateId: string): Promise<TemplateRecordResult> {
        return invokeTyped("gui_template_get", { templateId });
    }

    templateInspectBundle(bundlePath: string): Promise<TemplateBundleInspection> {
        return invokeTyped("gui_template_inspect_bundle", { bundlePath });
    }

    templatePlanImport(bundlePath: string, overrideExisting: boolean, expectedRevision: number): Promise<TemplatePlan> {
        return invokeTyped("gui_template_plan_import", { params: { bundlePath, override: overrideExisting, expectedRevision } });
    }

    templateApplyImport(planId: string): Promise<TemplateApplyResult> {
        return invokeTyped("gui_template_apply_import", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    templatePlanDerive(params: { projectId: string; expectedProjectRevision: number; templateId: string; templateVersion: string; displayName: string; description?: string }): Promise<TemplatePlan> {
        return invokeTyped("gui_template_plan_derive", { params });
    }

    templateApplyDerive(planId: string): Promise<TemplateApplyResult> {
        return invokeTyped("gui_template_apply_derive", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    templateExport(templateId: string, expectedRevision: number, targetPath: string): Promise<TemplateExportResult> {
        return invokeTyped("gui_template_export", { params: { templateId, expectedRevision, targetPath } });
    }

    templateSetFavorite(templateId: string, favorite: boolean, expectedRevision: number): Promise<TemplateRecordResult> {
        return invokeTyped("gui_template_set_favorite", { params: { templateId, favorite, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    templateRemove(templateId: string, expectedRevision: number): Promise<TemplateRemoveResult> {
        return invokeTyped("gui_template_remove", { params: { templateId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    templatePlanCreateProject(templateId: string, expectedTemplateRevision: number, targetParent: string, targetLeaf: string): Promise<TemplatePlan> {
        return invokeTyped("gui_template_plan_create_project", { params: { templateId, expectedTemplateRevision, targetParent, targetLeaf } });
    }

    templateApplyCreateProject(planId: string): Promise<TemplateApplyResult> {
        return invokeTyped("gui_template_apply_create_project", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    backupsList(projectId?: string): Promise<BackupsListResult> {
        return invokeTyped("gui_backups_list", { params: { projectId, limit: 100 } });
    }

    backupGet(backupId: string): Promise<BackupRecord> {
        return invokeTyped("gui_backup_get", { backupId });
    }

    backupCreate(projectId: string, expectedRevision: number, compressionMode: "store" | "fast" | "maximum", excludeVpmPackages: boolean): Promise<BackupCreateResult> {
        return invokeTyped("gui_backup_create", { params: { projectId, expectedRevision, compressionMode, excludeVpmPackages, idempotencyKey: crypto.randomUUID() } });
    }

    backupPlanRestore(backupId: string, targetParent: string, targetLeaf: string): Promise<BackupRestorePlan> {
        return invokeTyped("gui_backup_plan_restore", { params: { backupId, targetParent, targetLeaf } });
    }

    backupApplyRestore(planId: string): Promise<BackupApplyRestoreResult> {
        return invokeTyped("gui_backup_apply_restore", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    extensionsList(): Promise<ExtensionsListResult> {
        return invokeTyped("gui_extensions_list", { params: { limit: 100 } });
    }

    extensionGet(extensionId: string): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_get", { extensionId });
    }

    extensionPlanInstall(packagePath: string, expectedRevision: number, publisherApproval: "none" | "approve_for_extension"): Promise<ExtensionPlanResult> {
        return invokeTyped("gui_extension_plan_install", { params: { sourceKind: "local_owner_selected", packagePath, expectedRevision, publisherApproval } });
    }

    extensionApplyInstall(planId: string): Promise<ExtensionOperationResult> {
        return invokeTyped("gui_extension_apply_install", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    extensionEnable(extensionId: string, expectedRevision: number): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_enable", { params: { extensionId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    extensionDisable(extensionId: string, expectedRevision: number): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_disable", { params: { extensionId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    extensionPlanUninstall(extensionId: string, expectedRevision: number, dataDisposition: "retain_data" | "delete_data"): Promise<ExtensionPlanResult> {
        return invokeTyped("gui_extension_plan_uninstall", { params: { extensionId, expectedRevision, dataDisposition } });
    }

    extensionApplyUninstall(planId: string): Promise<ExtensionOperationResult> {
        return invokeTyped("gui_extension_apply_uninstall", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    extensionSetGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number): Promise<ExtensionGrantResult> {
        return invokeTyped("gui_extension_set_grant", { params: { extensionId, permission, resourceKind, resourceId, expectedGrantRevision, idempotencyKey: crypto.randomUUID() } });
    }

    extensionRevokeGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number): Promise<ExtensionGrantResult> {
        return invokeTyped("gui_extension_revoke_grant", { params: { extensionId, permission, resourceKind, resourceId, expectedGrantRevision, idempotencyKey: crypto.randomUUID() } });
    }

    extensionUiOpen(params: ExtensionUiOpenParams): Promise<ExtensionUiOpenResult> {
        return invokeTyped("gui_extension_ui_open", { params });
    }

    extensionUiRefresh(params: ExtensionUiRefreshParams): Promise<ExtensionUiSnapshotResult> {
        return invokeTyped("gui_extension_ui_refresh", { params });
    }

    extensionUiDispatch(params: ExtensionUiDispatchParams): Promise<ExtensionUiDispatchResult> {
        return invokeTyped("gui_extension_ui_dispatch", { params });
    }

    extensionUiClose(params: ExtensionUiCloseParams): Promise<ExtensionUiCloseResult> {
        return invokeTyped("gui_extension_ui_close", { params });
    }
}

async function invokeTyped<Result>(command: string, args: Record<string, unknown>): Promise<Result> {
    try {
        return await invoke<Result>(command, args);
    } catch (error: unknown) {
        throw normalizeRpcError(error);
    }
}

export function normalizeRpcError(value: unknown): RpcError {
    if (isRecord(value) && typeof value.code === "string") {
        return {
            code: value.code,
            message: typeof value.message === "string"
                ? value.message
                : "The request could not be completed.",
            ...(typeof value.diagnosticId === "string"
                ? { diagnosticId: value.diagnosticId }
                : {})
        };
    }
    return {
        code: "daemon_unavailable",
        message: "The ALCOMD core is unavailable."
    };
}

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null;
}

export const guiRpcClient: GuiRpcClient = new TauriGuiRpcClient();
