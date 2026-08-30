import type {
    ExtensionRecord,
    ExtensionResult,
    ExtensionUiCloseResult,
    ExtensionUiDispatchParams,
    ExtensionUiDispatchResult,
    ExtensionUiOpenParams,
    ExtensionUiOpenResult,
    ExtensionUiRefreshParams,
    ExtensionUiSnapshotResult,
    UiDocument,
    UiSnapshot
} from "@alcomd/sdk";
import React from "react";
import ReactDOM from "react-dom/client";

import discordDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/discord-presence-snapshot.json" with { type: "json" };
import mcpDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/mcp-management-snapshot.json" with { type: "json" };
import { App } from "../../src/App";
import type { OfficialSettings, Operation, PackagePlan, SettingsGetResult } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";
import "../../src/styles.css";
import { MaterialFoundationEvidence } from "./MaterialFoundationEvidence";

type HarnessMode = "ready" | "empty" | "error" | "disconnected" | "loading" | "stale" | "failed" | "cancelled" | "create-error" | "restore-error" | "favorite-pages" | "favorite-error" | "favorite-conflict" | "unity-automatic" | "unity-zero" | "unity-multiple" | "package-no-repositories" | "package-multiple" | "package-user-source" | "package-partial-failure" | "package-revision-conflict";

const query = new URLSearchParams(window.location.search);
const initialRoute = query.get("route") ?? "/";
const mode = (query.get("state") ?? "ready") as HarnessMode;
const materialEvidence = query.get("material") === "1";
if (!materialEvidence) window.history.replaceState(null, "", initialRoute);

class DeterministicGuiClient implements GuiRpcClient {
    private settings: SettingsGetResult = {
        configSchema: 2,
        revision: 7,
        settings: {
            appearance: { mode: "system", sourceColor: null, density: "default", motion: "system" },
            locale: "en-US",
            packages: {
                showPrerelease: false,
                hiddenRepositoryIds: [],
                hideLocalUserPackages: false
            }
        }
    };
    private operationReads = 0;
    private pendingProject?: ReturnType<typeof project>;
    private pendingDeleteProjectId?: string;
    private deletedProjectIds = new Set<string>();
    private createdProjects: ReturnType<typeof project>[] = [];
    private favoriteConflictPending = true;
    private selectionCleared = false;
    private selectionExplicit = false;
    private snapshotRevision = 1;
    private repositoryRefreshCompleted = false;
    private repositoryRefreshAttempts = new Map<string, number>();
    private userPackageRemoved = false;
    private userPackageRevision = 1;

    constructor(private readonly mode: HarnessMode) {}

    private async value<T>(value: T): Promise<T> {
        if (this.mode === "loading") return new Promise<T>(() => undefined);
        if (this.mode === "error") throw { code: "internal_error", diagnosticId: "00000000-0000-4000-8000-000000000999" };
        if (this.mode === "disconnected") throw { code: "daemon_unavailable" };
        return structuredClone(value);
    }

    systemStatus(): ReturnType<GuiRpcClient["systemStatus"]> {
        return this.value({ product: "ALCOMD", daemonVersion: "4.0.0-alpha.0", rpcVersion: 1, state: "ready", capabilities: ["extensions.ui.portable.v1"] });
    }

    stateCheck(): ReturnType<GuiRpcClient["stateCheck"]> {
        this.operationReads = 0;
        return this.value({ operationId: OPERATION_ID, replayed: false });
    }

    operationsList(): ReturnType<GuiRpcClient["operationsList"]> {
        return this.value({ operations: this.mode === "empty" ? [] : [runningOperation()] });
    }

    operationGet(operationId: string): ReturnType<GuiRpcClient["operationGet"]> {
        this.operationReads += 1;
        const terminal = this.operationReads > 2;
        const state = this.mode === "failed" ? "failed" : this.mode === "cancelled" ? "cancelled" : terminal ? "succeeded" : "running";
        if (state === "succeeded" && this.pendingProject !== undefined && !this.createdProjects.some((candidate) => candidate.projectId === this.pendingProject?.projectId)) {
            this.createdProjects.push(this.pendingProject);
        }
        if (state === "succeeded" && this.pendingDeleteProjectId !== undefined) {
            this.deletedProjectIds.add(this.pendingDeleteProjectId);
            this.pendingDeleteProjectId = undefined;
        }
        return this.value({
            ...runningOperation(),
            operationId,
            state,
            progress: { phase: state === "running" ? "extracting" : "state_committed" },
            ...(state === "succeeded" && this.pendingProject !== undefined ? { result: { projectId: this.pendingProject.projectId, revision: 1 } } : {}),
            ...(state === "failed" ? { errorCode: "package_archive_invalid" } : {})
        });
    }

    operationCancel(operationId: string): ReturnType<GuiRpcClient["operationCancel"]> {
        return this.value({ operation: { ...runningOperation(), operationId, state: "cancelling", revision: 4 }, replayed: false });
    }

    settingsGet(): ReturnType<GuiRpcClient["settingsGet"]> {
        return this.value(this.settings);
    }

    settingsUpdate(expectedRevision: number, update: Partial<OfficialSettings>): ReturnType<GuiRpcClient["settingsUpdate"]> {
        if (expectedRevision !== this.settings.revision) return Promise.reject({ code: "revision_conflict" });
        this.settings = {
            configSchema: 2,
            revision: expectedRevision + 1,
            settings: {
                appearance: { ...this.settings.settings.appearance, ...update.appearance },
                locale: update.locale ?? this.settings.settings.locale,
                packages: { ...this.settings.settings.packages, ...update.packages }
            }
        };
        return this.value(this.settings);
    }

    activityList(): ReturnType<GuiRpcClient["activityList"]> {
        return this.value({ items: this.mode === "empty" ? [] : [{ occurredAtMs: 1_700_000_000_000, type: "operation", summaryCode: "operation.packages.apply.running", operationId: OPERATION_ID, state: "running" }] });
    }

    diagnosticsList(): ReturnType<GuiRpcClient["diagnosticsList"]> {
        return this.value({ items: this.mode === "empty" ? [] : [{ occurredAtMs: 1_700_000_000_000, severity: "error", subsystem: "packages", code: "package_archive_invalid", diagnosticId: "00000000-0000-4000-8000-000000000998", operationId: OPERATION_ID, summary: "The operation failed. Use the diagnostic ID when requesting support." }] });
    }

    projectsInspect(): ReturnType<GuiRpcClient["projectsInspect"]> { return this.value({ project: project() }); }
    projectsList(cursor?: { registeredAtMs: number; id: string }): ReturnType<GuiRpcClient["projectsList"]> {
        if (this.mode === "favorite-pages") {
            return cursor === undefined
                ? this.value({ projects: [project(PROJECT_ID, "Alpha")], nextCursor: { registeredAtMs: 1_690_000_000_000, id: PROJECT_ID } })
                : this.value({ projects: [project("00000000-0000-4000-8000-000000000110", "Zulu Favorite", true)] });
        }
        const projects = [project(), ...this.createdProjects].filter((candidate) => candidate.projectId === undefined || !this.deletedProjectIds.has(candidate.projectId));
        return this.value({ projects: this.mode === "empty" ? [] : projects });
    }
    projectGet(projectId: string): ReturnType<GuiRpcClient["projectGet"]> { return this.value({ project: this.createdProjects.find((candidate) => candidate.projectId === projectId) ?? project(projectId) }); }
    openProjectDirectory(): ReturnType<GuiRpcClient["openProjectDirectory"]> { return this.value(undefined); }
    selectDirectory(): ReturnType<GuiRpcClient["selectDirectory"]> { return this.value("C:\\Fixture\\Avatar"); }
    projectRegister(): ReturnType<GuiRpcClient["projectRegister"]> { return this.value({ project: project(), replayed: false }); }
    projectRefresh(): ReturnType<GuiRpcClient["projectRefresh"]> { return this.value({ project: project(), replayed: false }); }
    projectSetFavorite(_projectId: string, favorite: boolean): ReturnType<GuiRpcClient["projectSetFavorite"]> {
        if (this.mode === "favorite-error") return Promise.reject({ code: "internal_error" });
        if (this.mode === "favorite-conflict" && this.favoriteConflictPending) {
            this.favoriteConflictPending = false;
            return Promise.reject({ code: "revision_conflict" });
        }
        return this.value({ project: { ...project(), favorite, revision: 3 }, replayed: false });
    }
    projectUnregister(): ReturnType<GuiRpcClient["projectUnregister"]> { return this.value({ projectId: PROJECT_ID, revision: 3, unregistered: true, replayed: false }); }
    projectPlanCopy(_sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string): ReturnType<GuiRpcClient["projectPlanCopy"]> {
        return this.value({
            plan: {
                planId: "00000000-0000-4000-8000-000000000060",
                sourceProjectId: PROJECT_ID,
                sourceProjectRevision: expectedRevision,
                sourceCanonicalRootPath: "C:\\Projects\\Sample",
                targetParentCanonicalPath: targetParentPath,
                normalizedTargetLeaf: targetLeaf,
                targetProjectId: "00000000-0000-4000-8000-000000000061",
                writerEvidence: { state: "not_observed", observedAtMs: 1 },
                profile: {
                    id: "alcomd-project-copy",
                    version: 1,
                    excludes: ["root/Logs", "root/Obj", "root/Temp", "**/.git"],
                    quota: { maxEntries: 500000, maxSingleFileBytes: 34359738368, maxTotalRegularFileBytes: 137438953472, maxDepth: 128, maxNormalizedPathUtf8Bytes: 1024 }
                },
                createdAtMs: 1,
                expiresAtMs: 900001
            },
            replayed: false
        });
    }
    projectApplyCopy(): ReturnType<GuiRpcClient["projectApplyCopy"]> { return this.value({ operationId: "00000000-0000-4000-8000-000000000062", targetProjectId: "00000000-0000-4000-8000-000000000061", replayed: false }); }
    projectPlanDeleteDirectory(_projectId: string, expectedRevision: number): ReturnType<GuiRpcClient["projectPlanDeleteDirectory"]> {
        return this.value({
            plan: {
                planId: "00000000-0000-4000-8000-000000000063",
                projectId: PROJECT_ID,
                projectRevision: expectedRevision,
                canonicalRootPath: "C:\\Projects\\Sample",
                normalizedLeaf: "Sample",
                writerEvidence: { state: "not_observed", observedAtMs: 1, safeEvidence: [] },
                profile: { id: "alcomd-project-delete", version: 1, mode: "sibling-quarantine-permanent-v1", protectedRootProfileVersion: 1, progress: "phase-only" },
                createdAtMs: 1,
                expiresAtMs: 900001
            },
            replayed: false
        });
    }
    projectApplyDeleteDirectory(): ReturnType<GuiRpcClient["projectApplyDeleteDirectory"]> {
        this.pendingDeleteProjectId = PROJECT_ID;
        return this.value({ operationId: "00000000-0000-4000-8000-000000000064", projectId: PROJECT_ID, replayed: false });
    }

    repositoriesInspect(): ReturnType<GuiRpcClient["repositoriesInspect"]> { return this.value({ repository: repository() }); }
    repositoriesList(cursor?: { registeredAtMs: number; id: string }): ReturnType<GuiRpcClient["repositoriesList"]> {
        if (this.mode === "empty" || this.mode === "package-no-repositories") return this.value({ repositories: [] });
        if (["package-multiple", "package-partial-failure"].includes(this.mode)) {
            return cursor === undefined
                ? this.value({ repositories: [repository()], nextCursor: { registeredAtMs: 1_690_000_000_000, id: REPOSITORY_ID } })
                : this.value({ repositories: [localRepository()] });
        }
        return this.value({ repositories: [repository()] });
    }
    repositoryGet(repositoryId: string): ReturnType<GuiRpcClient["repositoryGet"]> {
        const value = repositoryId === LOCAL_REPOSITORY_ID ? localRepository() : repository();
        return this.value({ repository: this.mode === "package-revision-conflict" ? { ...value, revision: 3 } : value });
    }
    repositoryPackages(repositoryId: string): ReturnType<GuiRpcClient["repositoryPackages"]> {
        const packages = repositoryId === LOCAL_REPOSITORY_ID
            ? [{ packageId: "com.example.local", version: "2.0.0", displayName: "Local tools", yanked: false }]
            : [
                { packageId: "com.example.avatar", version: "1.2.3", displayName: "Avatar tools", yanked: false, unity: ">=2022.3" },
                { packageId: "com.example.avatar", version: "1.3.0", displayName: "Avatar tools", yanked: false, unity: ">=2022.3" },
                { packageId: "com.example.remote", version: "1.0.0", displayName: "Remote tools", yanked: false },
                ...(this.repositoryRefreshCompleted ? [{ packageId: "com.example.refreshed", version: "1.0.0", displayName: "Refreshed package", yanked: false }] : [])
            ];
        return this.value({ packages });
    }

    openPackageLink(): ReturnType<GuiRpcClient["openPackageLink"]> {
        return this.value(undefined);
    }
    repositoryRegister(): ReturnType<GuiRpcClient["repositoryRegister"]> { return this.value({ repository: repository(), replayed: false }); }
    repositoryRefresh(repositoryId: string): ReturnType<GuiRpcClient["repositoryRefresh"]> {
        const attempts = (this.repositoryRefreshAttempts.get(repositoryId) ?? 0) + 1;
        this.repositoryRefreshAttempts.set(repositoryId, attempts);
        if (this.mode === "package-partial-failure" && repositoryId === LOCAL_REPOSITORY_ID) return Promise.reject({ code: "repository_unavailable" });
        if (this.mode === "package-revision-conflict" && repositoryId === REPOSITORY_ID && attempts === 1) return Promise.reject({ code: "revision_conflict" });
        if (this.mode === "package-revision-conflict" && repositoryId === REPOSITORY_ID && attempts > 2) return Promise.reject({ code: "unexpected_retry" });
        this.repositoryRefreshCompleted = true;
        return this.value({ repository: repositoryId === LOCAL_REPOSITORY_ID ? localRepository() : repository(), replayed: false });
    }
    repositoryUnregister(): ReturnType<GuiRpcClient["repositoryUnregister"]> { return this.value({ repositoryId: REPOSITORY_ID, revision: 3, unregistered: true, replayed: false }); }

    packagePlanInstall(): ReturnType<GuiRpcClient["packagePlanInstall"]> { return this.value(packagePlan("install")); }
    packagePlanRemove(): ReturnType<GuiRpcClient["packagePlanRemove"]> { return this.value(packagePlan("remove")); }
    packagePlanUpgrade(): ReturnType<GuiRpcClient["packagePlanUpgrade"]> { return this.value(packagePlan("upgrade")); }
    packagePlanDowngrade(): ReturnType<GuiRpcClient["packagePlanDowngrade"]> { return this.value(packagePlan("downgrade")); }
    packagePlanResolve(): ReturnType<GuiRpcClient["packagePlanResolve"]> { return this.value(packagePlan("resolve")); }
    packagePlanReinstall(): ReturnType<GuiRpcClient["packagePlanReinstall"]> { return this.value(packagePlan("reinstall")); }
    packagePlanBulk(): ReturnType<GuiRpcClient["packagePlanBulk"]> { return this.value(packagePlan("bulk")); }
    packageApplyPlan(): ReturnType<GuiRpcClient["packageApplyPlan"]> {
        if (this.mode === "stale") return Promise.reject({ code: "plan_stale" });
        this.operationReads = 0;
        return this.value({ operationId: OPERATION_ID, replayed: false });
    }

    userPackagesList(): ReturnType<GuiRpcClient["userPackagesList"]> {
        return this.value({ userPackages: this.mode === "package-user-source" && !this.userPackageRemoved ? [userPackage(this.userPackageRevision)] : [] });
    }
    userPackageGet(): ReturnType<GuiRpcClient["userPackageGet"]> { return this.value({ userPackage: userPackage() }); }
    userPackageEnroll(): ReturnType<GuiRpcClient["userPackageEnroll"]> { return this.value({ userPackage: userPackage(), replayed: false }); }
    userPackageRefresh(): ReturnType<GuiRpcClient["userPackageRefresh"]> {
        this.userPackageRevision = 2;
        return this.value({ userPackage: userPackage(2), replayed: false });
    }
    userPackageRemove(): ReturnType<GuiRpcClient["userPackageRemove"]> {
        this.userPackageRemoved = true;
        return this.value({ userPackageId: USER_PACKAGE_ID, revision: 2, removed: true, replayed: false });
    }

    unityInstallationsList(): ReturnType<GuiRpcClient["unityInstallationsList"]> {
        const installations = this.mode === "empty" || this.mode === "unity-zero"
            ? []
            : this.mode === "unity-multiple"
                ? [installation(), { ...installation(), installationId: "00000000-0000-4000-8000-000000000111", filesystemIdentity: "opaque-two", executablePath: "<private-editor-two>" }]
                : [installation()];
        return this.value({ installations, replayed: false });
    }
    unityInstallationGet(): ReturnType<GuiRpcClient["unityInstallationGet"]> { return this.value({ installation: installation(), replayed: false }); }
    unityInstallationRegister(): ReturnType<GuiRpcClient["unityInstallationRegister"]> { return this.value({ installation: installation(), replayed: false }); }
    unityInstallationRemove(): ReturnType<GuiRpcClient["unityInstallationRemove"]> { return this.value({ installationId: INSTALLATION_ID, removed: true, replayed: false }); }
    unityInstallationsRefresh(): ReturnType<GuiRpcClient["unityInstallationsRefresh"]> { return this.value({ installations: [installation()], replayed: false }); }
    unityProjectEditorGet(): ReturnType<GuiRpcClient["unityProjectEditorGet"]> { return this.value({ preference: editorPreference(), replayed: false }); }
    unityProjectEditorSet(_projectId: string, installationId: string, arguments_: string[]): ReturnType<GuiRpcClient["unityProjectEditorSet"]> {
        this.selectionExplicit = true;
        return this.value({ preference: { ...editorPreference(), installationId, arguments: arguments_, revision: 3 }, replayed: false });
    }
    unityProjectEditorSelectionGet(): ReturnType<GuiRpcClient["unityProjectEditorSelectionGet"]> {
        const automatic = this.selectionCleared || (!this.selectionExplicit && ["unity-automatic", "unity-zero", "unity-multiple"].includes(this.mode));
        return this.value({ preference: automatic ? { ...editorSelection(), selection: { mode: "automatic" }, arguments: ["-logFile"], revision: this.selectionCleared ? 3 : 0, updatedAtMs: this.selectionCleared ? 1_700_000_000_001 : 0 } : editorSelection() });
    }
    unityProjectEditorClear(): ReturnType<GuiRpcClient["unityProjectEditorClear"]> {
        this.selectionCleared = true;
        return this.value({ preference: { ...editorSelection(), selection: { mode: "automatic" }, arguments: ["-logFile"], revision: 3 }, replayed: false });
    }
    unityWriterState(): ReturnType<GuiRpcClient["unityWriterState"]> { return this.value({ projectId: PROJECT_ID, state: "not_observed", evidence: [], checkedAtMs: 1_700_000_000_000 }); }
    unityLaunch(): ReturnType<GuiRpcClient["unityLaunch"]> {
        if (this.mode === "unity-zero") return Promise.reject({ code: "unity_installation_not_found" });
        if (this.mode === "unity-multiple" && !this.selectionExplicit) return Promise.reject({ code: "unity_editor_selection_required" });
        return new Promise((resolve) => window.setTimeout(() => resolve({ launch: launch(), replayed: false }), 100));
    }
    unityLaunchStatus(): ReturnType<GuiRpcClient["unityLaunchStatus"]> { return this.value({ launch: launch(), replayed: false }); }

    templatesList(): ReturnType<GuiRpcClient["templatesList"]> { return this.value({ templates: this.mode === "empty" ? [] : [template()] }); }
    templateGet(): ReturnType<GuiRpcClient["templateGet"]> { return this.value({ template: template(), replayed: false }); }
    templateInspectBundle(): ReturnType<GuiRpcClient["templateInspectBundle"]> { return this.value({ formatVersion: 1, templateId: TEMPLATE_ID, templateVersion: "1.0.0", displayName: "Avatar starter", provenance: "public-fixture", bundleSha256: HASH, manifestFingerprint: HASH, payloadTreeSha256: HASH, entryCount: 4, totalBytes: 4096 }); }
    templatePlanImport(): ReturnType<GuiRpcClient["templatePlanImport"]> { return this.value(templatePlan("import")); }
    templateApplyImport(): ReturnType<GuiRpcClient["templateApplyImport"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    templatePlanDerive(): ReturnType<GuiRpcClient["templatePlanDerive"]> { return this.value(templatePlan("derive")); }
    templateApplyDerive(): ReturnType<GuiRpcClient["templateApplyDerive"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    templateExport(): ReturnType<GuiRpcClient["templateExport"]> { return this.value({ exported: true }); }
    templateSetFavorite(): ReturnType<GuiRpcClient["templateSetFavorite"]> { return this.value({ template: { ...template(), favorite: true }, replayed: false }); }
    templateRemove(): ReturnType<GuiRpcClient["templateRemove"]> { return this.value({ templateId: TEMPLATE_ID, removed: true, replayed: false }); }
    templatePlanCreateProject(_templateId: string, _expectedTemplateRevision: number, _targetParent: string, targetLeaf: string): ReturnType<GuiRpcClient["templatePlanCreateProject"]> {
        if (this.mode === "create-error") return Promise.reject({ code: "template_plan_stale" });
        this.pendingProject = project(CREATED_PROJECT_ID, targetLeaf);
        return this.value({ ...templatePlan("create-project"), targetLeaf });
    }
    templateApplyCreateProject(): ReturnType<GuiRpcClient["templateApplyCreateProject"]> { this.operationReads = 0; return this.value({ operationId: OPERATION_ID, replayed: false }); }

    backupsList(): ReturnType<GuiRpcClient["backupsList"]> { return this.value({ backups: this.mode === "empty" ? [] : [backup()] }); }
    backupGet(): ReturnType<GuiRpcClient["backupGet"]> { return this.value(backup()); }
    backupCreate(): ReturnType<GuiRpcClient["backupCreate"]> { return this.value({ operationId: OPERATION_ID, backupId: BACKUP_ID, replayed: false }); }
    backupPlanRestore(_backupId: string, targetParent: string, targetLeaf: string): ReturnType<GuiRpcClient["backupPlanRestore"]> {
        if (this.mode === "restore-error") return Promise.reject({ code: "backup_target_conflict" });
        this.pendingProject = project(RESTORED_PROJECT_ID, targetLeaf);
        return this.value({ planId: PLAN_ID, projectId: RESTORED_PROJECT_ID, backupId: BACKUP_ID, target: { parent: targetParent, leaf: targetLeaf, mustBeAbsent: true }, archiveSha256: HASH, packagesRequireResolve: false, excludedPackages: [], planFingerprint: HASH });
    }
    backupApplyRestore(): ReturnType<GuiRpcClient["backupApplyRestore"]> { this.operationReads = 0; return this.value({ operationId: OPERATION_ID, projectId: RESTORED_PROJECT_ID, replayed: false }); }

    extensionsList(): ReturnType<GuiRpcClient["extensionsList"]> { return this.value({ extensions: this.mode === "empty" ? [] : [extension(DISCORD_ID), extension(MCP_ID)] }); }
    extensionGet(extensionId: string): ReturnType<GuiRpcClient["extensionGet"]> { return this.value({ extension: extension(extensionId) }); }
    extensionPlanInstall(): ReturnType<GuiRpcClient["extensionPlanInstall"]> { return this.value({ plan: extensionPlan("install") }); }
    extensionApplyInstall(): ReturnType<GuiRpcClient["extensionApplyInstall"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    extensionEnable(extensionId: string): ReturnType<GuiRpcClient["extensionEnable"]> { return this.value({ extension: { ...extension(extensionId), desiredState: "enabled" } }); }
    extensionDisable(extensionId: string): ReturnType<GuiRpcClient["extensionDisable"]> { return this.value({ extension: { ...extension(extensionId), desiredState: "installed_disabled" } }); }
    extensionPlanUninstall(): ReturnType<GuiRpcClient["extensionPlanUninstall"]> { return this.value({ plan: extensionPlan("uninstall") }); }
    extensionApplyUninstall(): ReturnType<GuiRpcClient["extensionApplyUninstall"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    extensionSetGrant(): ReturnType<GuiRpcClient["extensionSetGrant"]> { return this.value({ extensionId: DISCORD_ID, grantRevision: 4, state: "granted", replayed: false }); }
    extensionRevokeGrant(): ReturnType<GuiRpcClient["extensionRevokeGrant"]> { return this.value({ extensionId: DISCORD_ID, grantRevision: 4, state: "revoked", replayed: false }); }

    extensionUiOpen(params: ExtensionUiOpenParams): Promise<ExtensionUiOpenResult> {
        const snapshot = this.snapshot(params.extensionId);
        return this.value({ session: { sessionId: snapshot.sessionId, extensionId: params.extensionId, locale: params.locale, idleTimeoutMs: 300_000, absoluteTimeoutMs: 3_600_000 }, snapshot });
    }
    extensionUiRefresh(params: ExtensionUiRefreshParams): Promise<ExtensionUiSnapshotResult> { return this.value({ snapshot: this.snapshot(params.sessionId.includes("mcp") ? MCP_ID : DISCORD_ID) }); }
    extensionUiDispatch(params: ExtensionUiDispatchParams): Promise<ExtensionUiDispatchResult> { this.snapshotRevision += 1; return this.value({ snapshot: this.snapshot(params.sessionId.includes("mcp") ? MCP_ID : DISCORD_ID), replayed: false }); }
    extensionUiClose(): Promise<ExtensionUiCloseResult> { return this.value({ closed: true }); }

    private snapshot(extensionId: string): UiSnapshot {
        const isMcp = extensionId.includes("mcp");
        const document = structuredClone((isMcp ? mcpDocumentJson : discordDocumentJson) as UiDocument);
        if (!isMcp) {
            const readOnly = document.nodes.find((node) => node.nodeId === "presence-text");
            if (readOnly?.kind === "text-field") {
                readOnly.payload.readOnly = true;
                readOnly.payload.validation = {
                    state: "invalid",
                    message: "The host rejected the previous value."
                };
            }
        }
        return {
            sessionId: `session-${isMcp ? "mcp" : "discord"}`,
            snapshotRevision: this.snapshotRevision,
            document
        };
    }
}

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";
const CREATED_PROJECT_ID = "00000000-0000-4000-8000-000000000108";
const RESTORED_PROJECT_ID = "00000000-0000-4000-8000-000000000109";
const REPOSITORY_ID = "00000000-0000-4000-8000-000000000102";
const LOCAL_REPOSITORY_ID = "00000000-0000-4000-8000-000000000112";
const USER_PACKAGE_ID = "00000000-0000-4000-8000-000000000113";
const INSTALLATION_ID = "00000000-0000-4000-8000-000000000103";
const TEMPLATE_ID = "com.cqmhv.template.avatar";
const BACKUP_ID = "00000000-0000-4000-8000-000000000104";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";
const PLAN_ID = "00000000-0000-4000-8000-000000000106";
const DISCORD_ID = "com.cqmhv.discord";
const MCP_ID = "com.cqmhv.mcp-management";
const HASH = "a".repeat(64);

function project(projectId = PROJECT_ID, name = "Sample", favorite = false) {
    const rootPath = projectId === PROJECT_ID && name === "Sample" ? "<private-project>" : `C:\\Fixture\\${name}`;
    return { projectId, registeredAtMs: 1_690_000_000_000, favorite, rootPath, projectType: "avatars", unityVersion: "2022.3.22f1", vpmManifest: "valid", upmManifest: "valid", directDependencies: [{ packageId: "com.example.avatar", value: "^1.2.0" }], lockedDependencies: [{ packageId: "com.example.avatar", value: "1.2.3" }], issues: [], observedAtMs: 1_700_000_000_000, revision: 2 };
}
function repository() { return { repositoryId: REPOSITORY_ID, source: { kind: "remote" as const, url: "https://packages.example.invalid/index.json" }, declaredId: "example", name: "Example packages", declaredUrl: "https://packages.example.invalid/index.json", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function localRepository() { return { repositoryId: LOCAL_REPOSITORY_ID, source: { kind: "local" as const, path: "<private-local-repository>" }, declaredId: "local-example", name: "Local packages", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function userPackage(revision = 1) { return { userPackageId: USER_PACKAGE_ID, sourceRootPath: "<private-user-package>", packageId: "com.example.avatar", version: "1.2.3", displayName: "Local avatar tools", revision, archiveSha256: HASH, createdAtMs: 1_700_000_000_000, updatedAtMs: revision === 1 ? 1_700_000_000_000 : 1_700_000_001_000 }; }
function installation() { return { installationId: INSTALLATION_ID, executablePath: "<private-editor>", filesystemIdentity: "opaque", unityVersion: "2022.3.22f1", architecture: "x86_64", sourceKind: "manual", revision: 2, observedAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 }; }
function editorPreference() { return { projectId: PROJECT_ID, installationId: INSTALLATION_ID, arguments: [], revision: 2, updatedAtMs: 1_700_000_000_000 }; }
function editorSelection() { return { projectId: PROJECT_ID, selection: { mode: "explicit" as const, installationId: INSTALLATION_ID }, arguments: ["-logFile"], revision: 2, updatedAtMs: 1_700_000_000_000 }; }
function launch() { return { launchId: "00000000-0000-4000-8000-000000000107", projectId: PROJECT_ID, installationId: INSTALLATION_ID, state: "spawned", spawnAccepted: true, createdAtMs: 1_700_000_000_000 }; }
function template() { return { templateId: TEMPLATE_ID, sourceKind: "built-in", templateVersion: "1.0.0", displayName: "Avatar starter", description: "A deterministic public fixture.", provenance: "built-in", favorite: false, bundleSha256: HASH, manifestFingerprint: HASH, revision: 2, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 }; }
function backup() { return { backupId: BACKUP_ID, sourceProjectId: PROJECT_ID, archiveSha256: HASH, archiveBytes: 4096, formatVersion: 1, createdAtMs: 1_700_000_000_000, compressionMode: "fast", excludeVpmPackages: true }; }
function runningOperation(): Operation { return { operationId: OPERATION_ID, kind: "packages.apply", state: "running", revision: 3, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000, progress: { phase: "extracting" } }; }
function packagePlan(action: PackagePlan["action"]): PackagePlan { return { planId: PLAN_ID, action, state: "unapplied", projectId: PROJECT_ID, projectRevision: 2, changeSetFingerprint: HASH, changeSet: { formatVersion: 1, mutations: [{ kind: action === "remove" ? "remove" : "install", packageId: "com.example.avatar", ...(action === "remove" ? { fromVersion: "1.2.3" } : { toVersion: "1.2.3" }) }], dependencyEdges: [], vpmManifestSha256: HASH } }; }
function templatePlan(action: string) { return { planId: PLAN_ID, action, state: "unapplied", planFingerprint: HASH }; }
function extension(extensionId: string): ExtensionRecord { return { extensionId, version: "1.0.0", apiMajor: 1, packageDigest: HASH, publisherFingerprint: `ed25519-sha256:${HASH}`, trustDecision: "official", desiredState: "enabled", quarantineState: "clear", runtimeState: "running", grantRevision: 3, lifecycleGeneration: 2, revision: 4, ui: { protocol: "portable-v1" } }; }
function extensionPlan(action: string) { return { planId: PLAN_ID, action, state: "unapplied", sourceKind: "local_owner_selected", extensionId: DISCORD_ID, version: "1.0.0", apiMajor: 1, profileVersion: 1, packageDigest: HASH, publisherFingerprint: `ed25519-sha256:${HASH}`, trustDecision: "official", dataDisposition: "retain_data", planFingerprint: HASH, uiProtocol: "portable-v1" }; }

const root = document.getElementById("root");
if (root === null) throw new Error("Missing #root element");
ReactDOM.createRoot(root).render(
    <React.StrictMode>
        {materialEvidence
            ? <MaterialFoundationEvidence />
            : <App client={new DeterministicGuiClient(mode)} />}
    </React.StrictMode>
);
