import type * as Types from "./index.js";

/** Execution semantics for supported native RPC methods. */
export const SDK_METHODS = {
    "backups.create": "write",
    "backups.restore": "write",
    "events.list": "read",
    "events.subscribe": "stream",
    "extensions.applyInstall": "write",
    "extensions.applyUninstall": "write",
    "extensions.disable": "write",
    "extensions.enable": "write",
    "extensions.get": "read",
    "extensions.inspectSource": "read",
    "extensions.list": "read",
    "extensions.permissions.list": "read",
    "extensions.planInstall": "write",
    "extensions.planInstallFromSource": "write",
    "extensions.planUninstall": "write",
    "extensions.revokeGrant": "write",
    "extensions.setGrant": "write",
    "extensions.ui.close": "write",
    "extensions.ui.dispatch": "write",
    "extensions.ui.open": "write",
    "extensions.ui.refresh": "write",
    "logs.read": "read",
    "operations.cancel": "write",
    "operations.get": "read",
    "operations.list": "read",
    "packages.applyPlan": "write",
    "packages.planBulk": "write",
    "packages.planDowngrade": "write",
    "packages.planInstall": "write",
    "packages.planReinstall": "write",
    "packages.planRemove": "write",
    "packages.planResolve": "write",
    "packages.planUpgrade": "write",
    "packages.projectInventory": "read",
    "packages.queryProjectCandidates": "read",
    "packages.userPackages.enroll": "write",
    "packages.userPackages.get": "read",
    "packages.userPackages.list": "read",
    "packages.userPackages.refresh": "write",
    "packages.userPackages.remove": "write",
    "plans.get": "read",
    "projectTemplates.edit": "write",
    "projectTemplates.export": "write",
    "projectTemplates.get": "read",
    "projectTemplates.list": "read",
    "projectTemplates.remove": "write",
    "projectTemplates.save": "write",
    "projects.applyCopy": "write",
    "projects.applyDeleteDirectory": "write",
    "projects.applyUnityMigration": "write",
    "projects.create": "write",
    "projects.get": "read",
    "projects.inspect": "read",
    "projects.list": "read",
    "projects.planCopy": "write",
    "projects.planDeleteDirectory": "write",
    "projects.planUnityMigration": "write",
    "projects.refresh": "write",
    "projects.register": "write",
    "projects.setFavorite": "write",
    "projects.unregister": "write",
    "repositories.cancelListRegistration": "write",
    "repositories.exportListFile": "write",
    "repositories.get": "read",
    "repositories.importListFile": "write",
    "repositories.inspect": "read",
    "repositories.inspectListFile": "read",
    "repositories.list": "read",
    "repositories.move": "write",
    "repositories.packages": "read",
    "repositories.previewListExport": "read",
    "repositories.refresh": "write",
    "repositories.refreshAll": "write",
    "repositories.register": "write",
    "repositories.setCredentials": "write",
    "repositories.setDisplayName": "write",
    "repositories.unregister": "write",
    "settings.get": "read",
    "settings.update": "write",
    "state.check": "write",
    "system.status": "read",
    "unity.hub.open": "write",
    "unity.installations.get": "read",
    "unity.installations.list": "read",
    "unity.installations.refresh": "write",
    "unity.installations.register": "write",
    "unity.installations.remove": "write",
    "unity.launch": "write",
    "unity.launchOptions": "read",
    "unity.projectLaunchConfig.clear": "write",
    "unity.projectLaunchConfig.get": "read",
    "unity.projectLaunchConfig.set": "write",
    "unity.writerState": "read",
} as const;

export type BackupCreateParams = {
    projectId: string;
    expectedRevision: number;
    compressionMode?: Types.BackupCompressionMode;
    excludeVpmPackages?: boolean;
    includeLibrary?: boolean;
    destinationPath?: string;
    idempotencyKey: string;
};
export type BackupRestoreParams = {
    archivePath: string;
    projectName?: string;
    destinationPath?: string;
    idempotencyKey: string;
};
export type EmptyParams = Record<string, never>;
export type EventsListParams = { afterSequence: number; limit?: number };
export type EventsListResult = { events: Types.CoreEvent[]; nextSequence: number };
export type ExtensionApplyParams = { planId: string; idempotencyKey: string };
export type ExtensionEnableParams = { extensionId: string; expectedRevision: number; idempotencyKey: string; activationPackageDigest?: string };
export type ExtensionGrantParams = {
    extensionId: string;
    permission: Types.ExtensionPermissionGrant["permission"];
    resourceKind: Types.ExtensionPermissionGrant["resourceKind"];
    resourceId: string;
    expectedGrantRevision: number;
    idempotencyKey: string;
};
export type ExtensionIdParams = { extensionId: string };
export type ExtensionInspectSourceParams = { url: string };
export type ExtensionInstallApplyParams = { planId: string; idempotencyKey: string; consent: Types.ExtensionInstallConsent };
export type ExtensionLifecycleParams = { extensionId: string; expectedRevision: number; idempotencyKey: string };
export type ExtensionPlanInstallFromSourceParams = { url: string; version: string; expectedSourceFingerprint: string };
export type ExtensionPlanInstallParams = { sourceKind: "local_owner_selected"; packagePath: string; expectedRevision: number };
export type ExtensionPlanUninstallParams = { extensionId: string; expectedRevision: number; dataDisposition: "retain_data" | "delete_data" };
export type ExtensionsListParams = { cursor?: string; limit?: number };
export type OperationsCancelParams = { operationId: string; expectedRevision: number; idempotencyKey: string };
export type OperationsGetParams = { operationId: string };
export type PackageApplyPlanResult = Types.OperationAccepted;
export type PackagePlanUpgradeParams = Types.PackagePlanInstallParams;
export type ProjectIdParams = { projectId: string };
export type ProjectMutationParams = { projectId: string; expectedRevision: number; idempotencyKey: string };
export type ProjectRegisterParams = { path: string; idempotencyKey: string };
export type ProjectSetFavoriteParams = { projectId: string; favorite: boolean; expectedRevision: number; idempotencyKey: string };
export type ProjectUnityLaunchConfigClearParams = { projectId: string; expectedRevision: number; idempotencyKey: string };
export type ProjectUnityLaunchConfigSetParams = { projectId: string; arguments: string[]; expectedRevision: number; idempotencyKey: string };
export type ProjectsApplyCopyParams = { planId: string; expectedRevision: number; idempotencyKey: string };
export type ProjectsApplyDeleteDirectoryParams = { planId: string; expectedRevision: number; idempotencyKey: string };
export type ProjectsApplyUnityMigrationParams = { planId: string; idempotencyKey: string };
export type ProjectsInspectParams = { path: string; discoveryMode: "exact-root" | "search-parents" };
export type ProjectsListParams = { cursor?: Types.RegistryCursor; limit?: number; view?: Types.ProjectListView; expectedViewSequence?: number };
export type ProjectsPlanCopyParams = { sourceProjectId: string; expectedRevision: number; targetParentPath: string; targetLeaf: string; idempotencyKey: string };
export type ProjectsPlanDeleteDirectoryParams = { projectId: string; expectedRevision: number; idempotencyKey: string };
export type ProjectsPlanUnityMigrationParams = { projectId: string; targetInstallationId: string; expectedProjectRevision: number; idempotencyKey: string };
export type RegistryListParams = { cursor?: Types.RepositoryCursor; limit?: number };
export type RepositoryIdParams = { repositoryId: string };
export type RepositoryInspectParams = { source: Types.RepositorySource; credentials?: Types.RepositoryCredentials };
export type RepositoryListExportParams = { format?: Types.RepositoryListFormat; targetPath: string; expectedFingerprint: string };
export type RepositoryListInspectParams = { path: string };
export type RepositoryListPreviewParams = { format?: Types.RepositoryListFormat };
export type RepositoryMutationParams = { repositoryId: string; expectedRevision: number; idempotencyKey: string };
export type RepositoryPackagesParams = { repositoryId: string; cursor?: Types.PackageCursor; limit?: number };
export type RepositoryRegisterParams = { source: Types.RepositorySource; idempotencyKey: string; credentials?: Types.RepositoryCredentials };
export type RepositorySetCredentialsParams = { repositoryId: string; expectedRevision: number; idempotencyKey: string; credentials: Types.RepositoryCredentials | null };
export type SettingsUpdateParams = { expectedRevision: number; update: Types.SettingsUpdate };
export type StateCheckParams = { idempotencyKey: string };
export type SystemStatusResult = Types.SystemStatus;
export type UnityInstallationIdParams = { installationId: string };
export type UnityInstallationRefreshParams = { idempotencyKey: string };
export type UnityInstallationRegisterParams = { executablePath: string; idempotencyKey: string };
export type UnityInstallationRemoveParams = { installationId: string; expectedRevision: number; idempotencyKey: string };
export type UnityInstallationsListParams = { cursor?: string; limit?: number; includeVersionOptions?: boolean };
export type UnityLaunchOptionsParams = { projectId: string; expectedProjectRevision: number; includeMigrationVersionOptions?: boolean };
export type UnityLaunchParams = { projectId: string; installationId: string; expectedProjectRevision: number; idempotencyKey: string };
export type UnityProjectIdParams = { projectId: string };
export type UserPackageEnrollParams = { sourcePath: string; idempotencyKey: string };
export type UserPackageGetParams = { userPackageId: string };
export type UserPackageMutationParams = { userPackageId: string; expectedRevision: number; idempotencyKey: string };
export type UserPackagesListParams = { cursor?: Types.UserPackageCursor; limit?: number };

type NativeProjectSnapshot = Omit<Types.ProjectSnapshot, "projectId" | "unityRevision" | "revision"> & {
    projectId: string | null;
    unityRevision: string | null;
    revision: number | null;
};
type NativeProjectResult = { project: NativeProjectSnapshot };
type NativeProjectWriteResult = NativeProjectResult & { replayed: boolean };
type NativeProjectsListResult = Omit<Types.ProjectsListResult, "projects"> & { projects: NativeProjectSnapshot[] };

type NativeRepositorySnapshot = Omit<Types.RepositorySnapshot, "repositoryId" | "declaredId" | "name" | "declaredUrl" | "revision"> & {
    repositoryId: string | null;
    declaredId: string | null;
    name: string | null;
    declaredUrl: string | null;
    revision: number | null;
};
type NativeRepositoryResult = { repository: NativeRepositorySnapshot };
type NativeRepositoryWriteResult = NativeRepositoryResult & { replayed: boolean };
type NativeRepositoriesListResult = Omit<Types.RepositoriesListResult, "repositories"> & { repositories: NativeRepositorySnapshot[] };

/** The wire DTOs accepted and returned by Core over native RPC v1. */
export interface NativeRpcMethods {
    "backups.create": { params: BackupCreateParams; result: Types.OperationAccepted };
    "backups.restore": { params: BackupRestoreParams; result: Types.OperationAccepted };
    "events.list": { params: EventsListParams; result: EventsListResult };
    "events.subscribe": { params: Types.EventsSubscribeParams; result: Types.EventsSubscribeResult };
    "extensions.applyInstall": { params: ExtensionInstallApplyParams; result: Types.ExtensionOperationResult };
    "extensions.applyUninstall": { params: ExtensionApplyParams; result: Types.ExtensionOperationResult };
    "extensions.disable": { params: ExtensionLifecycleParams; result: Types.ExtensionResult };
    "extensions.enable": { params: ExtensionEnableParams; result: Types.ExtensionResult };
    "extensions.get": { params: ExtensionIdParams; result: Types.ExtensionResult };
    "extensions.inspectSource": { params: ExtensionInspectSourceParams; result: Types.ExtensionSourcePreview };
    "extensions.list": { params: ExtensionsListParams; result: Types.ExtensionsListResult };
    "extensions.permissions.list": { params: Types.ExtensionPermissionsListParams; result: Types.ExtensionPermissionsListResult };
    "extensions.planInstall": { params: ExtensionPlanInstallParams; result: Types.ExtensionPlanResult };
    "extensions.planInstallFromSource": { params: ExtensionPlanInstallFromSourceParams; result: Types.ExtensionPlanResult };
    "extensions.planUninstall": { params: ExtensionPlanUninstallParams; result: Types.ExtensionPlanResult };
    "extensions.revokeGrant": { params: ExtensionGrantParams; result: Types.ExtensionGrantResult };
    "extensions.setGrant": { params: ExtensionGrantParams; result: Types.ExtensionGrantResult };
    "extensions.ui.close": { params: Types.ExtensionUiCloseParams; result: Types.ExtensionUiCloseResult };
    "extensions.ui.dispatch": { params: Types.ExtensionUiDispatchParams; result: Types.ExtensionUiDispatchResult };
    "extensions.ui.open": { params: Types.ExtensionUiOpenParams; result: Types.ExtensionUiOpenResult };
    "extensions.ui.refresh": { params: Types.ExtensionUiRefreshParams; result: Types.ExtensionUiSnapshotResult };
    "logs.read": { params: Types.LogsReadParams; result: Types.LogsReadResult };
    "operations.cancel": { params: OperationsCancelParams; result: Types.OperationWriteResult };
    "operations.get": { params: OperationsGetParams; result: Types.Operation };
    "operations.list": { params: Types.OperationsListParams; result: Types.OperationsListResult };
    "packages.applyPlan": { params: Types.PackageApplyPlanParams; result: PackageApplyPlanResult };
    "packages.planBulk": { params: Types.PackagePlanBulkParams; result: Types.PackagePlan };
    "packages.planDowngrade": { params: Types.PackagePlanDowngradeParams; result: Types.PackagePlan };
    "packages.planInstall": { params: Types.PackagePlanInstallParams; result: Types.PackagePlan };
    "packages.planReinstall": { params: Types.PackagePlanReinstallParams; result: Types.PackagePlan };
    "packages.planRemove": { params: Types.PackagePlanRemoveParams; result: Types.PackagePlan };
    "packages.planResolve": { params: Types.PackagePlanResolveParams; result: Types.PackagePlan };
    "packages.planUpgrade": { params: PackagePlanUpgradeParams; result: Types.PackagePlan };
    "packages.projectInventory": { params: Types.ProjectInventoryParams; result: Types.ProjectInventoryResult };
    "packages.queryProjectCandidates": { params: Types.ProjectCandidateRequest; result: Types.ProjectCandidatePage };
    "packages.userPackages.enroll": { params: UserPackageEnrollParams; result: Types.UserPackageWriteResult };
    "packages.userPackages.get": { params: UserPackageGetParams; result: Types.UserPackageResult };
    "packages.userPackages.list": { params: UserPackagesListParams; result: Types.UserPackagesListResult };
    "packages.userPackages.refresh": { params: UserPackageMutationParams; result: Types.UserPackageWriteResult };
    "packages.userPackages.remove": { params: UserPackageMutationParams; result: Types.UserPackageRemoveResult };
    "plans.get": { params: Types.PlansGetParams; result: Types.PlansGetResult };
    "projectTemplates.edit": { params: Types.ProjectTemplateEditParams; result: Types.ProjectTemplatesSaveResult };
    "projectTemplates.export": { params: Types.ProjectTemplatesExportParams; result: Types.ProjectTemplatesSaveResult };
    "projectTemplates.get": { params: Types.ProjectTemplateGetParams; result: Types.ProjectTemplateGetResult };
    "projectTemplates.list": { params: Types.ProjectTemplatesListParams; result: Types.ProjectTemplatesListResult };
    "projectTemplates.remove": { params: Types.ProjectTemplatesRemoveParams; result: Types.ProjectTemplatesRemoveResult };
    "projectTemplates.save": { params: Types.ProjectTemplatesSaveParams; result: Types.ProjectTemplatesSaveResult };
    "projects.applyCopy": { params: ProjectsApplyCopyParams; result: Types.ProjectsApplyCopyResult };
    "projects.applyDeleteDirectory": { params: ProjectsApplyDeleteDirectoryParams; result: Types.ProjectsApplyDeleteDirectoryResult };
    "projects.applyUnityMigration": { params: ProjectsApplyUnityMigrationParams; result: Types.ProjectsApplyUnityMigrationResult };
    "projects.create": { params: Types.ProjectsCreateParams; result: Types.ProjectsCreateResult };
    "projects.get": { params: ProjectIdParams; result: NativeProjectResult };
    "projects.inspect": { params: ProjectsInspectParams; result: NativeProjectResult };
    "projects.list": { params: ProjectsListParams; result: NativeProjectsListResult };
    "projects.planCopy": { params: ProjectsPlanCopyParams; result: Types.ProjectsPlanCopyResult };
    "projects.planDeleteDirectory": { params: ProjectsPlanDeleteDirectoryParams; result: Types.ProjectsPlanDeleteDirectoryResult };
    "projects.planUnityMigration": { params: ProjectsPlanUnityMigrationParams; result: Types.ProjectsPlanUnityMigrationResult };
    "projects.refresh": { params: ProjectMutationParams; result: NativeProjectWriteResult };
    "projects.register": { params: ProjectRegisterParams; result: NativeProjectWriteResult };
    "projects.setFavorite": { params: ProjectSetFavoriteParams; result: NativeProjectWriteResult };
    "projects.unregister": { params: ProjectMutationParams; result: Types.ProjectUnregisterResult };
    "repositories.cancelListRegistration": { params: Types.RepositoryListCancelParams; result: Types.RepositoryListCancelResult };
    "repositories.exportListFile": { params: RepositoryListExportParams; result: Types.RepositoryListExportResult };
    "repositories.get": { params: RepositoryIdParams; result: NativeRepositoryResult };
    "repositories.importListFile": { params: Types.RepositoryListImportParams; result: Types.RepositoryListImportResult };
    "repositories.inspect": { params: RepositoryInspectParams; result: NativeRepositoryResult };
    "repositories.inspectListFile": { params: RepositoryListInspectParams; result: Types.RepositoryListInspection };
    "repositories.list": { params: RegistryListParams; result: NativeRepositoriesListResult };
    "repositories.move": { params: Types.RepositoryMoveParams; result: Types.RepositoryMoveResult };
    "repositories.packages": { params: RepositoryPackagesParams; result: Types.RepositoryPackagesResult };
    "repositories.previewListExport": { params: RepositoryListPreviewParams; result: Types.RepositoryListPreview };
    "repositories.refresh": { params: RepositoryMutationParams; result: NativeRepositoryWriteResult };
    "repositories.refreshAll": { params: Types.RepositoryRefreshAllParams; result: Types.RepositoryRefreshAllResult };
    "repositories.register": { params: RepositoryRegisterParams; result: NativeRepositoryWriteResult };
    "repositories.setCredentials": { params: RepositorySetCredentialsParams; result: NativeRepositoryWriteResult };
    "repositories.setDisplayName": { params: Types.RepositorySetDisplayNameParams; result: NativeRepositoryWriteResult };
    "repositories.unregister": { params: RepositoryMutationParams; result: Types.RepositoryUnregisterResult };
    "settings.get": { params: EmptyParams; result: Types.SettingsGetResult };
    "settings.update": { params: SettingsUpdateParams; result: Types.SettingsGetResult };
    "state.check": { params: StateCheckParams; result: Types.OperationAccepted };
    "system.status": { params: EmptyParams; result: SystemStatusResult };
    "unity.hub.open": { params: Types.UnityHubOpenParams; result: Types.UnityHubOpenResult };
    "unity.installations.get": { params: UnityInstallationIdParams; result: Types.UnityInstallationResult };
    "unity.installations.list": { params: UnityInstallationsListParams; result: Types.UnityInstallationsListResult };
    "unity.installations.refresh": { params: UnityInstallationRefreshParams; result: Types.UnityInstallationsListResult };
    "unity.installations.register": { params: UnityInstallationRegisterParams; result: Types.UnityInstallationResult };
    "unity.installations.remove": { params: UnityInstallationRemoveParams; result: Types.UnityInstallationRemoveResult };
    "unity.launch": { params: UnityLaunchParams; result: Types.UnityLaunchResult };
    "unity.launchOptions": { params: UnityLaunchOptionsParams; result: Types.UnityLaunchOptionsResult };
    "unity.projectLaunchConfig.clear": { params: ProjectUnityLaunchConfigClearParams; result: Types.ProjectUnityLaunchConfigMutationResult };
    "unity.projectLaunchConfig.get": { params: UnityProjectIdParams; result: Types.ProjectUnityLaunchConfigResult };
    "unity.projectLaunchConfig.set": { params: ProjectUnityLaunchConfigSetParams; result: Types.ProjectUnityLaunchConfigMutationResult };
    "unity.writerState": { params: UnityProjectIdParams; result: Types.UnityWriterState };
}
