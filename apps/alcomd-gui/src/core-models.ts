import type { ExtensionRecord } from "@alcomd/sdk";

export interface SystemStatus {
    product: string;
    daemonVersion: string;
    rpcVersion: number;
    state: string;
    capabilities: string[];
}

export interface OperationProgress {
    phase: string;
}

export interface Operation {
    operationId: string;
    kind: string;
    state: string;
    revision: number;
    createdAtMs: number;
    updatedAtMs: number;
    startedAtMs?: number;
    completedAtMs?: number;
    result?: unknown;
    errorCode?: string;
    diagnosticId?: string;
    progress?: OperationProgress;
}

export interface OperationAccepted { operationId: string; replayed: boolean }
export interface OperationWriteResult { operation: Operation; replayed: boolean }

export interface OperationsListResult {
    operations: Operation[];
    nextCursor?: { createdAtMs: number; operationId: string };
}

export type AppearanceMode = "system" | "light" | "dark";
export type AppearanceDensity = "default" | "compact";
export type AppearanceMotion = "system" | "reduced";
export type SettingsLocale = "system" | "en-US" | "zh-CN" | "ja-JP";

export interface OfficialSettings {
    appearance: {
        mode: AppearanceMode;
        sourceColor: string | null;
        density: AppearanceDensity;
        motion: AppearanceMotion;
    };
    locale: SettingsLocale;
    packages: {
        showPrerelease: boolean;
        hiddenRepositoryIds: string[];
        hideLocalUserPackages: boolean;
    };
}

export interface SettingsGetResult {
    configSchema: 2;
    revision: number;
    settings: OfficialSettings;
}

export interface SettingsUpdate {
    appearance?: {
        mode?: AppearanceMode;
        sourceColor?: string | null;
        density?: AppearanceDensity;
        motion?: AppearanceMotion;
    };
    locale?: SettingsLocale;
    packages?: {
        showPrerelease?: boolean;
        hiddenRepositoryIds?: string[];
        hideLocalUserPackages?: boolean;
    };
}

export interface ActivityCursor {
    occurredAtMs: number;
    sourceRank: number;
    stableId: string;
}

export interface ActivityItem {
    occurredAtMs: number;
    type: "operation" | "event";
    summaryCode: string;
    operationId?: string;
    eventSequence?: number;
    resourceKind?: string;
    resourceId?: string;
    state?: string;
}

export interface ActivityListResult {
    items: ActivityItem[];
    nextCursor?: ActivityCursor;
}

export interface DiagnosticCursor {
    occurredAtMs: number;
    operationId: string;
}

export interface DiagnosticItem {
    occurredAtMs: number;
    severity: "warning" | "error";
    subsystem: string;
    code: string;
    diagnosticId?: string;
    operationId?: string;
    summary: string;
}

export interface DiagnosticsListResult {
    items: DiagnosticItem[];
    nextCursor?: DiagnosticCursor;
}

export interface DependencyIdentity {
    packageId: string;
    value: string;
}

export interface ReadIssue {
    code: string;
    component: string;
    item: string;
    line?: number;
    column?: number;
}

export interface ProjectSnapshot {
    projectId?: string;
    registeredAtMs?: number;
    favorite?: boolean;
    rootPath: string;
    projectType: string;
    unityVersion: string;
    unityRevision?: string;
    vpmManifest: string;
    upmManifest: string;
    directDependencies: DependencyIdentity[];
    lockedDependencies: DependencyIdentity[];
    issues: ReadIssue[];
    observedAtMs: number;
    revision?: number;
}

export interface ProjectResult { project: ProjectSnapshot }
export interface ProjectsListResult { projects: ProjectSnapshot[]; nextCursor?: RegistryCursor }
export interface ProjectWriteResult { project: ProjectSnapshot; replayed: boolean }
export interface ProjectUnregisterResult { projectId: string; revision: number; unregistered: boolean; replayed: boolean }

export interface ProjectCopyPlan {
    planId: string;
    sourceProjectId: string;
    sourceProjectRevision: number;
    sourceCanonicalRootPath: string;
    targetParentCanonicalPath: string;
    normalizedTargetLeaf: string;
    targetProjectId: string;
    writerEvidence: { state: string; observedAtMs: number };
    profile: {
        id: string;
        version: number;
        excludes: string[];
        quota: {
            maxEntries: number;
            maxSingleFileBytes: number;
            maxTotalRegularFileBytes: number;
            maxDepth: number;
            maxNormalizedPathUtf8Bytes: number;
        };
    };
    createdAtMs: number;
    expiresAtMs: number;
}

export interface ProjectsPlanCopyResult { plan: ProjectCopyPlan; replayed: boolean }
export interface ProjectsApplyCopyResult { operationId: string; targetProjectId: string; replayed: boolean }

export interface ProjectDeletePlan {
    planId: string;
    projectId: string;
    projectRevision: number;
    canonicalRootPath: string;
    normalizedLeaf: string;
    writerEvidence: { state: string; observedAtMs: number; safeEvidence: string[] };
    profile: { id: string; version: number; mode: string; protectedRootProfileVersion: number; progress: string };
    createdAtMs: number;
    expiresAtMs: number;
}

export interface ProjectsPlanDeleteDirectoryResult { plan: ProjectDeletePlan; replayed: boolean }
export interface ProjectsApplyDeleteDirectoryResult { operationId: string; projectId: string; replayed: boolean }

export interface RegistryCursor {
    registeredAtMs: number;
    id: string;
}

export type RepositorySource =
    | { kind: "local"; path: string }
    | { kind: "remote"; url: string };

export interface RepositorySnapshot {
    repositoryId?: string;
    source: RepositorySource;
    declaredId?: string;
    name?: string;
    declaredUrl?: string;
    issues: ReadIssue[];
    revision?: number;
    refreshedAtMs: number;
}

export interface RepositoryResult { repository: RepositorySnapshot }
export interface RepositoriesListResult { repositories: RepositorySnapshot[]; nextCursor?: RegistryCursor }
export interface RepositoryWriteResult { repository: RepositorySnapshot; replayed: boolean }
export interface RepositoryUnregisterResult { repositoryId: string; revision: number; unregistered: boolean; replayed: boolean }

export interface RepositoryPackageVersion {
    packageId: string;
    version: string;
    displayName?: string;
    description?: string;
    yanked: boolean;
    unity?: string;
    prerelease?: boolean;
    links?: {
        documentation?: { url: string };
        changelog?: { url: string };
    };
}

export interface RepositoryPackagesResult {
    packages: RepositoryPackageVersion[];
    nextCursor?: PackageCursor;
}

export interface PackageCursor {
    packageId: string;
    version: string;
}

export interface RepositoryPackageSourcePin {
    repositoryId: string;
    repositoryRevision: number;
    sourceIdentity: string;
    manifestFingerprint: string;
    packageId: string;
    version: string;
    artifactUrl: string;
    archiveSha256: string;
}

export interface UserPackageSourcePin {
    userPackageId: string;
    sourceRevision: number;
    sourceIdentity: string;
    manifestFingerprint: string;
    packageId: string;
    version: string;
    archiveSha256: string;
}

export type PackageSourcePin = RepositoryPackageSourcePin | UserPackageSourcePin;
export type PackageSourceSelector =
    | { kind: "repository"; repositoryId: string }
    | { kind: "user_package"; userPackageId: string };

export interface PackageMutation {
    kind: "install" | "remove" | "replace";
    packageId: string;
    fromVersion?: string;
    toVersion?: string;
    source?: PackageSourcePin;
}

export interface PackageChangeSet {
    formatVersion: number;
    mutations: PackageMutation[];
    dependencyEdges: Array<{ fromPackageId: string; toPackageId: string; range: string; direct: boolean }>;
    vpmManifestSha256: string;
}

export interface PackagePlan {
    planId: string;
    action: "install" | "remove" | "upgrade" | "downgrade" | "resolve" | "reinstall" | "bulk";
    state: "unapplied" | "applied";
    projectId: string;
    projectRevision: number;
    changeSetFingerprint: string;
    changeSet: PackageChangeSet;
}

export interface PackagePlanInstallParams {
    projectId: string;
    expectedRevision: number;
    packageId: string;
    versionRange?: string;
    repositoryId?: string;
    source?: PackageSourceSelector;
    includePrerelease: boolean;
}

export interface PackagePlanRemoveParams { projectId: string; expectedRevision: number; packageId: string }
export interface PackagePlanDowngradeParams { projectId: string; expectedRevision: number; packageId: string; version: string; repositoryId?: string; source?: PackageSourceSelector }
export interface PackagePlanResolveParams { projectId: string; expectedRevision: number; includePrerelease: boolean }
export type PackageReinstallSelection = { kind: "packages"; packageIds: string[] } | { kind: "all" };
export interface PackageReinstallSource { packageId: string; source: PackageSourceSelector }
export interface PackagePlanReinstallParams { projectId: string; expectedRevision: number; selection: PackageReinstallSelection; sources?: PackageReinstallSource[] }
export type PackageBulkIntent =
    | { kind: "install"; packageId: string; versionRange?: string; source?: PackageSourceSelector; includePrerelease: boolean }
    | { kind: "upgrade"; packageId: string; versionRange?: string; source?: PackageSourceSelector; includePrerelease: boolean }
    | { kind: "remove"; packageId: string }
    | { kind: "reinstall"; packageId: string; source?: PackageSourceSelector };
export interface PackagePlanBulkParams { projectId: string; expectedRevision: number; intents: PackageBulkIntent[] }
export interface PackageApplyPlanParams { planId: string; expectedRevision: number; idempotencyKey: string }

export interface UserPackageRecord {
    userPackageId: string;
    sourceRootPath: string;
    packageId: string;
    version: string;
    displayName?: string;
    revision: number;
    archiveSha256: string;
    createdAtMs: number;
    updatedAtMs: number;
}
export interface UserPackageCursor { updatedAtMs: number; userPackageId: string }
export interface UserPackagesListResult { userPackages: UserPackageRecord[]; nextCursor?: UserPackageCursor }
export interface UserPackageResult { userPackage: UserPackageRecord }
export interface UserPackageWriteResult { userPackage: UserPackageRecord; replayed: boolean }
export interface UserPackageRemoveResult { userPackageId: string; revision: number; removed: boolean; replayed: boolean }

export interface UnityInstallation {
    installationId: string;
    executablePath: string;
    filesystemIdentity: string;
    unityVersion: string;
    architecture: string;
    sourceKind: string;
    revision: number;
    observedAtMs: number;
    updatedAtMs: number;
}

export interface UnityInstallationsListResult {
    installations: UnityInstallation[];
    nextCursor?: string;
    replayed: boolean;
}

export interface UnityInstallationResult { installation: UnityInstallation; replayed: boolean }
export interface UnityInstallationRemoveResult { installationId: string; removed: boolean; replayed: boolean }

export interface ProjectEditorPreference {
    projectId: string;
    installationId: string;
    arguments: string[];
    revision: number;
    updatedAtMs: number;
}

export interface ProjectEditorResult { preference: ProjectEditorPreference; replayed: boolean }

export type ProjectEditorSelection =
    | { mode: "automatic" }
    | { mode: "explicit"; installationId: string };

export interface ProjectEditorSelectionState {
    projectId: string;
    selection: ProjectEditorSelection;
    arguments: string[];
    revision: number;
    updatedAtMs: number;
}

export interface ProjectEditorSelectionResult { preference: ProjectEditorSelectionState }
export interface ProjectEditorClearResult { preference: ProjectEditorSelectionState; replayed: boolean }

export interface UnityWriterState {
    projectId: string;
    state: string;
    evidence: Array<{ kind: string }>;
    checkedAtMs: number;
}

export interface UnityLaunchRecord {
    launchId: string;
    projectId: string;
    installationId: string;
    state: string;
    spawnAccepted: boolean;
    createdAtMs: number;
}

export interface UnityLaunchResult { launch: UnityLaunchRecord; replayed: boolean }

export interface TemplatePlan {
    planId: string;
    action: string;
    state: string;
    planFingerprint: string;
    [key: string]: unknown;
}

export interface TemplateApplyResult { operationId: string; replayed: boolean }
export interface TemplateExportResult { exported: boolean }
export interface TemplateRemoveResult { templateId: string; removed: boolean; replayed: boolean }

export interface TemplateRecord {
    templateId: string;
    sourceKind: string;
    templateVersion: string;
    displayName: string;
    description?: string;
    provenance: string;
    favorite: boolean;
    bundleSha256: string;
    manifestFingerprint: string;
    revision: number;
    createdAtMs: number;
    updatedAtMs: number;
}

export interface TemplatesListResult { templates: TemplateRecord[]; nextCursor?: string }
export interface TemplateRecordResult { template: TemplateRecord; replayed: boolean }
export interface TemplateBundleInspection {
    formatVersion: number;
    templateId: string;
    templateVersion: string;
    displayName: string;
    description?: string;
    provenance: string;
    bundleSha256: string;
    manifestFingerprint: string;
    payloadTreeSha256: string;
    entryCount: number;
    totalBytes: number;
}

export interface BackupRecord {
    backupId: string;
    sourceProjectId: string;
    archiveSha256: string;
    archiveBytes: number;
    formatVersion: number;
    createdAtMs: number;
    compressionMode: string;
    excludeVpmPackages: boolean;
}

export interface BackupsListResult { backups: BackupRecord[]; nextCursor?: string }

export interface BackupCreateResult { operationId: string; backupId: string; replayed: boolean }
export interface BackupRestorePlan {
    planId: string;
    projectId: string;
    backupId: string;
    target: { parent: string; leaf: string; mustBeAbsent: boolean };
    archiveSha256: string;
    packagesRequireResolve: boolean;
    excludedPackages: Array<{ packageId: string; version: string }>;
    planFingerprint: string;
}
export interface BackupApplyRestoreResult { operationId: string; projectId: string; replayed: boolean }
export interface ExtensionsListResult { extensions: ExtensionRecord[]; nextCursor?: string }

export interface ExtensionPlan {
    planId: string;
    action: string;
    state: string;
    sourceKind: string;
    extensionId: string;
    version: string;
    apiMajor: number;
    profileVersion: number;
    packageDigest: string;
    publisherFingerprint: string;
    trustDecision: string;
    dataDisposition: string;
    planFingerprint: string;
    uiProtocol?: string;
}

export interface ExtensionPlanResult { plan: ExtensionPlan }
export interface ExtensionOperationResult { operationId: string; replayed: boolean }
export interface ExtensionGrantResult { extensionId: string; grantRevision: number; state: string; replayed: boolean }
