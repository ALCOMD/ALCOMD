import type { ProjectsCreateParams, ProjectsCreateResult, ProjectTemplatesExportParams, ProjectTemplatesExportResult, ProjectTemplatesListResult, ProjectTemplatesSaveParams, ProjectTemplatesSaveResult, ProjectTemplatesRemoveParams, ProjectTemplatesRemoveResult } from "@alcomd/sdk";
import type { LogsReadParams, LogsReadResult } from "@alcomd/sdk";
import type { ProjectCandidateRequest, ProjectCandidatePage, ExtensionResult, ExtensionPermissionsListResult, ExtensionUiCloseParams, ExtensionUiCloseResult, ExtensionUiDispatchParams, ExtensionUiDispatchResult, ExtensionUiOpenParams, ExtensionUiOpenResult, ExtensionUiRefreshParams, ExtensionUiSnapshotResult, RpcError } from "@alcomd/sdk";
import type { UnityHubOpenResult } from "@alcomd/sdk";
import type { RepositoryCredentials, RepositoryListCancelParams, RepositoryListCancelResult, RepositoryListExportResult, RepositoryListImportParams, RepositoryListImportResult, RepositoryListInspection, RepositoryListPreview, RepositoryRefreshAllParams, RepositoryRefreshAllResult, RepositorySetDisplayNameParams } from "@alcomd/sdk";
import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { GuiEventsRequest, GuiEventMessage, GuiEventSubscription } from "./event-types";
import type { GuiUnityWindows, UnityWindowSnapshot } from "./unity-window-observation";
import type { ProjectListView } from "./core-models";

import type { BackupApplyRestoreResult, BackupCreateResult, BackupRecord, BackupRestorePlan, BackupsListResult, ExtensionGrantResult, ExtensionOperationResult, ExtensionPlanResult, ExtensionsListResult, Operation, OperationAccepted, OperationWriteResult, OperationsListResult, PackageApplyPlanParams, PackageCursor, PackagePlan, PackagePlanBulkParams, PackagePlanDowngradeParams, PackagePlanInstallParams, PackagePlanRemoveParams, PackagePlanResolveParams, PackagePlanReinstallParams, ProjectUnityLaunchConfigMutationResult, ProjectUnityLaunchConfigResult, ProjectResult, ProjectsApplyCopyResult, ProjectsApplyDeleteDirectoryResult, ProjectsPlanCopyResult, ProjectsPlanDeleteDirectoryResult, ProjectsApplyUnityMigrationResult, ProjectsPlanUnityMigrationResult, ProjectUnregisterResult, ProjectWriteResult, ProjectsListResult, RegistryCursor, RepositoriesListResult, RepositoryPackagesResult, RepositoryResult, RepositorySource, RepositoryUnregisterResult, RepositoryWriteResult, SettingsGetResult, SettingsUpdate, SystemStatus, UnityInstallationResult, UnityInstallationRemoveResult, UnityInstallationsListResult, UnityLaunchResult, UnityLaunchOptionsResult, UnityWriterState, UserPackageCursor, UserPackageRemoveResult, UserPackageResult, UserPackagesListResult, UserPackageWriteResult } from "./core-models";

export type FileDrag = { type: "leave" } | { type: "hover"; x: number; y: number } | { type: "drop"; x: number; y: number; paths: string[] };
export type NativeFileKind = "repositoryManifest" | "unityEditor" | "extensionPackage";

export interface GuiRpcClient {
    readonly unityWindows?: GuiUnityWindows;
    subscribeEvents(request: GuiEventsRequest, receive: (message: GuiEventMessage) => void): Promise<GuiEventSubscription>;
    packageQueryProjectCandidates(params: ProjectCandidateRequest): Promise<ProjectCandidatePage>;
    systemStatus(): Promise<SystemStatus>;
    stateCheck(): Promise<OperationAccepted>;
    operationsList(cursor?: OperationsListResult["nextCursor"], activeOnly?: boolean): Promise<OperationsListResult>;
    logsRead(params: LogsReadParams): Promise<LogsReadResult>;
    operationGet(operationId: string): Promise<Operation>;
    operationCancel(operationId: string, expectedRevision: number): Promise<OperationWriteResult>;
    settingsGet(): Promise<SettingsGetResult>;
    settingsUpdate(expectedRevision: number, update: SettingsUpdate): Promise<SettingsGetResult>;
    projectsInspect(path: string, discoveryMode: "exact-root" | "search-parents"): Promise<ProjectResult>;
    projectsList(cursor?: RegistryCursor, view?: ProjectListView, expectedViewSequence?: number): Promise<ProjectsListResult>;
    projectGet(projectId: string): Promise<ProjectResult>;
    openProjectDirectory(projectId: string): Promise<void>;
    selectDirectory(currentPath?: string): Promise<string | undefined>;
    selectFile(kind: NativeFileKind, currentPath?: string): Promise<string | undefined>;
    subscribeFileDrag?(receive: (event: FileDrag) => void): Promise<() => void>;
    selectProjectTemplateFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined>;
    selectUnityHub(currentPath: string | null): Promise<string | undefined>;
    unityHubOpen(expectedRevision: number, expectedPath: string | null): Promise<UnityHubOpenResult>;
    openUnityHubDownload(): Promise<void>;
    projectRegister(path: string): Promise<ProjectWriteResult>;
    projectRefresh(projectId: string, expectedRevision: number): Promise<ProjectWriteResult>;
    projectSetFavorite(projectId: string, favorite: boolean, expectedRevision: number): Promise<ProjectWriteResult>;
    projectUnregister(projectId: string, expectedRevision: number): Promise<ProjectUnregisterResult>;
    projectPlanCopy(sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string): Promise<ProjectsPlanCopyResult>;
    projectApplyCopy(planId: string, expectedRevision: number): Promise<ProjectsApplyCopyResult>;
    projectPlanDeleteDirectory(projectId: string, expectedRevision: number): Promise<ProjectsPlanDeleteDirectoryResult>;
    projectApplyDeleteDirectory(planId: string, expectedRevision: number): Promise<ProjectsApplyDeleteDirectoryResult>;
    repositoriesInspect(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryResult>;
    selectRepositoryListFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined>;
    repositoryListInspectFile(path: string): Promise<RepositoryListInspection>;
    repositoryListImportFile(path: string, expectedFingerprint: string, idempotencyKey: string, overwriteExisting?: boolean): Promise<RepositoryListImportResult>;
    repositoryListCancelRegistration(idempotencyKey: string): Promise<RepositoryListCancelResult>;
    repositoryListPreviewExport(format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListPreview>;
    repositoryListExportFile(targetPath: string, expectedFingerprint: string, format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListExportResult>;
    repositoriesList(cursor?: RegistryCursor): Promise<RepositoriesListResult>;
    repositoryGet(repositoryId: string): Promise<RepositoryResult>;
    repositoryPackages(repositoryId: string, cursor?: PackageCursor): Promise<RepositoryPackagesResult>;
    openPackageLink(url: string): Promise<void>;
    repositoryRegister(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryWriteResult>;
    repositorySetCredentials(repositoryId: string, expectedRevision: number, credentials: RepositoryCredentials | null): Promise<RepositoryWriteResult>;
    repositorySetDisplayName(repositoryId: string, expectedRevision: number, displayName: string | null): Promise<RepositoryWriteResult>;
    repositoryRefresh(repositoryId: string, expectedRevision: number): Promise<RepositoryWriteResult>;
    repositoriesRefreshAll(): Promise<RepositoryRefreshAllResult>;
    repositoryUnregister(repositoryId: string, expectedRevision: number): Promise<RepositoryUnregisterResult>;
    packagePlanInstall(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanRemove(params: PackagePlanRemoveParams): Promise<PackagePlan>;
    packagePlanUpgrade(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanDowngrade(params: PackagePlanDowngradeParams): Promise<PackagePlan>;
    packagePlanResolve(params: PackagePlanResolveParams): Promise<PackagePlan>;
    packagePlanReinstall(params: PackagePlanReinstallParams): Promise<PackagePlan>;
    packagePlanBulk(params: PackagePlanBulkParams): Promise<PackagePlan>;
    packageApplyPlan(params: Omit<PackageApplyPlanParams, "idempotencyKey">): Promise<{ operationId: string; replayed: boolean }>;
    userPackagesList(cursor?: UserPackageCursor): Promise<UserPackagesListResult>;
    userPackageGet(userPackageId: string): Promise<UserPackageResult>;
    userPackageEnroll(sourcePath: string): Promise<UserPackageWriteResult>;
    userPackageRefresh(userPackageId: string, expectedRevision: number): Promise<UserPackageWriteResult>;
    userPackageRemove(userPackageId: string, expectedRevision: number): Promise<UserPackageRemoveResult>;
    unityInstallationsList(cursor?: string, includeVersionOptions?: boolean): Promise<UnityInstallationsListResult>;
    unityInstallationGet(installationId: string): Promise<UnityInstallationResult>;
    unityInstallationRegister(executablePath: string): Promise<UnityInstallationResult>;
    unityInstallationRemove(installationId: string, expectedRevision: number): Promise<UnityInstallationRemoveResult>;
    unityInstallationsRefresh(): Promise<UnityInstallationsListResult>;
    unityProjectLaunchConfigGet(projectId: string): Promise<ProjectUnityLaunchConfigResult>;
    unityProjectLaunchConfigSet(projectId: string, arguments_: string[], expectedRevision: number): Promise<ProjectUnityLaunchConfigMutationResult>;
    unityProjectLaunchConfigClear(projectId: string, expectedRevision: number): Promise<ProjectUnityLaunchConfigMutationResult>;
    unityWriterState(projectId: string): Promise<UnityWriterState>;
    unityLaunchOptions(projectId: string, expectedProjectRevision: number, includeMigrationVersionOptions?: boolean): Promise<UnityLaunchOptionsResult>;
    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number): Promise<UnityLaunchResult>;
    projectPlanUnityMigration(projectId: string, targetInstallationId: string, expectedProjectRevision: number): Promise<ProjectsPlanUnityMigrationResult>;
    projectApplyUnityMigration(planId: string): Promise<ProjectsApplyUnityMigrationResult>;
    projectsCreate(params: ProjectsCreateParams): Promise<ProjectsCreateResult>;
    projectTemplatesList(cursor?: string): Promise<ProjectTemplatesListResult>;
    projectTemplatesSave(params: ProjectTemplatesSaveParams): Promise<ProjectTemplatesSaveResult>;
    projectTemplatesExport(params: ProjectTemplatesExportParams): Promise<ProjectTemplatesExportResult>;
    projectTemplatesRemove(params: ProjectTemplatesRemoveParams): Promise<ProjectTemplatesRemoveResult>;
    backupsList(projectId?: string, cursor?: string): Promise<BackupsListResult>;
    backupGet(backupId: string): Promise<BackupRecord>;
    backupCreate(projectId: string, expectedRevision: number, compressionMode?: "store" | "fast" | "level3" | "level5" | "level7" | "maximum", excludeVpmPackages?: boolean, includeLibrary?: boolean): Promise<BackupCreateResult>;
    backupPlanRestore(backupId: string, targetParent: string, targetLeaf: string): Promise<BackupRestorePlan>;
    backupApplyRestore(planId: string): Promise<BackupApplyRestoreResult>;
    extensionsList(cursor?: string): Promise<ExtensionsListResult>;
    extensionGet(extensionId: string): Promise<ExtensionResult>;
    extensionPermissionsList(extensionId: string, cursor?: string): Promise<ExtensionPermissionsListResult>;
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
    selectFile(kind: NativeFileKind, currentPath?: string): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_file", { kind, currentPath: currentPath ?? null }).then((path) => path ?? undefined);
    }

    subscribeFileDrag(receive: (event: FileDrag) => void): Promise<() => void> {
        return getCurrentWebview().onDragDropEvent(({ payload }) => {
            if (payload.type === "leave") { receive({ type: "leave" }); return; }
            // Native positions are physical pixels; DOM hit testing uses CSS pixels.
            const x = payload.position.x / window.devicePixelRatio;
            const y = payload.position.y / window.devicePixelRatio;
            receive(payload.type === "drop" ? { type: "drop", x, y, paths: payload.paths } : { type: "hover", x, y });
        });
    }

    readonly unityWindows: GuiUnityWindows = {
        start: (token, targets) => invokeTyped<UnityWindowSnapshot>("gui_unity_windows_start", { token, targets }),
        next: (token, after) => invokeTyped<UnityWindowSnapshot>("gui_unity_windows_next", { token, after }),
        stop: (token) => invokeTyped<void>("gui_unity_windows_stop", { token }),
        focus: (projectId, revision) => invokeTyped<void>("gui_focus_project_unity", { projectId, revision })
    };

    async subscribeEvents(request: GuiEventsRequest, receive: (message: GuiEventMessage) => void): Promise<GuiEventSubscription> {
        const channel = new Channel<GuiEventMessage>();
        channel.onmessage = receive;
        await invokeTyped<void>("gui_events_start", { ...request, channel });
        return {
            acknowledge: (batch) => invokeTyped("gui_events_ack", { consumerId: request.consumerId, generation: request.generation,
                subscriptionId: batch.subscriptionId, batchId: batch.batchId }),
            close: () => invokeTyped("gui_events_cancel", { consumerId: request.consumerId, generation: request.generation })
        };
    }
    selectRepositoryListFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_repository_list_file", { mode, fileName, currentPath: currentPath ?? null }).then((path) => path ?? undefined);
    }

    selectProjectTemplateFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_project_template_file", { mode, fileName, currentPath: currentPath ?? null }).then((path) => path ?? undefined);
    }
    repositoryListInspectFile(path: string): Promise<RepositoryListInspection> {
        return invokeTyped("gui_repository_list_inspect_file", { path });
    }

    repositoryListImportFile(path: string, expectedFingerprint: string, idempotencyKey: string, overwriteExisting?: boolean): Promise<RepositoryListImportResult> {
        const params: RepositoryListImportParams = { path, expectedFingerprint, idempotencyKey, ...(overwriteExisting === undefined ? {} : { overwriteExisting }) };
        return invokeTyped("gui_repository_list_import_file", { params });
    }
    repositoryListCancelRegistration(idempotencyKey: string): Promise<RepositoryListCancelResult> {
        const params: RepositoryListCancelParams = { idempotencyKey };
        return invokeTyped("gui_repository_list_cancel_registration", { params });
    }
    repositoryListPreviewExport(format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListPreview> {
        return invokeTyped("gui_repository_list_preview_export", { format: format ?? "native" });
    }
    repositoryListExportFile(targetPath: string, expectedFingerprint: string, format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListExportResult> {
        return invokeTyped("gui_repository_list_export_file", { targetPath, expectedFingerprint, format: format ?? "native" });
    }
    packageQueryProjectCandidates(params: ProjectCandidateRequest): Promise<ProjectCandidatePage> {
        return invokeTyped("gui_package_query_project_candidates", { params });
    }
    systemStatus(): Promise<SystemStatus> {
        return invokeTyped("gui_system_status", {});
    }

    stateCheck(): Promise<OperationAccepted> {
        return invokeTyped("gui_state_check", { idempotencyKey: crypto.randomUUID() });
    }

    logsRead(params: LogsReadParams): Promise<LogsReadResult> {
        return invoke("gui_logs_read", { params });
    }

    operationsList(cursor?: OperationsListResult["nextCursor"], activeOnly = false): Promise<OperationsListResult> {
        return invokeTyped("gui_operations_list", { params: { cursor, limit: 100, activeOnly } });
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

    projectsInspect(path: string, discoveryMode: "exact-root" | "search-parents"): Promise<ProjectResult> {
        return invokeTyped("gui_projects_inspect", { params: { path, discoveryMode } });
    }

    projectsList(cursor?: RegistryCursor, view?: ProjectListView, expectedViewSequence?: number): Promise<ProjectsListResult> {
        return invokeTyped("gui_projects_list", { params: { cursor, limit: 100, view, expectedViewSequence } });
    }

    projectGet(projectId: string): Promise<ProjectResult> {
        return invokeTyped("gui_project_get", { projectId });
    }

    openProjectDirectory(projectId: string): Promise<void> {
        return invokeTyped("gui_open_project_directory", { projectId });
    }

    selectDirectory(currentPath?: string): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_directory", { currentPath: currentPath ?? null }).then((path) => path ?? undefined);
    }

    selectUnityHub(currentPath: string | null): Promise<string | undefined> {
        return invokeTyped<string | null>("gui_select_unity_hub", { currentPath }).then((path) => path ?? undefined);
    }

    unityHubOpen(expectedRevision: number, expectedPath: string | null): Promise<UnityHubOpenResult> {
        return invokeTyped("gui_unity_hub_open", { params: { expectedRevision, expectedPath } });
    }

    openUnityHubDownload(): Promise<void> {
        return invokeTyped("gui_open_unity_hub_download", {});
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

    projectPlanDeleteDirectory(projectId: string, expectedRevision: number): Promise<ProjectsPlanDeleteDirectoryResult> {
        return invokeTyped("gui_project_plan_delete_directory", { params: { projectId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    projectApplyDeleteDirectory(planId: string, expectedRevision: number): Promise<ProjectsApplyDeleteDirectoryResult> {
        return invokeTyped("gui_project_apply_delete_directory", { params: { planId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    repositoriesInspect(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryResult> {
        return invokeTyped("gui_repositories_inspect", { params: { source, credentials } });
    }

    repositoriesList(cursor?: RegistryCursor): Promise<RepositoriesListResult> {
        return invokeTyped("gui_repositories_list", { params: { cursor, limit: 100 } });
    }

    repositoryGet(repositoryId: string): Promise<RepositoryResult> {
        return invokeTyped("gui_repository_get", { repositoryId });
    }

    repositoryPackages(repositoryId: string, cursor?: PackageCursor): Promise<RepositoryPackagesResult> {
        return invokeTyped("gui_repository_packages", { params: { repositoryId, cursor, limit: 100 } });
    }

    openPackageLink(url: string): Promise<void> {
        return invokeTyped("gui_open_package_link", { url });
    }

    repositoryRegister(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_register", { source, credentials, idempotencyKey: crypto.randomUUID() });
    }

    repositorySetCredentials(repositoryId: string, expectedRevision: number, credentials: RepositoryCredentials | null): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_set_credentials", { params: { repositoryId, expectedRevision, credentials, idempotencyKey: crypto.randomUUID() } });
    }

    repositorySetDisplayName(repositoryId: string, expectedRevision: number, displayName: string | null): Promise<RepositoryWriteResult> {
        const params: RepositorySetDisplayNameParams = { repositoryId, expectedRevision, displayName, idempotencyKey: crypto.randomUUID() };
        return invokeTyped("gui_repository_set_display_name", { params });
    }

    repositoryRefresh(repositoryId: string, expectedRevision: number): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_refresh", { repositoryId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    repositoriesRefreshAll(): Promise<RepositoryRefreshAllResult> {
        const params: RepositoryRefreshAllParams = { idempotencyKey: crypto.randomUUID() };
        return invokeTyped("gui_repositories_refresh_all", { params });
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

    packagePlanReinstall(params: PackagePlanReinstallParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_reinstall", { params });
    }

    packagePlanBulk(params: PackagePlanBulkParams): Promise<PackagePlan> {
        return invokeTyped("gui_package_plan_bulk", { params });
    }

    packageApplyPlan(params: Omit<PackageApplyPlanParams, "idempotencyKey">): Promise<{ operationId: string; replayed: boolean }> {
        return invokeTyped("gui_package_apply_plan", { params: { ...params, idempotencyKey: crypto.randomUUID() } });
    }

    userPackagesList(cursor?: UserPackageCursor): Promise<UserPackagesListResult> {
        return invokeTyped("gui_user_packages_list", { params: { cursor, limit: 100 } });
    }

    userPackageGet(userPackageId: string): Promise<UserPackageResult> {
        return invokeTyped("gui_user_package_get", { userPackageId });
    }

    userPackageEnroll(sourcePath: string): Promise<UserPackageWriteResult> {
        return invokeTyped("gui_user_package_enroll", { sourcePath, idempotencyKey: crypto.randomUUID() });
    }

    userPackageRefresh(userPackageId: string, expectedRevision: number): Promise<UserPackageWriteResult> {
        return invokeTyped("gui_user_package_refresh", { userPackageId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    userPackageRemove(userPackageId: string, expectedRevision: number): Promise<UserPackageRemoveResult> {
        return invokeTyped("gui_user_package_remove", { userPackageId, expectedRevision, idempotencyKey: crypto.randomUUID() });
    }

    unityInstallationsList(cursor?: string, includeVersionOptions?: boolean): Promise<UnityInstallationsListResult> {
        return invokeTyped("gui_unity_installations_list", { params: { cursor, limit: 100, includeVersionOptions } });
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

    unityProjectLaunchConfigGet(projectId: string): Promise<ProjectUnityLaunchConfigResult> {
        return invokeTyped("gui_unity_project_launch_config_get", { projectId });
    }

    unityProjectLaunchConfigSet(projectId: string, arguments_: string[], expectedRevision: number): Promise<ProjectUnityLaunchConfigMutationResult> {
        return invokeTyped("gui_unity_project_launch_config_set", { params: { projectId, arguments: arguments_, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    unityProjectLaunchConfigClear(projectId: string, expectedRevision: number): Promise<ProjectUnityLaunchConfigMutationResult> {
        return invokeTyped("gui_unity_project_launch_config_clear", { params: { projectId, expectedRevision, idempotencyKey: crypto.randomUUID() } });
    }

    unityWriterState(projectId: string): Promise<UnityWriterState> {
        return invokeTyped("gui_unity_writer_state", { projectId });
    }

    async unityLaunchOptions(projectId: string, expectedProjectRevision: number, includeMigrationVersionOptions?: boolean): Promise<UnityLaunchOptionsResult> {
        const result = await invokeTyped<UnityLaunchOptionsResult>("gui_unity_launch_options", { params: { projectId, expectedProjectRevision, includeMigrationVersionOptions } });
        if (includeMigrationVersionOptions && result.migrationVersionOptions === undefined) throw new Error("Missing Unity migration version options");
        return result;
    }

    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number): Promise<UnityLaunchResult> {
        return invokeTyped("gui_unity_launch", { params: { projectId, installationId, expectedProjectRevision, idempotencyKey: crypto.randomUUID() } });
    }

    projectPlanUnityMigration(projectId: string, targetInstallationId: string, expectedProjectRevision: number): Promise<ProjectsPlanUnityMigrationResult> {
        return invokeTyped("gui_project_plan_unity_migration", { params: { projectId, targetInstallationId, expectedProjectRevision, idempotencyKey: crypto.randomUUID() } });
    }

    projectApplyUnityMigration(planId: string): Promise<ProjectsApplyUnityMigrationResult> {
        return invokeTyped("gui_project_apply_unity_migration", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    projectsCreate(params: ProjectsCreateParams): Promise<ProjectsCreateResult> {
        return invokeTyped("gui_projects_create", { params });
    }

    projectTemplatesList(cursor?: string): Promise<ProjectTemplatesListResult> {
        return invokeTyped("gui_project_templates_list", { params: { limit: 100, cursor } });
    }

    projectTemplatesSave(params: ProjectTemplatesSaveParams): Promise<ProjectTemplatesSaveResult> {
        return invokeTyped("gui_project_templates_save", { params });
    }

    projectTemplatesExport(params: ProjectTemplatesExportParams): Promise<ProjectTemplatesExportResult> {
        return invokeTyped("gui_project_templates_export", { params });
    }

    projectTemplatesRemove(params: ProjectTemplatesRemoveParams): Promise<ProjectTemplatesRemoveResult> {
        return invokeTyped("gui_project_templates_remove", { params });
    }

    backupsList(projectId?: string, cursor?: string): Promise<BackupsListResult> {
        return invokeTyped("gui_backups_list", { params: { projectId, cursor, limit: 100 } });
    }

    backupGet(backupId: string): Promise<BackupRecord> {
        return invokeTyped("gui_backup_get", { backupId });
    }

    backupCreate(projectId: string, expectedRevision: number, compressionMode?: "store" | "fast" | "level3" | "level5" | "level7" | "maximum", excludeVpmPackages?: boolean, includeLibrary?: boolean): Promise<BackupCreateResult> {
        return invokeTyped("gui_backup_create", { params: { projectId, expectedRevision,
            ...(compressionMode === undefined ? {} : { compressionMode }),
            ...(excludeVpmPackages === undefined ? {} : { excludeVpmPackages }),
            ...(includeLibrary === undefined ? {} : { includeLibrary }),
            idempotencyKey: crypto.randomUUID() } });
    }

    backupPlanRestore(backupId: string, targetParent: string, targetLeaf: string): Promise<BackupRestorePlan> {
        return invokeTyped("gui_backup_plan_restore", { params: { backupId, targetParent, targetLeaf } });
    }

    backupApplyRestore(planId: string): Promise<BackupApplyRestoreResult> {
        return invokeTyped("gui_backup_apply_restore", { params: { planId, idempotencyKey: crypto.randomUUID() } });
    }

    extensionsList(cursor?: string): Promise<ExtensionsListResult> {
        return invokeTyped("gui_extensions_list", { params: { limit: 100, ...(cursor === undefined ? {} : { cursor }) } });
    }

    extensionGet(extensionId: string): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_get", { extensionId });
    }

    extensionPermissionsList(extensionId: string, cursor?: string): Promise<ExtensionPermissionsListResult> {
        return invokeTyped("gui_extension_permissions_list", { params: { extensionId, limit: 100, ...(cursor === undefined ? {} : { cursor }) } });
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
