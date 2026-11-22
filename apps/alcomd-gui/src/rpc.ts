import type { BackupCreateParams, BackupRestoreParams } from "@alcomd/sdk";
import type { ProjectsCreateParams, ProjectsCreateResult, ProjectTemplatesExportParams, ProjectTemplatesExportResult, ProjectTemplatesListResult, ProjectTemplatesSaveParams, ProjectTemplatesSaveResult, ProjectTemplatesRemoveParams, ProjectTemplatesRemoveResult } from "@alcomd/sdk";
import type { LogsReadParams, LogsReadResult } from "@alcomd/sdk";
import type { ProjectInventoryParams, ProjectInventoryResult, ProjectTemplateEditParams, ProjectTemplateGetParams, ProjectTemplateGetResult } from "@alcomd/sdk";
import type { ProjectCandidateRequest, ProjectCandidatePage, ExtensionResult, ExtensionPermissionsListResult, ExtensionUiCloseParams, ExtensionUiCloseResult, ExtensionUiDispatchParams, ExtensionUiDispatchResult, ExtensionUiOpenParams, ExtensionUiOpenResult, ExtensionUiRefreshParams, ExtensionUiSnapshotResult, RpcError } from "@alcomd/sdk";
import type { UnityHubOpenResult } from "@alcomd/sdk";
import type { RepositoryCredentials, RepositoryListCancelParams, RepositoryListCancelResult, RepositoryListExportResult, RepositoryListImportParams, RepositoryListImportResult, RepositoryListInspection, RepositoryListPreview, RepositoryRefreshAllParams, RepositoryRefreshAllResult, RepositoryMoveParams, RepositoryMoveResult, RepositorySetDisplayNameParams } from "@alcomd/sdk";
import { Channel, invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { GuiEventsRequest, GuiEventMessage, GuiEventSubscription } from "./event-types";
import type { GuiUnityWindows, UnityWindowSnapshot } from "./unity-window-observation";
import type { ProjectListView } from "./core-models";

import type { ExtensionGrantResult, ExtensionOperationResult, ExtensionPlanResult, ExtensionsListResult, Operation, OperationAccepted, OperationWriteResult, OperationsListResult, PackageApplyPlanParams, PackageCursor, PackagePlan, PackagePlanBulkParams, PackagePlanDowngradeParams, PackagePlanInstallParams, PackagePlanRemoveParams, PackagePlanResolveParams, PackagePlanReinstallParams, ProjectUnityLaunchConfigMutationResult, ProjectUnityLaunchConfigResult, ProjectResult, ProjectsApplyCopyResult, ProjectsApplyDeleteDirectoryResult, ProjectsPlanCopyResult, ProjectsPlanDeleteDirectoryResult, ProjectsApplyUnityMigrationResult, ProjectsPlanUnityMigrationResult, ProjectUnregisterResult, ProjectWriteResult, ProjectsListResult, RegistryCursor, RepositoriesListResult, RepositoryCursor, RepositoryPackagesResult, RepositoryResult, RepositorySource, RepositoryUnregisterResult, RepositoryWriteResult, SettingsGetResult, SettingsUpdate, SystemStatus, UnityInstallationResult, UnityInstallationRemoveResult, UnityInstallationsListResult, UnityLaunchResult, UnityLaunchOptionsResult, UnityWriterState, UserPackageCursor, UserPackageRemoveResult, UserPackageResult, UserPackagesListResult, UserPackageWriteResult } from "./core-models";

export type FileDrag = { type: "leave" } | { type: "hover"; x: number; y: number } | { type: "drop"; x: number; y: number; paths: string[] };
export type NativeFileKind = "backupZip" | "backupZipDestination" | "repositoryManifest" | "unityEditor" | "extensionPackage" | "templateAttachment";

export interface GuiRpcClient {
    readonly unityWindows?: GuiUnityWindows;
    subscribeEvents(request: GuiEventsRequest, receive: (message: GuiEventMessage) => void): Promise<GuiEventSubscription>;
    packageQueryProjectCandidates(params: ProjectCandidateRequest): Promise<ProjectCandidatePage>;
    systemStatus(): Promise<SystemStatus>;
    stateCheck(idempotencyKey?: string): Promise<OperationAccepted>;
    operationsList(cursor?: OperationsListResult["nextCursor"], activeOnly?: boolean): Promise<OperationsListResult>;
    logsRead(params: LogsReadParams): Promise<LogsReadResult>;
    operationGet(operationId: string): Promise<Operation>;
    operationCancel(operationId: string, expectedRevision: number, idempotencyKey: string): Promise<OperationWriteResult>;
    settingsGet(): Promise<SettingsGetResult>;
    settingsUpdate(expectedRevision: number, update: SettingsUpdate): Promise<SettingsGetResult>;
    projectsInspect(path: string, discoveryMode: "exact-root" | "search-parents"): Promise<ProjectResult>;
    projectsList(cursor?: RegistryCursor, view?: ProjectListView, expectedViewSequence?: number): Promise<ProjectsListResult>;
    projectGet(projectId: string): Promise<ProjectResult>;
    projectInventory(params: ProjectInventoryParams): Promise<ProjectInventoryResult>;
    openProjectDirectory(projectId: string): Promise<void>;
    selectDirectory(currentPath?: string): Promise<string | undefined>;
    selectFile(kind: NativeFileKind, currentPath?: string): Promise<string | undefined>;
    subscribeFileDrag?(receive: (event: FileDrag) => void): Promise<() => void>;
    selectProjectTemplateFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined>;
    selectUnityHub(currentPath: string | null): Promise<string | undefined>;
    unityHubOpen(expectedRevision: number, expectedTargetToken: string | null): Promise<UnityHubOpenResult>;
    openUnityHubDownload(): Promise<void>;
    projectRegister(path: string, idempotencyKey: string): Promise<ProjectWriteResult>;
    projectRefresh(projectId: string, expectedRevision: number, idempotencyKey?: string): Promise<ProjectWriteResult>;
    projectSetFavorite(projectId: string, favorite: boolean, expectedRevision: number, idempotencyKey: string): Promise<ProjectWriteResult>;
    projectUnregister(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectUnregisterResult>;
    projectPlanCopy(sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string, idempotencyKey: string): Promise<ProjectsPlanCopyResult>;
    projectApplyCopy(planId: string, expectedRevision: number, progress?: "inline"): Promise<ProjectsApplyCopyResult>;
    projectPlanDeleteDirectory(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectsPlanDeleteDirectoryResult>;
    projectApplyDeleteDirectory(planId: string, expectedRevision: number): Promise<ProjectsApplyDeleteDirectoryResult>;
    repositoriesInspect(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryResult>;
    selectRepositoryListFile(mode: "open" | "save", fileName?: string, currentPath?: string): Promise<string | undefined>;
    repositoryListInspectFile(path: string): Promise<RepositoryListInspection>;
    repositoryListImportFile(path: string, expectedFingerprint: string, idempotencyKey: string, overwriteExisting?: boolean): Promise<RepositoryListImportResult>;
    repositoryListCancelRegistration(idempotencyKey: string): Promise<RepositoryListCancelResult>;
    repositoryListPreviewExport(format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListPreview>;
    repositoryListExportFile(targetPath: string, expectedFingerprint: string, format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListExportResult>;
    repositoriesList(cursor?: RepositoryCursor): Promise<RepositoriesListResult>;
    repositoryGet(repositoryId: string): Promise<RepositoryResult>;
    repositoryPackages(repositoryId: string, cursor?: PackageCursor): Promise<RepositoryPackagesResult>;
    openPackageLink(url: string): Promise<void>;
    repositoryRegister(source: RepositorySource, credentials: RepositoryCredentials | undefined, idempotencyKey: string): Promise<RepositoryWriteResult>;
    repositorySetCredentials(repositoryId: string, expectedRevision: number, credentials: RepositoryCredentials | null, idempotencyKey: string): Promise<RepositoryWriteResult>;
    repositoryMove(repositoryId: string, beforeRepositoryId: string | null, expectedOrderFingerprint: string, idempotencyKey: string): Promise<RepositoryMoveResult>;
    repositorySetDisplayName(repositoryId: string, expectedRevision: number, displayName: string | null, idempotencyKey: string): Promise<RepositoryWriteResult>;
    repositoryRefresh(repositoryId: string, expectedRevision: number, idempotencyKey: string): Promise<RepositoryWriteResult>;
    repositoriesRefreshAll(idempotencyKey: string): Promise<RepositoryRefreshAllResult>;
    repositoryUnregister(repositoryId: string, expectedRevision: number, idempotencyKey: string): Promise<RepositoryUnregisterResult>;
    packagePlanInstall(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanRemove(params: PackagePlanRemoveParams): Promise<PackagePlan>;
    packagePlanUpgrade(params: PackagePlanInstallParams): Promise<PackagePlan>;
    packagePlanDowngrade(params: PackagePlanDowngradeParams): Promise<PackagePlan>;
    packagePlanResolve(params: PackagePlanResolveParams): Promise<PackagePlan>;
    packagePlanReinstall(params: PackagePlanReinstallParams): Promise<PackagePlan>;
    packagePlanBulk(params: PackagePlanBulkParams): Promise<PackagePlan>;
    packageApplyPlan(params: PackageApplyPlanParams): Promise<{ operationId: string; replayed: boolean }>;
    userPackagesList(cursor?: UserPackageCursor): Promise<UserPackagesListResult>;
    userPackageGet(userPackageId: string): Promise<UserPackageResult>;
    userPackageEnroll(sourcePath: string, idempotencyKey: string): Promise<UserPackageWriteResult>;
    userPackageRefresh(userPackageId: string, expectedRevision: number, idempotencyKey: string): Promise<UserPackageWriteResult>;
    userPackageRemove(userPackageId: string, expectedRevision: number, idempotencyKey: string): Promise<UserPackageRemoveResult>;
    unityInstallationsList(cursor?: string, includeVersionOptions?: boolean): Promise<UnityInstallationsListResult>;
    unityInstallationGet(installationId: string): Promise<UnityInstallationResult>;
    unityInstallationRegister(executablePath: string, idempotencyKey: string): Promise<UnityInstallationResult>;
    unityInstallationRemove(installationId: string, expectedRevision: number, idempotencyKey: string): Promise<UnityInstallationRemoveResult>;
    unityInstallationsRefresh(idempotencyKey: string): Promise<UnityInstallationsListResult>;
    unityProjectLaunchConfigGet(projectId: string): Promise<ProjectUnityLaunchConfigResult>;
    unityProjectLaunchConfigSet(projectId: string, arguments_: string[], expectedRevision: number, idempotencyKey: string): Promise<ProjectUnityLaunchConfigMutationResult>;
    unityProjectLaunchConfigClear(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectUnityLaunchConfigMutationResult>;
    unityWriterState(projectId: string): Promise<UnityWriterState>;
    unityLaunchOptions(projectId: string, expectedProjectRevision: number, includeMigrationVersionOptions?: boolean): Promise<UnityLaunchOptionsResult>;
    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number, idempotencyKey: string): Promise<UnityLaunchResult>;
    projectPlanUnityMigration(projectId: string, targetInstallationId: string, expectedProjectRevision: number, idempotencyKey: string): Promise<ProjectsPlanUnityMigrationResult>;
    projectApplyUnityMigration(planId: string, idempotencyKey: string, progress?: "inline"): Promise<ProjectsApplyUnityMigrationResult>;
    projectsCreate(params: ProjectsCreateParams): Promise<ProjectsCreateResult>;
    projectTemplatesList(cursor?: string): Promise<ProjectTemplatesListResult>;
    projectTemplateGet(params: ProjectTemplateGetParams): Promise<ProjectTemplateGetResult>;
    projectTemplateEdit(params: ProjectTemplateEditParams): Promise<ProjectTemplatesSaveResult>;
    projectTemplatesSave(params: ProjectTemplatesSaveParams): Promise<ProjectTemplatesSaveResult>;
    projectTemplatesExport(params: ProjectTemplatesExportParams): Promise<ProjectTemplatesExportResult>;
    projectTemplatesRemove(params: ProjectTemplatesRemoveParams): Promise<ProjectTemplatesRemoveResult>;
    backupCreate(params: BackupCreateParams): Promise<OperationAccepted>;
    backupRestore(params: BackupRestoreParams): Promise<OperationAccepted>;
    extensionsList(cursor?: string): Promise<ExtensionsListResult>;
    extensionGet(extensionId: string): Promise<ExtensionResult>;
    extensionPermissionsList(extensionId: string, cursor?: string): Promise<ExtensionPermissionsListResult>;
    extensionInspectSource(url: string): Promise<import("@alcomd/sdk").ExtensionSourcePreview>;
    extensionPlanInstallFromSource(url: string, version: string, expectedSourceFingerprint: string): Promise<ExtensionPlanResult>;
    extensionPlanInstall(packagePath: string, expectedRevision: number): Promise<ExtensionPlanResult>;
    extensionApplyInstall(planId: string, consent: import("@alcomd/sdk").ExtensionInstallConsent, idempotencyKey: string): Promise<ExtensionOperationResult>;
    extensionEnable(extensionId: string, expectedRevision: number, activationPackageDigest: string | undefined, idempotencyKey: string): Promise<ExtensionResult>;
    extensionDisable(extensionId: string, expectedRevision: number, idempotencyKey: string): Promise<ExtensionResult>;
    extensionPlanUninstall(extensionId: string, expectedRevision: number, dataDisposition: "retain_data" | "delete_data"): Promise<ExtensionPlanResult>;
    extensionApplyUninstall(planId: string, idempotencyKey: string): Promise<ExtensionOperationResult>;
    extensionSetGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number, idempotencyKey: string): Promise<ExtensionGrantResult>;
    extensionRevokeGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number, idempotencyKey: string): Promise<ExtensionGrantResult>;
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
        return invokeTyped("gui_repository_list_preview_export", { format });
    }
    repositoryListExportFile(targetPath: string, expectedFingerprint: string, format?: import("@alcomd/sdk").RepositoryListFormat): Promise<RepositoryListExportResult> {
        return invokeTyped("gui_repository_list_export_file", { targetPath, expectedFingerprint, format });
    }
    packageQueryProjectCandidates(params: ProjectCandidateRequest): Promise<ProjectCandidatePage> {
        return invokeTyped("gui_package_query_project_candidates", { params });
    }
    systemStatus(): Promise<SystemStatus> {
        return invokeTyped("gui_system_status", {});
    }

    stateCheck(idempotencyKey: string = crypto.randomUUID()): Promise<OperationAccepted> {
        return invokeTyped("gui_state_check", { idempotencyKey });
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

    operationCancel(operationId: string, expectedRevision: number, idempotencyKey: string): Promise<OperationWriteResult> {
        return invokeTyped("gui_operation_cancel", { operationId, expectedRevision, idempotencyKey });
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

    unityHubOpen(expectedRevision: number, expectedTargetToken: string | null): Promise<UnityHubOpenResult> {
        return invokeTyped("gui_unity_hub_open", { params: { expectedRevision, expectedTargetToken } });
    }

    openUnityHubDownload(): Promise<void> {
        return invokeTyped("gui_open_unity_hub_download", {});
    }

    projectRegister(path: string, idempotencyKey: string): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_register", { path, idempotencyKey });
    }

    projectRefresh(projectId: string, expectedRevision: number, idempotencyKey: string = crypto.randomUUID()): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_refresh", { projectId, expectedRevision, idempotencyKey });
    }

    projectSetFavorite(projectId: string, favorite: boolean, expectedRevision: number, idempotencyKey: string): Promise<ProjectWriteResult> {
        return invokeTyped("gui_project_set_favorite", { params: { projectId, favorite, expectedRevision, idempotencyKey } });
    }

    projectUnregister(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectUnregisterResult> {
        return invokeTyped("gui_project_unregister", { projectId, expectedRevision, idempotencyKey });
    }

    projectPlanCopy(sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string, idempotencyKey: string): Promise<ProjectsPlanCopyResult> {
        return invokeTyped("gui_project_plan_copy", { params: { sourceProjectId, expectedRevision, targetParentPath, targetLeaf, idempotencyKey } });
    }

    projectApplyCopy(planId: string, expectedRevision: number): Promise<ProjectsApplyCopyResult> {
        return invokeTyped("gui_project_apply_copy", { params: { planId, expectedRevision, idempotencyKey: planId } });
    }

    projectPlanDeleteDirectory(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectsPlanDeleteDirectoryResult> {
        return invokeTyped("gui_project_plan_delete_directory", { params: { projectId, expectedRevision, idempotencyKey } });
    }

    projectApplyDeleteDirectory(planId: string, expectedRevision: number): Promise<ProjectsApplyDeleteDirectoryResult> {
        return invokeTyped("gui_project_apply_delete_directory", { params: { planId, expectedRevision, idempotencyKey: planId } });
    }

    repositoriesInspect(source: RepositorySource, credentials?: RepositoryCredentials): Promise<RepositoryResult> {
        return invokeTyped("gui_repositories_inspect", { params: { source, credentials } });
    }

    repositoriesList(cursor?: RepositoryCursor): Promise<RepositoriesListResult> {
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

    repositoryRegister(source: RepositorySource, credentials: RepositoryCredentials | undefined, idempotencyKey: string): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_register", { source, credentials, idempotencyKey });
    }

    repositorySetCredentials(repositoryId: string, expectedRevision: number, credentials: RepositoryCredentials | null, idempotencyKey: string): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_set_credentials", { params: { repositoryId, expectedRevision, credentials, idempotencyKey } });
    }

    repositoryMove(repositoryId: string, beforeRepositoryId: string | null, expectedOrderFingerprint: string, idempotencyKey: string): Promise<RepositoryMoveResult> {
        const params: RepositoryMoveParams = { repositoryId, beforeRepositoryId, expectedOrderFingerprint, idempotencyKey };
        return invokeTyped("gui_repository_move", { params });
    }

    repositorySetDisplayName(repositoryId: string, expectedRevision: number, displayName: string | null, idempotencyKey: string): Promise<RepositoryWriteResult> {
        const params: RepositorySetDisplayNameParams = { repositoryId, expectedRevision, displayName, idempotencyKey };
        return invokeTyped("gui_repository_set_display_name", { params });
    }

    repositoryRefresh(repositoryId: string, expectedRevision: number, idempotencyKey: string): Promise<RepositoryWriteResult> {
        return invokeTyped("gui_repository_refresh", { repositoryId, expectedRevision, idempotencyKey });
    }

    repositoriesRefreshAll(idempotencyKey: string): Promise<RepositoryRefreshAllResult> {
        const params: RepositoryRefreshAllParams = { idempotencyKey };
        return invokeTyped("gui_repositories_refresh_all", { params });
    }

    repositoryUnregister(repositoryId: string, expectedRevision: number, idempotencyKey: string): Promise<RepositoryUnregisterResult> {
        return invokeTyped("gui_repository_unregister", { repositoryId, expectedRevision, idempotencyKey });
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

    packageApplyPlan(params: PackageApplyPlanParams): Promise<{ operationId: string; replayed: boolean }> {
        return invokeTyped("gui_package_apply_plan", { params });
    }

    userPackagesList(cursor?: UserPackageCursor): Promise<UserPackagesListResult> {
        return invokeTyped("gui_user_packages_list", { params: { cursor, limit: 100 } });
    }

    userPackageGet(userPackageId: string): Promise<UserPackageResult> {
        return invokeTyped("gui_user_package_get", { userPackageId });
    }

    userPackageEnroll(sourcePath: string, idempotencyKey: string): Promise<UserPackageWriteResult> {
        return invokeTyped("gui_user_package_enroll", { sourcePath, idempotencyKey });
    }

    userPackageRefresh(userPackageId: string, expectedRevision: number, idempotencyKey: string): Promise<UserPackageWriteResult> {
        return invokeTyped("gui_user_package_refresh", { userPackageId, expectedRevision, idempotencyKey });
    }

    userPackageRemove(userPackageId: string, expectedRevision: number, idempotencyKey: string): Promise<UserPackageRemoveResult> {
        return invokeTyped("gui_user_package_remove", { userPackageId, expectedRevision, idempotencyKey });
    }

    unityInstallationsList(cursor?: string, includeVersionOptions?: boolean): Promise<UnityInstallationsListResult> {
        return invokeTyped("gui_unity_installations_list", { params: { cursor, limit: 100, includeVersionOptions } });
    }

    unityInstallationGet(installationId: string): Promise<UnityInstallationResult> {
        return invokeTyped("gui_unity_installation_get", { installationId });
    }

    unityInstallationRegister(executablePath: string, idempotencyKey: string): Promise<UnityInstallationResult> {
        return invokeTyped("gui_unity_installation_register", { executablePath, idempotencyKey });
    }

    unityInstallationRemove(installationId: string, expectedRevision: number, idempotencyKey: string): Promise<UnityInstallationRemoveResult> {
        return invokeTyped("gui_unity_installation_remove", { installationId, expectedRevision, idempotencyKey });
    }

    unityInstallationsRefresh(idempotencyKey: string): Promise<UnityInstallationsListResult> {
        return invokeTyped("gui_unity_installations_refresh", { idempotencyKey });
    }

    unityProjectLaunchConfigGet(projectId: string): Promise<ProjectUnityLaunchConfigResult> {
        return invokeTyped("gui_unity_project_launch_config_get", { projectId });
    }

    unityProjectLaunchConfigSet(projectId: string, arguments_: string[], expectedRevision: number, idempotencyKey: string): Promise<ProjectUnityLaunchConfigMutationResult> {
        return invokeTyped("gui_unity_project_launch_config_set", { params: { projectId, arguments: arguments_, expectedRevision, idempotencyKey } });
    }

    unityProjectLaunchConfigClear(projectId: string, expectedRevision: number, idempotencyKey: string): Promise<ProjectUnityLaunchConfigMutationResult> {
        return invokeTyped("gui_unity_project_launch_config_clear", { params: { projectId, expectedRevision, idempotencyKey } });
    }

    unityWriterState(projectId: string): Promise<UnityWriterState> {
        return invokeTyped("gui_unity_writer_state", { projectId });
    }

    async unityLaunchOptions(projectId: string, expectedProjectRevision: number, includeMigrationVersionOptions?: boolean): Promise<UnityLaunchOptionsResult> {
        const result = await invokeTyped<UnityLaunchOptionsResult>("gui_unity_launch_options", { params: { projectId, expectedProjectRevision, includeMigrationVersionOptions } });
        if (includeMigrationVersionOptions && result.migrationVersionOptions === undefined) throw new Error("Missing Unity migration version options");
        return result;
    }

    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number, idempotencyKey: string): Promise<UnityLaunchResult> {
        return invokeTyped("gui_unity_launch", { params: { projectId, installationId, expectedProjectRevision, idempotencyKey } });
    }

    projectPlanUnityMigration(projectId: string, targetInstallationId: string, expectedProjectRevision: number, idempotencyKey: string): Promise<ProjectsPlanUnityMigrationResult> {
        return invokeTyped("gui_project_plan_unity_migration", { params: { projectId, targetInstallationId, expectedProjectRevision, idempotencyKey } });
    }

    projectApplyUnityMigration(planId: string, idempotencyKey: string): Promise<ProjectsApplyUnityMigrationResult> {
        return invokeTyped("gui_project_apply_unity_migration", { params: { planId, idempotencyKey } });
    }

    projectsCreate(params: ProjectsCreateParams): Promise<ProjectsCreateResult> {
        return invokeTyped("gui_projects_create", { params });
    }

    projectTemplatesList(cursor?: string): Promise<ProjectTemplatesListResult> {
        return invokeTyped("gui_project_templates_list", { params: { limit: 100, cursor } });
    }

    projectInventory(params: ProjectInventoryParams): Promise<ProjectInventoryResult> {
        return invokeTyped("gui_project_inventory", { params });
    }

    projectTemplateGet(params: ProjectTemplateGetParams): Promise<ProjectTemplateGetResult> {
        return invokeTyped("gui_project_template_get", { params });
    }

    projectTemplateEdit(params: ProjectTemplateEditParams): Promise<ProjectTemplatesSaveResult> {
        return invokeTyped("gui_project_template_edit", { params });
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

    backupCreate(params: BackupCreateParams): Promise<OperationAccepted> {
        return invokeTyped("gui_backup_create", { params });
    }

    backupRestore(params: BackupRestoreParams): Promise<OperationAccepted> {
        return invokeTyped("gui_backup_restore", { params });
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

    extensionInspectSource(url: string): Promise<import("@alcomd/sdk").ExtensionSourcePreview> {
        return invokeTyped("gui_extension_inspect_source", { params: { url } });
    }

    extensionPlanInstallFromSource(url: string, version: string, expectedSourceFingerprint: string): Promise<ExtensionPlanResult> {
        return invokeTyped("gui_extension_plan_install_from_source", { params: { url, version, expectedSourceFingerprint } });
    }

    extensionPlanInstall(packagePath: string, expectedRevision: number): Promise<ExtensionPlanResult> {
        return invokeTyped("gui_extension_plan_install", { params: { sourceKind: "local_owner_selected", packagePath, expectedRevision } });
    }

    extensionApplyInstall(planId: string, consent: import("@alcomd/sdk").ExtensionInstallConsent, idempotencyKey: string): Promise<ExtensionOperationResult> {
        return invokeTyped("gui_extension_apply_install", { params: { planId, consent, idempotencyKey } });
    }

    extensionEnable(extensionId: string, expectedRevision: number, activationPackageDigest: string | undefined, idempotencyKey: string): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_enable", { params: { extensionId, expectedRevision, activationPackageDigest, idempotencyKey } });
    }

    extensionDisable(extensionId: string, expectedRevision: number, idempotencyKey: string): Promise<ExtensionResult> {
        return invokeTyped("gui_extension_disable", { params: { extensionId, expectedRevision, idempotencyKey } });
    }

    extensionPlanUninstall(extensionId: string, expectedRevision: number, dataDisposition: "retain_data" | "delete_data"): Promise<ExtensionPlanResult> {
        return invokeTyped("gui_extension_plan_uninstall", { params: { extensionId, expectedRevision, dataDisposition } });
    }

    extensionApplyUninstall(planId: string, idempotencyKey: string): Promise<ExtensionOperationResult> {
        return invokeTyped("gui_extension_apply_uninstall", { params: { planId, idempotencyKey } });
    }

    extensionSetGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number, idempotencyKey: string): Promise<ExtensionGrantResult> {
        return invokeTyped("gui_extension_set_grant", { params: { extensionId, permission, resourceKind, resourceId, expectedGrantRevision, idempotencyKey } });
    }

    extensionRevokeGrant(extensionId: string, permission: string, resourceKind: string, resourceId: string, expectedGrantRevision: number, idempotencyKey: string): Promise<ExtensionGrantResult> {
        return invokeTyped("gui_extension_revoke_grant", { params: { extensionId, permission, resourceKind, resourceId, expectedGrantRevision, idempotencyKey } });
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
    const request = isRecord(args.params) ? args.params : args;
    try {
        return await invoke<Result>(command, args);
    } catch (error: unknown) {
        const failure = normalizeRpcError(error);
        const replayable = typeof request.idempotencyKey === "string"
            || (command === "gui_extension_ui_dispatch" && typeof request.requestId === "string");
        if (failure.code !== "daemon_unavailable" || !replayable) {
            throw failure;
        }
        try {
            return await invoke<Result>(command, args);
        } catch (retryError: unknown) {
            throw normalizeRpcError(retryError);
        }
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
