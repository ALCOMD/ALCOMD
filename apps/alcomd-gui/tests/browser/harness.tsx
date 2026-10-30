import type {
    CandidateChoice,
    PackageCandidateEvidence,
    PackageCandidateSummary,
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
import { RPC_CAPABILITIES } from "@alcomd/sdk";
import React from "react";
import ReactDOM from "react-dom/client";

import discordDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/discord-presence-snapshot.json" with { type: "json" };
import mcpDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/mcp-management-snapshot.json" with { type: "json" };
import { App } from "../../src/App";
import type { Operation, PackagePlan, ProjectSnapshot, SettingsGetResult } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";
import "../../src/styles.css";
import { MaterialFoundationEvidence } from "./MaterialFoundationEvidence";

declare global {
    interface Window {
        packageRequests: Array<{ method: string; params: unknown }>;
        projectSnapshots: ProjectSnapshot[][];
        unityRequests: Array<{ method: string; params: unknown }>;
        candidateRequests: Array<Parameters<GuiRpcClient["packageQueryProjectCandidates"]>[0]>;
        resumeCandidateSource?: () => void;
        settingsWrites: Array<{ expectedRevision: number; revision: number }>;
        settingsRequests: Array<{ expectedRevision: number; update: Parameters<GuiRpcClient["settingsUpdate"]>[1] }>;
        settingsReads: number;
        heldInitialSettingsReads: number;
        releasedInitialSettingsReads: number;
        capabilityLoadingScreens: string[];
    }
}
window.packageRequests = [];
window.projectSnapshots = [];
window.unityRequests = [];
window.candidateRequests = [];
window.settingsWrites = [];
window.settingsRequests = [];
window.settingsReads = 0;
window.heldInitialSettingsReads = 0;
window.releasedInitialSettingsReads = 0;

type HarnessMode = "ready" | "empty" | "error" | "disconnected" | "loading" | "stale" | "failed" | "cancelled" | "create-error" | "restore-error" | "favorite-pages" | "favorite-error" | "favorite-conflict" | "unity-automatic" | "unity-zero" | "unity-multiple" | "unity-migration" | "package-no-repositories" | "package-multiple" | "package-user-source" | "package-partial-failure" | "package-revision-conflict" | "package-filter-conflict" | "package-filter-denied" | "capabilities-missing";

const SUPPORTED_CAPABILITIES = [
    "backups.create.v1",
    "backups.read.v1",
    "backups.restore.v1",
    RPC_CAPABILITIES.extensionsLifecycle,
    RPC_CAPABILITIES.extensionsPermissions,
    "extensions.ui.portable.v1",
    "operations.v1",
    "packages.apply.v1",
    "packages.candidates.v1",
    "packages.plan.v1",
    "packages.plan.v2",
    "packages.user-packages.v1",
    "projects.copy.v1",
    "projects.delete.v1",
    "projects.read.v1",
    "projects.registry.v1",
    "projects.unity-migration.v1",
    "repositories.read.v1",
    "repositories.registry.v1",
    "state.check.v1",
    "templates.create-project.v1",
    "templates.manage.v1",
    "templates.read.v1",
    "unity.hub.open.v1",
    "unity.launch.v1",
    "unity.manage.v1",
    "unity.read.v1"
] as const;

const CAPABILITIES_WITH_OPTIONAL_ACTIONS_MISSING = [
    "backups.read.v1",
    RPC_CAPABILITIES.extensionsLifecycle,
    "projects.read.v1",
    "repositories.read.v1",
    "templates.read.v1",
    "unity.read.v1"
] as const;

const query = new URLSearchParams(window.location.search);
const initialRoute = query.get("route") ?? "/";
const mode = (query.get("state") ?? "ready") as HarnessMode;
const materialEvidence = query.get("material") === "1";
if (!materialEvidence) window.history.replaceState(null, "", initialRoute);

class DeterministicGuiClient implements GuiRpcClient {
    private candidateSourceWasDelayed = false;
    private hubDefaultReady = query.has("hub-default");
    private hubReadFailurePending = false;
    private repositoriesReady: Promise<void> | undefined;
    private settingsReady: Promise<void> | undefined;
    private themeSaveRejected = false;
    private unityReady: Promise<void> | undefined;
    private settings: SettingsGetResult = {
        configSchema: 3,
        unityHub: { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" },
        revision: 7,
        settings: {
            unity: { hubPath: null },
            appearance: { mode: "system", sourceColor: null, scheme: "vibrant", density: "default", motion: "system" },
            locale: "en-US",
            packages: {
                showPrerelease: false,
                hiddenRepositoryIds: [],
                hideLocalUserPackages: false
            }
        }
    };
    private operationReads = 0;
    private projectLaunchTimes = new Map<string, number>();
    private pendingProject?: ReturnType<typeof project>;
    private pendingDeleteProjectId?: string;
    private pendingMigrationTarget?: string;
    private deletedProjectIds = new Set<string>();
    private createdProjects: ReturnType<typeof project>[] = [];
    private favoriteConflictPending = true;
    private currentProjectRevision = 2;
    private currentUnityVersion = "2022.3.22f1";
    private launchArguments = ["-logFile"];
    private launchConfigRevision = 2;
    private snapshotRevision = 1;
    private repositoryRefreshCompleted = false;
    private repositoryRefreshAttempts = new Map<string, number>();
    private userPackageRemoved = false;
    private userPackageRevision = 1;

    private connectionRestored = false;

    constructor(private readonly mode: HarnessMode) {
        window.addEventListener("test-hub-default-ready", () => { this.hubDefaultReady = true; });
        window.addEventListener("test-hub-read-error", () => { this.hubReadFailurePending = true; });
        const hubPath = query.get("hub-path");
        if (hubPath !== null) {
            this.settings.settings.unity.hubPath = hubPath;
            this.settings.unityHub = { status: "available", source: "configured", path: hubPath, configuredPathStatus: "available" };
        }
        const motion = query.get("motion");
        if (motion === "system" || motion === "full" || motion === "reduced") this.settings.settings.appearance.motion = motion;
        const themeSource = query.get("theme-source");
        if (themeSource !== null && /^#[0-9a-f]{6}$/i.test(themeSource)) {
            this.settings.settings.appearance.sourceColor = themeSource;
        }
        const themeMode = query.get("theme-mode");
        if (themeMode === "light" || themeMode === "dark" || themeMode === "system") {
            this.settings.settings.appearance.mode = themeMode;
        }
        const themeScheme = query.get("theme-scheme");
        const schemes = ["tonal-spot", "fidelity", "monochrome", "neutral", "vibrant", "expressive", "content", "rainbow", "fruit-salad"];
        if (themeScheme !== null && schemes.includes(themeScheme)) {
            this.settings.settings.appearance.scheme = themeScheme as SettingsGetResult["settings"]["appearance"]["scheme"];
        }
        if (mode === "disconnected") {
            window.addEventListener("test-daemon-online", () => { this.connectionRestored = true; }, { once: true });
        }
    }

    private async value<T>(value: T): Promise<T> {
        if (this.mode === "loading") return new Promise<T>(() => undefined);
        if (this.mode === "error") throw { code: "internal_error", diagnosticId: "00000000-0000-4000-8000-000000000999" };
        if (this.mode === "disconnected" && !this.connectionRestored) throw { code: "daemon_unavailable" };
        return structuredClone(value);
    }

    async systemStatus(): ReturnType<GuiRpcClient["systemStatus"]> {
        if (query.has("slowStatus")) await new Promise<void>((resolve) => setTimeout(resolve, 120));
        return this.value({
            product: "ALCOMD",
            daemonVersion: "4.0.0-alpha.0",
            rpcVersion: 1,
            state: "ready",
            capabilities: this.mode === "capabilities-missing"
                ? [...CAPABILITIES_WITH_OPTIONAL_ACTIONS_MISSING]
                : [...SUPPORTED_CAPABILITIES].filter((capability) => !(query.get("noPlanV2") && capability === "packages.plan.v2") && !(query.has("noUnityManage") && capability === "unity.manage.v1") && !(query.has("noHubOpen") && capability === "unity.hub.open.v1"))
        });
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
        if (state === "succeeded" && this.pendingMigrationTarget !== undefined) {
            this.currentUnityVersion = this.pendingMigrationTarget;
            this.currentProjectRevision += 1;
            this.pendingMigrationTarget = undefined;
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

    private hubLocation(hubPath: string | null): SettingsGetResult["unityHub"] {
        if (query.has("hub-unavailable")) return { status: "unavailable", source: "none", path: null, configuredPathStatus: hubPath === null ? "unset" : "unavailable" };
        if (query.has("hub-missing") && hubPath === query.get("hub-path")) return { status: "not_found", source: "none", path: null, configuredPathStatus: "invalid" };
        return hubPath !== null ? { status: "available", source: "configured", path: hubPath, configuredPathStatus: "available" }
            : this.hubDefaultReady ? { status: "available", source: "discovered", path: "C:\\Program Files\\Unity Hub\\Unity Hub.exe", configuredPathStatus: "unset" }
                : { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" };
    }

    async settingsGet(): ReturnType<GuiRpcClient["settingsGet"]> {
        window.settingsReads += 1;
        if (this.hubReadFailurePending) { this.hubReadFailurePending = false; throw { code: "internal_error" }; }
        this.settings.unityHub = this.hubLocation(this.settings.settings.unity.hubPath);
        // Projects has no settings.get page consumer, so these are the shell's initial reads.
        // Capture the old snapshot before delaying, rather than returning a newer value on release.
        if (query.get("theme-delay-initial-read") === "1" && window.location.pathname === "/projects") {
            const snapshot = structuredClone(this.settings);
            window.heldInitialSettingsReads += 1;
            await new Promise<void>((resolve) => window.addEventListener("test-theme-initial-read-ready", () => resolve(), { once: true }));
            window.releasedInitialSettingsReads += 1;
            return this.value(snapshot);
        }
        if (query.has("holdSettings")) {
            this.settingsReady ??= new Promise<void>((resolve) => window.addEventListener("test-settings-ready", () => resolve(), { once: true }));
            await this.settingsReady;
        }
        return this.value(this.settings);
    }

    async settingsUpdate(expectedRevision: number, update: Parameters<GuiRpcClient["settingsUpdate"]>[1]): ReturnType<GuiRpcClient["settingsUpdate"]> {
        window.settingsRequests.push({ expectedRevision, update: structuredClone(update) });
        if (query.has("hub-reject-invalid") && update.unity?.hubPath?.includes("Invalid")) throw { code: "invalid_request" };
        if (query.get("theme-save") === "error") throw { code: "internal_error" };
        if (query.get("theme-save") === "error-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            throw { code: "internal_error" };
        }
        if (query.get("theme-save") === "conflict-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            this.settings.revision += 1;
            this.settings.settings.appearance.density = "compact";
            this.settings.settings.appearance.mode = "dark";
            this.settings.settings.appearance.scheme = "neutral";
            this.settings.settings.packages.showPrerelease = true;
            throw { code: "revision_conflict" };
        }
        if (query.get("theme-save") === "hold") {
            await new Promise<void>((resolve) => window.addEventListener("test-theme-save-ready", () => resolve(), { once: true }));
        }
        if (this.mode === "package-filter-conflict") return Promise.reject({ code: "revision_conflict" });
        if (this.mode === "package-filter-denied") return Promise.reject({ code: "permission_denied" });
        if (expectedRevision !== this.settings.revision) return Promise.reject({ code: "revision_conflict" });
        if (update.unity?.hubPath && query.has("hub-normalize")) update = { ...update, unity: { hubPath: "C:\\Normalized\\Unity Hub.exe" } };
        this.settings = {
            configSchema: 3,
            unityHub: this.hubLocation(update.unity?.hubPath === undefined ? this.settings.settings.unity.hubPath : update.unity.hubPath),
            revision: expectedRevision + 1,
            settings: {
                unity: { ...this.settings.settings.unity, ...update.unity },
                appearance: { ...this.settings.settings.appearance, ...update.appearance },
                locale: update.locale ?? this.settings.settings.locale,
                packages: { ...this.settings.settings.packages, ...update.packages }
            }
        };
        window.settingsWrites.push({ expectedRevision, revision: this.settings.revision });
        if (query.get("theme-save") === "commit-error-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            throw { code: "internal_error" };
        }
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
        if (query.has("projectModified")) {
            const { lastModifiedAtMs: _unknown, ...unknown } = project("00000000-0000-4000-8000-000000000104", "Unknown history");
            const projects = [
                { ...project("00000000-0000-4000-8000-000000000102", "Equal B"), lastModifiedAtMs: 1_700_086_400_000 },
                unknown,
                { ...project("00000000-0000-4000-8000-000000000103", "Newest"), lastModifiedAtMs: 1_700_172_800_000 },
                { ...project("00000000-0000-4000-8000-000000000101", "Equal A"), lastModifiedAtMs: 1_700_086_400_000 },
                { ...project("00000000-0000-4000-8000-000000000105", "Favorite", true), lastModifiedAtMs: 1_700_000_000_000 }
            ].map((item) => ({ ...item, ...(this.projectLaunchTimes.has(item.projectId) ? { lastModifiedAtMs: this.projectLaunchTimes.get(item.projectId) } : {}) }));
            window.projectSnapshots.push(structuredClone(projects));
            return this.value({ projects });
        }
        if (query.has("projectGrid")) {
            const names = ["Avatar Studio", "World Workshop", "非常长的项目名称用于检查卡片信息与操作是否保持整齐 Long Project Name", "Package Sandbox", "Unknown Editor", "Avatar Preview", "Archive"];
            return this.value({ projects: names.map((name, index) => ({
                ...project(`00000000-0000-4000-8000-${String(101 + index).padStart(12, "0")}`, name, index === 0),
                rootPath: `C:\\Projects\\Workspace with a long directory name\\${name}`,
                projectType: index === 1 ? "worlds" : index === 3 ? "Unknown project type with long text" : "avatars",
                unityVersion: index === 4 ? "" : index === 1 ? "6000.0.40f1" : "2022.3.22f1",
                registeredAtMs: index === 4 ? undefined : 1_690_000_000_000
            })) });
        }
        if (this.mode === "favorite-pages") {
            return cursor === undefined
                ? this.value({ projects: [project(PROJECT_ID, "Alpha")], nextCursor: { registeredAtMs: 1_690_000_000_000, id: PROJECT_ID } })
                : this.value({ projects: [project("00000000-0000-4000-8000-000000000110", "Zulu Favorite", true)] });
        }
        const projects = [project(), ...this.createdProjects].filter((candidate) => candidate.projectId === undefined || !this.deletedProjectIds.has(candidate.projectId));
        return this.value({ projects: this.mode === "empty" ? [] : projects });
    }
    projectGet(projectId: string): ReturnType<GuiRpcClient["projectGet"]> {
        const value = this.createdProjects.find((candidate) => candidate.projectId === projectId) ?? { ...project(projectId), unityVersion: this.currentUnityVersion, revision: this.currentProjectRevision };
        if (this.projectLaunchTimes.has(projectId)) value.lastModifiedAtMs = this.projectLaunchTimes.get(projectId)!;
        if (query.get("twoInstalled")) {
            value.lockedDependencies = [...value.lockedDependencies, { packageId: "com.example.remote", value: "1.2.3" }];
            value.directDependencies = [...value.directDependencies, { packageId: "com.example.remote", value: "^1.2.0" }];
        }
        if (query.get("bulk257")) {
            const dependencies = Array.from({ length: 257 }, (_, index) => ({ packageId: `com.example.batch${String(index).padStart(3, "0")}`, value: "1.2.3" }));
            value.lockedDependencies = dependencies;
            value.directDependencies = dependencies;
        }
        return this.value({ project: value });
    }
    openProjectDirectory(): ReturnType<GuiRpcClient["openProjectDirectory"]> { return this.value(undefined); }
    async unityHubOpen(expectedRevision: number, expectedPath: string | null): ReturnType<GuiRpcClient["unityHubOpen"]> {
        window.unityRequests.push({ method: "unityHubOpen", params: { expectedRevision, expectedPath } });
        if (query.get("hub-open") === "hold") await new Promise<void>((resolve) => window.addEventListener("test-hub-open-ready", () => resolve(), { once: true }));
        if (query.get("hub-open") === "stale") {
            this.settings.revision += 1;
            this.settings.settings.unity.hubPath = "C:\\Changed\\Unity Hub.exe";
            this.settings.unityHub = this.hubLocation(this.settings.settings.unity.hubPath);
            throw { code: "hub_path_changed" };
        }
        if (query.get("hub-open") === "gone") {
            this.hubDefaultReady = false;
            this.settings.unityHub = this.hubLocation(this.settings.settings.unity.hubPath);
            throw { code: "hub_path_changed" };
        }
        if (query.get("hub-open") === "failed") throw { code: "hub_open_failed" };
        if (query.get("hub-open") === "unknown") throw { code: "daemon_unavailable" };
        if (query.get("hub-open") === "denied") throw { code: "permission_denied" };
        return this.value({ outcome: this.settings.unityHub.status === "available" ? "spawned" : "download_required" });
    }
    async openUnityHubDownload(): ReturnType<GuiRpcClient["openUnityHubDownload"]> {
        window.unityRequests.push({ method: "openUnityHubDownload", params: {} });
        if (query.has("hub-download-error")) throw { code: "unity_hub_download_open_failed" };
    }
    async selectUnityHub(currentPath: string | null): ReturnType<GuiRpcClient["selectUnityHub"]> {
        window.unityRequests.push({ method: "selectUnityHub", params: { currentPath } });
        if (query.get("hub-picker") === "hold") await new Promise<void>((resolve) => window.addEventListener("test-hub-picker-ready", () => resolve(), { once: true }));
        return query.get("hub-picker") === "cancel" ? undefined : "C:\\Tools\\Unity Hub\\Unity Hub.exe";
    }
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
    async repositoriesList(cursor?: { registeredAtMs: number; id: string }): ReturnType<GuiRpcClient["repositoriesList"]> {
        if (query.has("holdRepositories")) {
            this.repositoriesReady ??= new Promise<void>((resolve, reject) => window.addEventListener("test-repositories-ready", (event) => {
                if (event instanceof CustomEvent && event.detail === "error") reject({ code: "daemon_unavailable" });
                else resolve();
            }, { once: true }));
            await this.repositoriesReady;
        }
        if (this.mode === "empty" || this.mode === "package-no-repositories") return this.value({ repositories: [] });
        if (["package-multiple", "package-partial-failure"].includes(this.mode) || query.get("twoInstalled")) {
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
        if (query.get("versions") === "1") return this.value({ packages: [
            { packageId: "com.example.avatar", version: "1.2.3", displayName: "Avatar tools", yanked: false, prerelease: false },
            { packageId: "com.example.avatar", version: "1.1.0", displayName: "Avatar tools", yanked: false, prerelease: false },
            { packageId: "com.example.avatar", version: "1.10.0", displayName: "Avatar tools", yanked: false, prerelease: false },
            { packageId: "com.example.avatar", version: "2.0.0-beta.1", displayName: "Avatar tools", yanked: false, prerelease: true },
            { packageId: "com.example.avatar", version: "9.0.0", displayName: "Avatar tools", yanked: true, prerelease: false },
            { packageId: "com.example.avatar", version: "legacy", displayName: "Avatar tools", yanked: false }
        ] });
        const packages = repositoryId === LOCAL_REPOSITORY_ID
            ? [{ packageId: "com.example.local", version: "2.0.0", displayName: "Local tools", yanked: false }]
            : [
                { packageId: "com.example.avatar", version: "1.2.3", displayName: "Avatar tools", yanked: false, unity: ">=2022.3" },
                { packageId: "com.example.avatar", version: "1.3.0", displayName: "Avatar tools", yanked: false, unity: ">=2022.3" },
                { packageId: "com.example.remote", version: "1.0.0", displayName: "Remote tools", yanked: false },
                ...(this.repositoryRefreshCompleted ? [{ packageId: "com.example.refreshed", version: "1.0.0", displayName: "Refreshed package", yanked: false }] : [])
            ];
        return this.value({ packages: packages.map((item) => ({ ...item, prerelease: false })) });
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

    async packageQueryProjectCandidates(params: Parameters<GuiRpcClient["packageQueryProjectCandidates"]>[0]): ReturnType<GuiRpcClient["packageQueryProjectCandidates"]> {
        window.candidateRequests.push(params);
        if (query.get("delaySource") && !this.candidateSourceWasDelayed && params.view.kind === "summary" && params.view.sources?.length) {
            this.candidateSourceWasDelayed = true;
            await new Promise<void>((resolve) => { window.resumeCandidateSource = resolve; });
        }
        const fingerprint = String(this.settings.revision).padStart(64, "0");
        if (query.get("candidateError")) throw { code: query.get("candidateError") };
        if (params.expectedSnapshot && params.expectedSnapshot !== fingerprint) throw { code: "package_candidate_evidence_stale" };
        if (query.get("versionsStale") && params.view.kind === "versions") throw { code: "package_candidate_evidence_stale" };
        if (query.get("intentStale") && params.limit === 1) throw { code: "package_candidate_evidence_stale" };
        const base = { projectId: params.projectId, snapshot: { fingerprint, projectRevision: params.expectedRevision, configRevision: this.settings.revision }, showPrerelease: this.settings.settings.packages.showPrerelease, catalogComplete: query.get("catalogIncomplete") !== "1", nextCursor: null };
        const candidate = (version: string, relation: PackageCandidateEvidence["relation"], sourceId = REPOSITORY_ID, reasons: PackageCandidateEvidence["reasons"] = [], classification: PackageCandidateEvidence["classification"] = "stable"): PackageCandidateEvidence => ({ version, relation, source: sourceId === USER_PACKAGE_ID ? { kind: "user_package", user_package_id: sourceId } : { kind: "repository", repository_id: sourceId }, sourceRevision: 2, classification, unity: "compatible", eligibility: reasons.length ? "blocked" : "eligible", reasons });
        if (params.view.kind === "versions") {
            const providers = this.mode === "package-user-source" ? [REPOSITORY_ID, USER_PACKAGE_ID] : this.mode === "package-multiple" ? [REPOSITORY_ID, LOCAL_REPOSITORY_ID] : [REPOSITORY_ID];
            const items = providers.flatMap((id) => [
                candidate("9.0.0", "newer", id, ["all_yanked"]),
                candidate("2.0.0-beta.1", "newer", id, this.settings.settings.packages.showPrerelease ? [] : ["prerelease_excluded"], "prerelease"),
                candidate("1.10.0", "newer", id),
                candidate("1.2.3", "same_precedence", id),
                candidate("1.1.0", "older", id),
                candidate("legacy", "unknown", id, ["classification_unknown", "metadata_invalid"], "unknown")
            ]);
            const offset = params.cursor?.offset ?? 0;
            const count = query.get("paged") ? 3 : items.length;
            return this.value({ ...base, nextCursor: offset + count < items.length ? { snapshotFingerprint: fingerprint, queryFingerprint: HASH, offset: offset + count } : null, view: "versions", packageId: params.view.packageId, items: items.slice(offset, offset + count) });
        }
        const visibleRemote = !this.settings.settings.packages.hiddenRepositoryIds.includes(REPOSITORY_ID) && this.mode !== "package-no-repositories";
        const visibleLocal = (["package-multiple", "package-partial-failure"].includes(this.mode) || Boolean(query.get("twoInstalled"))) && !this.settings.settings.packages.hiddenRepositoryIds.includes(LOCAL_REPOSITORY_ID);
        const ids = params.view.packageIds ?? (query.get("bulk257") ? Array.from({ length: 257 }, (_, index) => `com.example.batch${String(index).padStart(3, "0")}`) : ["com.example.avatar", ...(visibleRemote ? ["com.example.remote", ...(this.repositoryRefreshCompleted ? ["com.example.refreshed"] : [])] : []), ...(visibleLocal ? ["com.example.local"] : [])]);
        const items: PackageCandidateSummary[] = ids.map((packageId) => {
            const installed = packageId === "com.example.avatar" || packageId.startsWith("com.example.batch") || (Boolean(query.get("twoInstalled")) && packageId === "com.example.remote");
            const override = params.view.kind === "summary" ? params.view.sources?.find((item) => item.packageId === packageId)?.source : undefined;
            const target = candidate(installed ? "1.10.0" : "1.0.0", installed ? "newer" : "not_installed", packageId === "com.example.local" ? LOCAL_REPOSITORY_ID : REPOSITORY_ID);
            if (override) target.source = override;
            let choice: CandidateChoice = !visibleRemote && packageId !== "com.example.local" && !override ? { kind: "none", reasons: ["no_visible_candidate"] } : { kind: "candidate", candidate: target };
            if (query.get("ambiguous") && installed && !override) choice = { kind: "ambiguous", reason: "source_ambiguous" };
            if (query.get("unknown") && installed) choice = { kind: "unknown", reasons: ["project_unity_unknown"] };
            if (query.get("mixedInvalid") && packageId === "com.example.remote") choice = { kind: "unknown", reasons: ["classification_unknown"] };
            if (query.get("noUpdate") && installed) target.relation = "same_precedence";
            const providers: PackageCandidateSummary["providers"] = [];
            if (visibleRemote && packageId !== "com.example.local") providers.push({ source: { kind: "repository", repository_id: REPOSITORY_ID }, sourceRevision: 2 });
            if (visibleLocal && (packageId === "com.example.local" || (packageId === "com.example.remote" && query.get("twoInstalled")) || (installed && (query.get("versions") || query.get("ambiguous"))))) providers.push({ source: { kind: "repository", repository_id: LOCAL_REPOSITORY_ID }, sourceRevision: 2 });
            if (packageId === "com.example.avatar" && this.mode === "package-user-source" && !this.settings.settings.packages.hideLocalUserPackages) providers.push({ source: { kind: "user_package", user_package_id: USER_PACKAGE_ID }, sourceRevision: 2 });
            let update: PackageCandidateSummary["update"] = choice.kind === "candidate" ? installed ? query.get("noUpdate") ? { kind: "no_update", reason: "same_precedence" } : { kind: "target", candidate: target } : { kind: "no_update", reason: "not_installed" } : { kind: "unavailable", reasons: choice.kind === "ambiguous" ? [choice.reason] : choice.reasons };
            if (query.get("knownExclusion") && packageId === "com.example.remote") { target.relation = "older"; update = { kind: "no_update", reason: "installed_newer" }; }
            const stableCandidate = { ...target };
            if (query.get("stableAlternate") && installed) { target.version = "2.0.0-beta.1"; target.classification = "prerelease"; }
            const stableChoice: CandidateChoice = query.get("stableAlternate") && installed ? { kind: "candidate", candidate: stableCandidate } : choice;
            const stableUpdate: PackageCandidateSummary["update"] = query.get("stableAlternate") && installed ? { kind: "target", candidate: stableCandidate } : update;
            return { packageId, providers, direct: installed && query.get("transitive") !== "1", installed: installed ? { kind: "locked", version: "1.2.3" } : { kind: "absent" }, latest: choice, latestStable: stableChoice, projectLatest: choice, projectLatestStable: stableChoice, update, stableUpdate };
        });
        const offset = params.cursor?.offset ?? 0;
        const count = query.get("paged") ? 1 : params.limit ?? 64;
        return this.value({ ...base, nextCursor: offset + count < items.length ? { snapshotFingerprint: fingerprint, queryFingerprint: HASH, offset: offset + count } : null, view: "summary", items: items.slice(offset, offset + count) });
    }
    packagePlanInstall(params: Parameters<GuiRpcClient["packagePlanInstall"]>[0]): ReturnType<GuiRpcClient["packagePlanInstall"]> {
        window.packageRequests.push({ method: "install", params });
        if (query.get("planError") === "1") return Promise.reject({ code: "package_source_ambiguous" });
        const plan = packagePlan("install");
        plan.changeSet.mutations = [{ kind: "replace", packageId: params.packageId, fromVersion: "1.2.3", toVersion: params.versionRange ?? "1.3.0" }];
        return this.value(plan);
    }
    packagePlanRemove(): ReturnType<GuiRpcClient["packagePlanRemove"]> { return this.value(packagePlan("remove")); }
    packagePlanUpgrade(params: Parameters<GuiRpcClient["packagePlanUpgrade"]>[0]): ReturnType<GuiRpcClient["packagePlanUpgrade"]> {
        window.packageRequests.push({ method: "upgrade", params });
        if (query.get("planError") === "1") return Promise.reject({ code: "package_source_ambiguous" });
        const plan = packagePlan("upgrade");
        plan.changeSet.mutations = [{ kind: "replace", packageId: params.packageId, fromVersion: "1.2.3", toVersion: params.versionRange?.replace(/^=/, "") ?? "1.10.0" }];
        return this.value(plan);
    }
    packagePlanDowngrade(params: Parameters<GuiRpcClient["packagePlanDowngrade"]>[0]): ReturnType<GuiRpcClient["packagePlanDowngrade"]> {
        window.packageRequests.push({ method: "downgrade", params });
        const plan = packagePlan("downgrade");
        plan.changeSet.mutations = [{ kind: "replace", packageId: params.packageId, fromVersion: "1.2.3", toVersion: params.version }];
        return this.value(plan);
    }
    packagePlanResolve(): ReturnType<GuiRpcClient["packagePlanResolve"]> { return this.value(packagePlan("resolve")); }
    packagePlanReinstall(params: Parameters<GuiRpcClient["packagePlanReinstall"]>[0]): ReturnType<GuiRpcClient["packagePlanReinstall"]> { window.packageRequests.push({ method: "reinstall", params }); return this.value(packagePlan("reinstall")); }
    packagePlanBulk(params: Parameters<GuiRpcClient["packagePlanBulk"]>[0]): ReturnType<GuiRpcClient["packagePlanBulk"]> {
        window.packageRequests.push({ method: "bulk", params });
        const plan = packagePlan("bulk");
        plan.changeSet.mutations = params.intents.map((intent) => ({ kind: intent.kind === "remove" ? "remove" : "replace", packageId: intent.packageId, fromVersion: "1.2.3", toVersion: intent.kind === "remove" ? null : "1.2.3" }));
        return this.value(plan);
    }
    packageApplyPlan(): ReturnType<GuiRpcClient["packageApplyPlan"]> {
        window.packageRequests.push({ method: "apply", params: {} });
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

    async unityInstallationsList(): ReturnType<GuiRpcClient["unityInstallationsList"]> {
        if (query.has("holdUnity")) {
            this.unityReady ??= new Promise<void>((resolve) => window.addEventListener("test-unity-ready", () => resolve(), { once: true }));
            await this.unityReady;
        }
        const installations = this.mode === "empty" || this.mode === "unity-zero"
            ? []
            : this.mode === "unity-multiple"
                ? [installation(), { ...installation(), installationId: "00000000-0000-4000-8000-000000000111", filesystemIdentity: "opaque-two", executablePath: "<private-editor-two>" }]
                : this.mode === "unity-migration"
                    ? [installation(), migrationInstallation()]
                : [installation()];
        return this.value({ installations, replayed: false });
    }
    unityInstallationGet(): ReturnType<GuiRpcClient["unityInstallationGet"]> { return this.value({ installation: installation(), replayed: false }); }
    unityInstallationRegister(): ReturnType<GuiRpcClient["unityInstallationRegister"]> { return this.value({ installation: installation(), replayed: false }); }
    unityInstallationRemove(): ReturnType<GuiRpcClient["unityInstallationRemove"]> { return this.value({ installationId: INSTALLATION_ID, removed: true, replayed: false }); }
    unityInstallationsRefresh(): ReturnType<GuiRpcClient["unityInstallationsRefresh"]> {
        window.unityRequests.push({ method: "unityInstallationsRefresh", params: {} });
        if (query.get("hub-refresh") === "error") return Promise.reject({ code: "internal_error" });
        return this.value({ installations: [installation()], replayed: false });
    }
    unityProjectLaunchConfigGet(): ReturnType<GuiRpcClient["unityProjectLaunchConfigGet"]> {
        return this.value({ config: launchConfig(this.launchArguments, this.launchConfigRevision) });
    }
    unityProjectLaunchConfigSet(_projectId: string, arguments_: string[]): ReturnType<GuiRpcClient["unityProjectLaunchConfigSet"]> {
        this.launchArguments = arguments_;
        this.launchConfigRevision += 1;
        return this.value({ config: launchConfig(this.launchArguments, this.launchConfigRevision), changed: true, replayed: false });
    }
    unityProjectLaunchConfigClear(): ReturnType<GuiRpcClient["unityProjectLaunchConfigClear"]> {
        this.launchArguments = [];
        this.launchConfigRevision += 1;
        return this.value({ config: launchConfig(this.launchArguments, this.launchConfigRevision), changed: true, replayed: false });
    }
    unityWriterState(): ReturnType<GuiRpcClient["unityWriterState"]> { return this.value({ projectId: PROJECT_ID, state: "not_observed", evidence: [], checkedAtMs: 1_700_000_000_000 }); }
    unityLaunchOptions(projectId: string, expectedProjectRevision: number): ReturnType<GuiRpcClient["unityLaunchOptions"]> {
        window.unityRequests.push({ method: "unity.launchOptions", params: { projectId, expectedProjectRevision } });
        if (query.has("unityOptionsError")) return Promise.reject({ code: "revision_conflict" });
        const exactMatchingInstallations = this.mode === "unity-zero"
            ? []
            : this.mode === "unity-multiple"
                ? [installation(), { ...installation(), installationId: "00000000-0000-4000-8000-000000000111", filesystemIdentity: "opaque-two", executablePath: "<private-editor-two>" }]
                : this.currentUnityVersion === migrationInstallation().unityVersion
                    ? [migrationInstallation()]
                    : [installation()];
        return this.value({ projectId, projectRevision: this.currentProjectRevision, projectUnityVersion: this.currentUnityVersion, exactMatchingInstallations });
    }
    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number): ReturnType<GuiRpcClient["unityLaunch"]> {
        window.unityRequests.push({ method: "unity.launch", params: { projectId, installationId, expectedProjectRevision } });
        if (query.has("projectModified")) this.projectLaunchTimes.set(projectId, 1_700_216_000_000);
        if (query.has("unityLaunchError")) return Promise.reject({ code: "launch_failed" });
        return new Promise((resolve) => window.setTimeout(() => resolve({ launch: launch(), replayed: false }), 100));
    }
    unityLaunchStatus(): ReturnType<GuiRpcClient["unityLaunchStatus"]> { return this.value({ launch: launch(), replayed: false }); }
    projectPlanUnityMigration(_projectId: string, targetInstallationId: string): ReturnType<GuiRpcClient["projectPlanUnityMigration"]> {
        window.unityRequests.push({ method: "projects.planUnityMigration", params: { projectId: _projectId, targetInstallationId } });
        const targetUnityVersion = targetInstallationId === MIGRATION_INSTALLATION_ID ? "2022.3.23f1" : "2022.3.22f1";
        return this.value({ kind: "planned", replayed: false, plan: { planId: PLAN_ID, projectId: PROJECT_ID, sourceUnityVersion: "2022.3.22f1", targetUnityVersion, targetInstallationId, classification: { kind: "patch_or_minor_upgrade", supportedForApply: true }, expiresAtMs: 1_800_000_000_000 } });
    }
    projectApplyUnityMigration(): ReturnType<GuiRpcClient["projectApplyUnityMigration"]> {
        this.pendingMigrationTarget = "2022.3.23f1";
        this.operationReads = 0;
        return this.value({ operationId: OPERATION_ID, replayed: false });
    }

    async templatesList(): ReturnType<GuiRpcClient["templatesList"]> {
        if (query.has("holdTemplates")) await new Promise<void>((resolve) => window.addEventListener("test-templates-ready", () => resolve(), { once: true }));
        return this.value({ templates: this.mode === "empty" ? [] : [template()] });
    }
    templateGet(): ReturnType<GuiRpcClient["templateGet"]> { return this.value({ template: template(), replayed: false }); }
    templateInspectBundle(): ReturnType<GuiRpcClient["templateInspectBundle"]> { return this.value({ formatVersion: 1, templateId: TEMPLATE_ID, templateVersion: "1.0.0", displayName: "Avatar starter", provenance: "public-fixture", bundleSha256: HASH, manifestFingerprint: HASH, payloadTreeSha256: HASH, entryCount: 4, totalBytes: 4096 }); }
    templatePlanImport(): ReturnType<GuiRpcClient["templatePlanImport"]> { return this.value(templatePlan("import")); }
    templateApplyImport(): ReturnType<GuiRpcClient["templateApplyImport"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    templatePlanDerive(): ReturnType<GuiRpcClient["templatePlanDerive"]> { return this.value(templatePlan("derive")); }
    templateApplyDerive(): ReturnType<GuiRpcClient["templateApplyDerive"]> { return this.value({ operationId: OPERATION_ID, replayed: false }); }
    templateExport(): ReturnType<GuiRpcClient["templateExport"]> { return this.value({ exported: true }); }
    templateSetFavorite(): ReturnType<GuiRpcClient["templateSetFavorite"]> { return this.value({ template: { ...template(), favorite: true }, replayed: false }); }
    templateRemove(): ReturnType<GuiRpcClient["templateRemove"]> { return this.value({ templateId: TEMPLATE_ID, removed: true, replayed: false }); }
    async templatePlanCreateProject(_templateId: string, _expectedTemplateRevision: number, _targetParent: string, targetLeaf: string): ReturnType<GuiRpcClient["templatePlanCreateProject"]> {
        if (query.has("holdCreatePlan")) await new Promise<void>((resolve) => window.addEventListener("test-create-plan-ready", () => resolve(), { once: true }));
        if (this.mode === "create-error") return Promise.reject({ code: "template_plan_stale" });
        this.pendingProject = project(CREATED_PROJECT_ID, targetLeaf);
        return this.value({ ...templatePlan("create-project"), targetLeaf });
    }
    templateApplyCreateProject(): ReturnType<GuiRpcClient["templateApplyCreateProject"]> { this.operationReads = 0; return this.value({ operationId: OPERATION_ID, replayed: false }); }

    async backupsList(): ReturnType<GuiRpcClient["backupsList"]> {
        if (query.has("holdBackups")) await new Promise<void>((resolve) => window.addEventListener("test-backups-ready", () => resolve(), { once: true }));
        return this.value({ backups: this.mode === "empty" ? [] : [backup()] });
    }
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
const MIGRATION_INSTALLATION_ID = "00000000-0000-4000-8000-000000000114";
const TEMPLATE_ID = "com.cqmhv.template.avatar";
const BACKUP_ID = "00000000-0000-4000-8000-000000000104";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";
const PLAN_ID = "00000000-0000-4000-8000-000000000106";
const DISCORD_ID = "com.cqmhv.discord";
const MCP_ID = "com.cqmhv.mcp-management";
const HASH = "a".repeat(64);

function project(projectId = PROJECT_ID, name = "Sample", favorite = false) {
    const rootPath = projectId === PROJECT_ID && name === "Sample" ? "<private-project>" : `C:\\Fixture\\${name}`;
    return { projectId, registeredAtMs: 1_690_000_000_000, lastModifiedAtMs: 1_700_000_000_000, favorite, rootPath, projectType: "avatars", unityVersion: "2022.3.22f1", vpmManifest: "valid", upmManifest: "valid", directDependencies: [{ packageId: "com.example.avatar", value: "^1.2.0" }], lockedDependencies: [{ packageId: "com.example.avatar", value: "1.2.3" }], issues: [], revision: 2 };
}
function repository() { return { repositoryId: REPOSITORY_ID, source: { kind: "remote" as const, url: "https://packages.example.invalid/index.json" }, declaredId: "example", name: "Example packages", declaredUrl: "https://packages.example.invalid/index.json", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function localRepository() { return { repositoryId: LOCAL_REPOSITORY_ID, source: { kind: "local" as const, path: "<private-local-repository>" }, declaredId: "local-example", name: "Local packages", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function userPackage(revision = 1) { return { userPackageId: USER_PACKAGE_ID, sourceRootPath: "<private-user-package>", packageId: "com.example.avatar", version: "1.2.3", displayName: "Local avatar tools", revision, archiveSha256: HASH, createdAtMs: 1_700_000_000_000, updatedAtMs: revision === 1 ? 1_700_000_000_000 : 1_700_000_001_000 }; }
function installation() { return { installationId: INSTALLATION_ID, executablePath: "<private-editor>", filesystemIdentity: "opaque", unityVersion: "2022.3.22f1", architecture: "x86_64", sourceKind: "manual", revision: 2, observedAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 }; }
function migrationInstallation() { return { ...installation(), installationId: MIGRATION_INSTALLATION_ID, executablePath: "<private-editor-migration>", filesystemIdentity: "opaque-migration", unityVersion: "2022.3.23f1" }; }
function launchConfig(arguments_: string[], revision: number) { return { projectId: PROJECT_ID, arguments: arguments_, revision, updatedAtMs: 1_700_000_000_000 }; }
function launch() { return { launchId: "00000000-0000-4000-8000-000000000107", projectId: PROJECT_ID, installationId: INSTALLATION_ID, state: "opening", spawnAccepted: true, createdAtMs: 1_700_000_000_000 }; }
function template() { return { templateId: TEMPLATE_ID, sourceKind: "builtin", templateVersion: "1.0.0", displayName: "Avatar starter", description: "A deterministic public fixture.", provenance: "built-in", favorite: false, bundleSha256: HASH, manifestFingerprint: HASH, revision: 2, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 }; }
function backup() { return { backupId: BACKUP_ID, sourceProjectId: PROJECT_ID, archiveSha256: HASH, archiveBytes: 4096, formatVersion: 1, createdAtMs: 1_700_000_000_000, compressionMode: "fast", excludeVpmPackages: true }; }
function runningOperation(): Operation { return { operationId: OPERATION_ID, kind: "packages.apply", state: "running", revision: 3, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000, progress: { phase: "extracting" } }; }
function packagePlan(action: PackagePlan["action"]): PackagePlan { return { planId: PLAN_ID, action, state: "unapplied", projectId: PROJECT_ID, projectRevision: 2, changeSetFingerprint: HASH, changeSet: { formatVersion: 1, mutations: [{ kind: action === "remove" ? "remove" : "install", packageId: "com.example.avatar", ...(action === "remove" ? { fromVersion: "1.2.3", toVersion: null } : { fromVersion: null, toVersion: "1.2.3" }) }], dependencyEdges: [], vpmManifestSha256: HASH } }; }
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
