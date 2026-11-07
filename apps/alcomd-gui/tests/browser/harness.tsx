import type { ProjectTemplateRecord } from "@alcomd/sdk";
import { browserCoreEvents, withBrowserCoreEvents } from "./core-events-client";
import type {
    CandidateBulkAction,
    CandidateBulkActions,
    CandidateBulkIntent,
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
import type { LogsReadParams, LogsReadResult, OperationLogEntry } from "@alcomd/sdk";
import { RPC_CAPABILITIES, type RepositoryListImportItem } from "@alcomd/sdk";
import React from "react";
import ReactDOM from "react-dom/client";

import discordDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/discord-presence-snapshot.json" with { type: "json" };
import mcpDocumentJson from "../../../../crates/alcomd-testing/fixtures/m7/mcp-management-snapshot.json" with { type: "json" };
import { App } from "../../src/App";
import type { Operation, PackagePlan, ProjectListView, ProjectsListResult, ProjectSnapshot, RegistryCursor, RepositorySnapshot, SettingsGetResult } from "../../src/core-models";
import { queryCandidates } from "../../src/package-candidates";
import type { FileDrag, GuiRpcClient, NativeFileKind } from "../../src/rpc";
import type { GuiUnityWindows, UnityWindowSnapshot, UnityWindowState, UnityWindowTarget } from "../../src/unity-window-observation";
import "../../src/styles.css";
import { MaterialFoundationEvidence } from "./MaterialFoundationEvidence";
import { OperationLogEvidence } from "./OperationLogEvidence";
import { withH4PortableClient } from "./h4-portable-client";
import { withH4ManagementClient } from "./h4-management-client";
import { withH5UtilitiesClient } from "./h5-utilities-client";
import { withH5SettingsClient } from "./h5-settings-client";

type CandidateQuery = (client: Pick<GuiRpcClient, "packageQueryProjectCandidates">, request: Parameters<typeof queryCandidates>[1]) => ReturnType<typeof queryCandidates>;

declare global {
    interface Window {
        unityWindowEvidence?: BrowserUnityWindows;
        fixtureCore: GuiRpcClient;
        fixtureLogs: { entries: OperationLogEntry[]; reads: LogsReadParams[]; replace(entries: OperationLogEntry[]): void; fail: boolean };
        h3ReadError?: string;
        queryCandidates: CandidateQuery;
        packageRequests: Array<{ method: string; params: unknown }>;
        projectSnapshots: ProjectSnapshot[][];
        autoDiscoveryProjects: ProjectSnapshot[];
        projectListReads: number;
        resumeProjectPoll?: () => void;
        unityRequests: Array<{ method: string; params: unknown }>;
        candidateRequests: Array<Parameters<GuiRpcClient["packageQueryProjectCandidates"]>[0]>;
        repositoryPackageRequests: Array<{ repositoryId: string; cursor?: Parameters<GuiRpcClient["repositoryPackages"]>[1] }>;
        templateRequests: Array<{ method: string; params: unknown }>;
        resourceRequests: Array<{ method: string; params: unknown }>;
        h3Requests: Array<{ method: string; params: Record<string, unknown> }>;
        h3Operation: Operation;
        templatePageCompletions: number;
        heldTemplateReads: number;
        resumeCandidateSource?: () => void;
        resumeCandidateVersions?: () => void;
        resumeCandidateSummary?: () => void;
        resumeCandidateFilter?: () => void;
        resumePackagePlan?: () => void;
        settingsWrites: Array<{ expectedRevision: number; revision: number }>;
        settingsRequests: Array<{ expectedRevision: number; update: Parameters<GuiRpcClient["settingsUpdate"]>[1] }>;
        settingsReads: number;
        heldSettingsReads: number;
        releasedSettingsReads: number;
        heldInitialSettingsReads: number;
        releasedInitialSettingsReads: number;
        capabilityLoadingScreens: string[];
    }
}
// The concurrency probe supplies only the read method used by the shared function.
window.queryCandidates = queryCandidates as CandidateQuery;
window.packageRequests = [];
window.fixtureLogs = {
    entries: [], reads: [], fail: false,
    replace(entries) {
        this.entries = structuredClone(entries);
        browserCoreEvents.invalidateOperationLog();
    }
};
window.projectSnapshots = [];
window.autoDiscoveryProjects = [];
const discovered = window.autoDiscoveryProjects;
window.autoDiscoveryProjects = new Proxy(discovered, {
    set(target, property, value) {
        const changed = Reflect.set(target, property, value);
        if (property !== "length" && value && typeof value.projectId === "string") {
            queueMicrotask(() => browserCoreEvents.emit("project", value.projectId, "project.registered", value.revision));
        }
        return changed;
    }
});
window.projectListReads = 0;
window.unityRequests = [];
window.candidateRequests = [];
window.repositoryPackageRequests = [];
window.templateRequests = [];
window.resourceRequests = [];
window.h3Requests = [];
window.h3Operation = { operationId: "00000000-0000-4000-8000-000000000105", kind: "backups.create", state: "running", revision: 3, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000, progress: { phase: "archiving" } };
let h3Value = window.h3Operation;
let h3PublishedRevision = h3Value.revision;
let h3PublishedState = h3Value.state;
let h3Queued = false;
const publishH3 = () => {
    if (h3Queued) return;
    h3Queued = true;
    queueMicrotask(() => {
        h3Queued = false;
        if (h3Value.revision <= h3PublishedRevision) h3Value.revision = h3PublishedRevision + 1;
        const kind = h3Value.state === h3PublishedState ? "operation.progress" : `operation.${h3Value.state}`;
        h3PublishedRevision = h3Value.revision; h3PublishedState = h3Value.state;
        browserCoreEvents.emit("operation", h3Value.operationId, kind, h3Value.revision);
    });
};
Object.defineProperty(window, "h3Operation", {
    configurable: true,
    get: () => h3Value,
    set: (value: Operation) => { h3Value = value; publishH3(); }
});
window.templatePageCompletions = 0;
window.heldTemplateReads = 0;
window.settingsWrites = [];
window.settingsRequests = [];
window.settingsReads = 0;
window.heldSettingsReads = 0;
window.releasedSettingsReads = 0;
window.heldInitialSettingsReads = 0;
window.releasedInitialSettingsReads = 0;

type HarnessMode = "ready" | "empty" | "error" | "disconnected" | "loading" | "stale" | "failed" | "cancelled" | "create-error" | "restore-error" | "favorite-pages" | "favorite-error" | "favorite-conflict" | "unity-automatic" | "unity-zero" | "unity-multiple" | "unity-migration" | "package-no-repositories" | "package-multiple" | "package-user-source" | "package-partial-failure" | "package-revision-conflict" | "package-filter-conflict" | "package-filter-denied" | "capabilities-missing";

const SUPPORTED_CAPABILITIES = [
    RPC_CAPABILITIES.logsRead,
    "backups.create.v1",
    "backups.read.v1",
    "backups.restore.v1",
    RPC_CAPABILITIES.extensionsLifecycle,
    RPC_CAPABILITIES.extensionsPermissions,
    RPC_CAPABILITIES.extensionsPermissionsRead,
    "extensions.ui.portable.v1",
    "operations.v1",
    "events.live.v1",
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
    RPC_CAPABILITIES.repositoriesListFiles,
    RPC_CAPABILITIES.repositoriesListImport,
    "state.check.v1",
    "projects.create.v1",
    "projectTemplates.v2",
    "unity.hub.open.v1",
    "unity.launch.v1",
    "unity.manage.v1",
    "unity.read.v1"
] as const;

const CAPABILITIES_WITH_OPTIONAL_ACTIONS_MISSING = [
    "events.live.v1",
    "backups.read.v1",
    RPC_CAPABILITIES.extensionsLifecycle,
    "projects.read.v1",
    "repositories.read.v1",
    "unity.read.v1"
] as const;

const query = new URLSearchParams(window.location.search);
const initialRoute = query.get("route") ?? "/";
const mode = (query.get("state") ?? "ready") as HarnessMode;
const materialEvidence = query.get("material") === "1";
const operationLogEvidence = query.get("operation-log-preview") === "1";
if (!materialEvidence && !operationLogEvidence) window.history.replaceState(null, "", initialRoute);

class BrowserUnityWindows implements GuiUnityWindows {
    readonly focused: UnityWindowTarget[] = [];
    starts = 0;
    reads = 0;
    private token: string | undefined;
    private version = 0;
    private projects: UnityWindowTarget[] = [];
    private states = new Map<string, UnityWindowState>();
    private wake: (() => void) | undefined;

    async start(token: string, targets: UnityWindowTarget[]) {
        this.starts += 1;
        this.token = token;
        this.projects = targets;
        return this.snapshot();
    }

    async next(token: string, after: number) {
        this.reads += 1;
        if (this.token === token && this.version === after) {
            await new Promise<void>((resolve) => { this.wake = resolve; });
        }
        if (this.token !== token) throw { code: "unity_window_observation_unavailable" };
        return this.snapshot();
    }

    async stop(token: string) {
        if (token === this.token) {
            this.token = undefined;
            this.notify();
        }
    }

    async focus(projectId: string, revision: number) {
        this.focused.push({ projectId, revision });
        if (query.has("unityWindowsReject")) throw { code: "unity_window_focus_rejected" };
    }

    set(projectId: string, state: UnityWindowState) {
        this.states.set(projectId, state);
        this.version += 1;
        this.notify();
    }

    private notify() {
        this.wake?.();
        this.wake = undefined;
    }

    private snapshot(): UnityWindowSnapshot {
        return { observing: true, version: this.version, states: this.projects.map(({ projectId }) => [projectId, this.states.get(projectId) ?? "closed"]) };
    }
}

class DeterministicGuiClient implements GuiRpcClient {
    readonly unityWindows = query.has("unityWindows") ? (window.unityWindowEvidence = new BrowserUnityWindows()) : undefined;
    async logsRead(params: LogsReadParams): Promise<LogsReadResult> {
        const fixture = window.fixtureLogs;
        fixture.reads.push(structuredClone(params));
        if (fixture.fail) throw { code: "daemon_unavailable", message: "" };
        const forward = params.afterSequence !== undefined;
        const matches = fixture.entries.filter((entry) => forward ? entry.sequence > params.afterSequence!
            : entry.sequence < (params.beforeSequence ?? Number.POSITIVE_INFINITY));
        matches.sort((a, b) => forward ? a.sequence - b.sequence : b.sequence - a.sequence);
        const entries = matches.slice(0, params.limit ?? 100);
        const hasMore = entries.length < matches.length;
        const nextSequence = hasMore ? entries.at(-1)?.sequence : undefined;
        if (!forward) entries.reverse();
        return { entries, highWatermark: Math.max(0, ...fixture.entries.map((entry) => entry.sequence)), hasMore, nextSequence };
    }

    subscribeEvents = browserCoreEvents.subscribe;
    private genericTasks = new Map<string, Operation>();
    private operationSequence = 0;
    private listExportAttempts = 0;
    private listPreviewReads = 0;
    selectFile(kind: NativeFileKind): ReturnType<GuiRpcClient["selectFile"]> {
        return this.value({ repositoryManifest: "C:/fixture/repository.json", unityEditor: "C:/Editors/Unity.exe", extensionPackage: "C:/fixture/test.alcomdext" }[kind]);
    }
    async subscribeFileDrag(receive: (event: FileDrag) => void) {
        const listener = (event: Event) => receive((event as CustomEvent<FileDrag>).detail);
        window.addEventListener("fixture-file-drag", listener);
        return () => window.removeEventListener("fixture-file-drag", listener);
    }
    selectRepositoryListFile(mode: "open" | "save", fileName?: string): ReturnType<GuiRpcClient["selectRepositoryListFile"]> {
        window.resourceRequests.push({ method: "selectRepositoryListFile", params: { mode, fileName } });
        return this.value(query.get("list-picker") === "cancel" ? undefined : mode === "open" ? "C:/fixture/repositories.txt" : `C:/fixture/${fileName}`);
    }
    private listImportStopped = false;
    private listFixtureCount(): number {
        const mode = query.get("list-import");
        return mode === "invalid" ? 0 : ["large", "hold", "transport"].includes(mode ?? "") ? 67 : 3;
    }
    repositoryListInspectFile(path: string): ReturnType<GuiRpcClient["repositoryListInspectFile"]> {
        window.resourceRequests.push({ method: "repositories.inspectListFile", params: { path } });
        const count = this.listFixtureCount();
        return this.value({ ignoredLines: 2, fingerprint: HASH, entries: [
            ...Array.from({ length: count }, (_, index) => ({ line: index + 1, status: "candidate" as const, source: { kind: "remote" as const, url: "https://list.example/" + (index + 1) + ".json?z=2&a=1" } })),
            { line: count + 1, status: "duplicate", source: { kind: "remote", url: "https://list.example/1.json?z=2&a=1" }, reason: "duplicate_source" },
            { line: count + 2, status: "invalid", reason: "source_invalid" },
            { line: count + 3, status: "unsupported", reason: "authentication_not_supported" }
        ] });
    }
    async repositoryListImportFile(path: string, expectedFingerprint: string, idempotencyKey: string): ReturnType<GuiRpcClient["repositoryListImportFile"]> {
        window.resourceRequests.push({ method: "repositories.importListFile", params: { path, expectedFingerprint, idempotencyKey } });
        const mode = query.get("list-import");
        if (mode === "hold") await new Promise<void>((resolve) => window.addEventListener("test-list-register-ready", () => resolve(), { once: true }));
        if (mode === "transport") throw { code: "daemon_unavailable" };
        if (mode === "changed") throw { code: "repository_list_changed" };
        return this.value({ items: Array.from({ length: this.listFixtureCount() }, (_, index): RepositoryListImportItem => {
            const line = index + 1;
            if (line === 2 && mode === "partial") return { line, status: "failed", code: "repository_unavailable" };
            if (line === 2 && mode === "unknown") return { line, status: "unknown", code: "internal_error" };
            if ((line === 3 && mode === "unknown") || (this.listImportStopped && line > 64)) return { line, status: "unprocessed" };
            if (line === 1 && mode === "duplicate") return { line, status: "existing" };
            return { line, status: "added" };
        }) });
    }
    repositoryListCancelRegistration(idempotencyKey: string): ReturnType<GuiRpcClient["repositoryListCancelRegistration"]> {
        window.resourceRequests.push({ method: "repositories.cancelListRegistration", params: { idempotencyKey } });
        this.listImportStopped = true;
        return this.value({ stopped: true });
    }
    repositoryListPreviewExport(): ReturnType<GuiRpcClient["repositoryListPreviewExport"]> {
        window.resourceRequests.push({ method: "repositories.previewListExport", params: {} });
        this.listPreviewReads += 1;
        return this.value({ exportableCount: 230, excluded: [{ repositoryId: LOCAL_REPOSITORY_ID, reason: "local_source" }], byteCount: 12000, fingerprint: String(this.listPreviewReads).repeat(64) });
    }
    repositoryListExportFile(targetPath: string, expectedFingerprint: string): ReturnType<GuiRpcClient["repositoryListExportFile"]> {
        window.resourceRequests.push({ method: "repositories.exportListFile", params: { targetPath, expectedFingerprint } });
        this.listExportAttempts += 1;
        if (this.listExportAttempts === 1 && query.get("list-export-error")) return Promise.reject({ code: query.get("list-export-error") });
        return this.value({ exportedCount: 230, byteCount: 12000, sha256: HASH });
    }
    private candidateSourceWasDelayed = false;
    private hubDefaultReady = query.has("hub-default");
    private hubReadFailurePending = false;
    private repositoriesReady: Promise<void> | undefined;
    private settingsReady: Promise<void> | undefined;
    private themeSaveRejected = false;
    private argumentsSaveRejected = false;
    private argumentsReadFailurePending = false;
    private unityReady: Promise<void> | undefined;
    private unityDiscoveryComplete = false;
    private settings: SettingsGetResult = {
        configSchema: 12,
        directories: { projectDirectory: query.get("project-directory") ?? (query.has("windows-paths") ? "\\\\?\\C:\\Projects" : "C:\\Fixture"), backupDirectory: "C:\\Fixture\\Backups" },
        unityHub: { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" },
        revision: 7,
        settings: {
            projects: { autoDiscover: true },
            backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false },
            paths: { projectDirectory: query.get("project-directory"), backupDirectory: null },
            unity: { hubPath: null, defaultLaunchArguments: query.has("default-arguments") ? JSON.parse(query.get("default-arguments")!) as string[] : null },
            window: { rememberState: false, fullscreen: query.has("window-fullscreen") },
            integrations: { handleVccLinks: true },
            network: { proxy: { mode: "system" } },
            appearance: { mode: "system", sourceColor: null, scheme: "vibrant", motion: "system" },
            locale: query.get("locale") === "zh-CN" ? "zh-CN" : query.get("locale") === "ja-JP" ? "ja-JP" : "en-US",
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
    private launchArguments = query.has("project-arguments") ? JSON.parse(query.get("project-arguments")!) as string[] : ["-logFile"];
    private launchConfigRevision = query.has("launch-inherit") ? 0 : 2;
    private snapshotRevision = 1;
    private projectFavorites = new Map<string, boolean>();
    private repositoryRefreshCompleted = false;
    private repositoryRefreshAttempts = new Map<string, number>();
    private refreshedRepositoryRevisions = new Map<string, number>();
    private userPackageRemoved = false;
    private userPackageRevision = 1;
    private userPackageEnrolled = false;
    private userPackagePath = query.has("windows-paths") ? "\\\\?\\UNC\\nas\\Packages\\Avatar" : "<private-user-package>";
    private userDetailsUnavailable = query.has("user-details-error");
    private lifecycleRepositories = new Map<string, RepositorySnapshot>();
    private templatePageReads = 0;
    private templatesUnavailable = query.has("template-list-error");
    private projectTemplates = [...builtinTemplates(), ...(query.has("template-existing")
        ? [userTemplate(USER_TEMPLATE_ID, query.has("template-same-name") ? "VRChat Avatars 2022" : "Existing fixture")] : [])];
    private templateRevision = 8;
    private templateRequestsByKey = new Map<string, string>();
    private creationRequests = new Map<string, { fingerprint: string; projectId: string; operationId: string }>();

    private connectionRestored = false;

    constructor(private readonly mode: HarnessMode) {
        this.userPackageEnrolled = mode === "package-user-source";
        if (mode === "empty") this.projectTemplates = [];
        if (initialRoute.startsWith("/operations") && mode !== "empty" && !query.has("h5-utilities") && !query.has("h4-management")) this.beginOperation();
        window.addEventListener("test-user-details-readable", () => { this.userDetailsUnavailable = false; });
        window.addEventListener("test-user-package-advance", () => { this.userPackageRevision += 1; });
        window.addEventListener("test-resource-repository-advance", () => {
            for (const [id, record] of this.lifecycleRepositories) {
                this.lifecycleRepositories.set(id, { ...record, name: `${record.name} (updated)`, revision: (record.revision ?? 0) + 1 });
            }
        });
        window.addEventListener("test-template-advance", () => {
            this.projectTemplates = this.projectTemplates.map((template) => template.id === USER_TEMPLATE_ID
                ? { ...template, revision: ++this.templateRevision } : template);
            browserCoreEvents.emit("project-template", TEMPLATE_EVENT_ID, "project-template.changed", this.templateRevision);
        });
        window.addEventListener("test-template-list-readable", () => { this.templatesUnavailable = false; });
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
                : SUPPORTED_CAPABILITIES.filter((capability) => !(query.get("noPlanV2") && capability === "packages.plan.v2") && !(query.has("noUnityManage") && capability === "unity.manage.v1") && !(query.has("noHubOpen") && capability === "unity.hub.open.v1") && !(query.has("noProjectTemplates") && capability === "projectTemplates.v2"))
        });
    }

    private beginOperation(kind = "packages.apply", knownId?: string, createdProject?: ReturnType<typeof project>) {
        const operationId = knownId ?? (this.operationSequence++ === 0 ? OPERATION_ID : crypto.randomUUID());
        const operation: Operation = { ...runningOperation(), operationId, kind, revision: 1 };
        this.genericTasks.set(operationId, operation);
        const project = createdProject ?? this.pendingProject;
        const deleting = this.pendingDeleteProjectId;
        const migration = this.pendingMigrationTarget;
        browserCoreEvents.emit("operation", operationId, "operation.created", 1);
        if (!query.has("hold-core-operation")) setTimeout(() => {
            const state = this.mode === "failed" ? "failed" : this.mode === "cancelled" ? "cancelled" : "succeeded";
            if (state === "succeeded" && project && !this.createdProjects.some((candidate) => candidate.projectId === project.projectId)) this.createdProjects.push(project);
            if (state === "succeeded" && deleting) this.deletedProjectIds.add(deleting);
            if (state === "succeeded" && migration) { this.currentUnityVersion = migration; this.currentProjectRevision++; }
            const next: Operation = { ...operation, state, revision: 2, updatedAtMs: operation.updatedAtMs + 1000,
                completedAtMs: operation.updatedAtMs + 1000, progress: { phase: "state_committed" },
                ...(state === "succeeded" && project ? { result: { projectId: project.projectId, revision: 1 } } : {}),
                ...(state === "failed" ? { errorCode: "package_archive_invalid" } : {}) };
            this.genericTasks.set(operationId, next);
            if (state === "succeeded" && project) browserCoreEvents.emit("project", project.projectId, "project.registered", project.revision);
            if (deleting) browserCoreEvents.emit("project", deleting, "project.directory_deleted", 3);
            if (migration) browserCoreEvents.emit("project", PROJECT_ID, "project.unity_changed", this.currentProjectRevision);
            browserCoreEvents.emit("operation", operationId, `operation.${state}`, next.revision);
        }, 250);
        return operationId;
    }
    stateCheck(): ReturnType<GuiRpcClient["stateCheck"]> {
        return this.value({ operationId: this.beginOperation("state.check"), replayed: false });
    }

    operationsList(cursor?: Parameters<GuiRpcClient["operationsList"]>[0], activeOnly = false): ReturnType<GuiRpcClient["operationsList"]> {
        let operations = query.has("h3") ? [window.h3Operation] : [...this.genericTasks.values()];
        const activeCount = operations.filter((item) => !["succeeded", "failed", "cancelled"].includes(item.state)).length;
        if (activeOnly) operations = operations.filter((item) => !["succeeded", "failed", "cancelled"].includes(item.state));
        if (query.has("h3")) {
            window.h3Requests.push({ method: "operations.list", params: { cursor, activeOnly } });
            return this.value({ activeCount, operations: [{ ...window.h3Operation, operationId: cursor === undefined ? OPERATION_ID : "00000000-0000-4000-8000-000000000612" }].filter((item) => !activeOnly || !["succeeded", "failed", "cancelled"].includes(item.state)), ...(cursor === undefined && query.has("h3-pages") && !activeOnly ? { nextCursor: { createdAtMs: 1_700_000_000_000, operationId: OPERATION_ID } } : {}) });
        }
        return this.value({ operations: this.mode === "empty" ? [] : operations, activeCount });
    }

    operationGet(operationId: string): ReturnType<GuiRpcClient["operationGet"]> {
        if (query.has("h3")) {
            window.h3Requests.push({ method: "operations.get", params: { operationId } });
            if (window.h3ReadError !== undefined) return Promise.reject({ code: window.h3ReadError });
            const reads = window.h3Requests.filter((request) => request.method === "operations.get").length;
            if (String(reads) === query.get("h3-read-error-at") || reads <= Number(query.get("h3-read-error-until") ?? "0")) return Promise.reject({ code: "daemon_unavailable" });
            return this.value({ ...window.h3Operation, operationId, ...(window.h3Operation.state === "succeeded" ? { result: this.pendingProject === undefined ? { backupId: BACKUP_ID } : { projectId: this.pendingProject.projectId } } : {}) });
        }
        return this.value(this.genericTasks.get(operationId) ?? runningOperation());
    }

    async operationCancel(operationId: string, expectedRevision: number): ReturnType<GuiRpcClient["operationCancel"]> {
        if (query.has("h3")) {
            window.h3Requests.push({ method: "operations.cancel", params: { operationId, expectedRevision } });
            if (query.has("h3-cancel-conflict") && window.h3Requests.filter((request) => request.method === "operations.cancel").length === 1) return Promise.reject({ code: "resource_revision_conflict" });
            window.h3Operation = { ...window.h3Operation, state: "cancelling", revision: expectedRevision + 1 };
            return this.value({ operation: window.h3Operation, replayed: false });
        }
        const current = this.genericTasks.get(operationId) ?? { ...runningOperation(), operationId };
        if (current.revision !== expectedRevision) throw { code: "revision_conflict" };
        const result = await this.value({ operation: { ...current, state: "cancelling" as const, revision: expectedRevision + 1 }, replayed: false });
        this.genericTasks.set(operationId, result.operation);
        browserCoreEvents.emit("operation", operationId, "operation.state_changed", result.operation.revision);
        return result;
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
        if (this.argumentsReadFailurePending) { this.argumentsReadFailurePending = false; throw { code: "internal_error" }; }
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
            window.heldSettingsReads += 1;
            await this.settingsReady;
            window.releasedSettingsReads += 1;
        }
        return this.value(this.settings);
    }

    async settingsUpdate(expectedRevision: number, update: Parameters<GuiRpcClient["settingsUpdate"]>[1]): ReturnType<GuiRpcClient["settingsUpdate"]> {
        window.settingsRequests.push({ expectedRevision, update: structuredClone(update) });
        const argumentsChange = update.unity?.defaultLaunchArguments !== undefined;
        const argumentsFailure = query.get("arguments-save");
        if (argumentsChange && argumentsFailure === "denied") throw { code: "permission_denied" };
        if (argumentsChange && !this.argumentsSaveRejected && argumentsFailure === "error-once") { this.argumentsSaveRejected = true; throw { code: "internal_error" }; }
        if (argumentsChange && !this.argumentsSaveRejected && argumentsFailure === "conflict-once") {
            this.argumentsSaveRejected = true;
            this.settings.revision += 1;
            this.settings.settings.unity.defaultLaunchArguments = ["-batchmode"];
            throw { code: "revision_conflict" };
        }
        if (query.has("hub-reject-invalid") && update.unity?.hubPath?.includes("Invalid")) throw { code: "invalid_request" };
        if (query.get("theme-save") === "error") throw { code: "internal_error" };
        if (query.get("theme-save") === "error-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            throw { code: "internal_error" };
        }
        if (query.get("theme-save") === "conflict-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            this.settings.revision += 1;
            this.settings.settings.appearance.motion = "reduced";
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
            configSchema: 12,
            directories: {
                projectDirectory: update.paths?.projectDirectory === undefined ? this.settings.directories.projectDirectory : update.paths.projectDirectory ?? "C:\\Fixture",
                backupDirectory: update.paths?.backupDirectory === undefined ? this.settings.directories.backupDirectory : update.paths.backupDirectory ?? "C:\\Fixture\\Backups"
            },
            unityHub: this.hubLocation(update.unity?.hubPath === undefined ? this.settings.settings.unity.hubPath : update.unity.hubPath),
            revision: expectedRevision + 1,
            settings: {
                projects: { ...this.settings.settings.projects, ...update.projects },
                backups: { ...this.settings.settings.backups, ...update.backups },
                paths: { ...this.settings.settings.paths, ...update.paths },
                unity: { ...this.settings.settings.unity, ...update.unity },
                window: { ...this.settings.settings.window, ...update.window },
                integrations: update.integrations ?? this.settings.settings.integrations,
                network: update.network ?? this.settings.settings.network,
                appearance: { ...this.settings.settings.appearance, ...update.appearance },
                locale: update.locale ?? this.settings.settings.locale,
                packages: { ...this.settings.settings.packages, ...update.packages }
            }
        };
        window.settingsWrites.push({ expectedRevision, revision: this.settings.revision });
        if (argumentsChange && !this.argumentsSaveRejected && ["commit-error-once", "commit-read-error-once"].includes(argumentsFailure ?? "")) {
            this.argumentsSaveRejected = true;
            this.argumentsReadFailurePending = argumentsFailure === "commit-read-error-once";
            throw { code: "internal_error" };
        }
        if (query.get("theme-save") === "commit-error-once" && !this.themeSaveRejected) {
            this.themeSaveRejected = true;
            throw { code: "internal_error" };
        }
        return this.value(this.settings);
    }

    projectsInspect(): ReturnType<GuiRpcClient["projectsInspect"]> { return this.value({ project: project() }); }
    projectsList(cursor?: RegistryCursor, view?: ProjectListView, expectedViewSequence?: number): ReturnType<GuiRpcClient["projectsList"]> {
        window.projectListReads += 1;
        const page = (projects: ProjectSnapshot[]) => projectListPage(projects, cursor, view, expectedViewSequence, this.mode === "favorite-pages");
        if (query.has("auto-projects")) {
            const projects = [project(), ...window.autoDiscoveryProjects];
            window.projectSnapshots.push(structuredClone(projects));
            if (query.has("hold-project-poll") && window.projectListReads === 2) {
                const snapshot = structuredClone(page(projects));
                return new Promise((resolve) => { window.resumeProjectPoll = () => resolve(snapshot); });
            }
            return this.value(page(projects));
        }
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
            return this.value(page(projects));
        }
        if (query.has("projectGrid")) {
            const names = ["Avatar Studio", "World Workshop", "非常长的项目名称用于检查卡片信息与操作是否保持整齐 Long Project Name", "Package Sandbox", "Unknown Editor", "Avatar Preview", "Archive"];
            return this.value(page(names.map((name, index) => ({
                ...project(`00000000-0000-4000-8000-${String(101 + index).padStart(12, "0")}`, name, index === 0),
                rootPath: `C:\\Projects\\Workspace with a long directory name\\${name}`,
                projectType: index === 1 ? "worlds" : index === 3 ? "Unknown project type with long text" : "avatars",
                unityVersion: index === 4 ? "" : index === 1 ? "6000.0.40f1" : "2022.3.22f1",
                registeredAtMs: index === 4 ? undefined : 1_690_000_000_000
            }))));
        }
        if (this.mode === "favorite-pages") {
            return this.value(page([project(PROJECT_ID, "Alpha"), project("00000000-0000-4000-8000-000000000110", "Zulu Favorite", true)]));
        }
        const projects = [project(), ...this.createdProjects].map((item) => this.projectFavorites.has(item.projectId) ? { ...item, favorite: this.projectFavorites.get(item.projectId)!, revision: 3 } : item).filter((candidate) => candidate.projectId === undefined || !this.deletedProjectIds.has(candidate.projectId));
        const result = page(this.mode === "empty" ? [] : projects);
        if (query.has("hold-project-refresh") && this.createdProjects.length > 0) return new Promise((resolve) => window.addEventListener("test-project-refresh-ready", () => resolve(result), { once: true }));
        return this.value(result);
    }
    projectGet(projectId: string): ReturnType<GuiRpcClient["projectGet"]> {
        const value = this.createdProjects.find((candidate) => candidate.projectId === projectId) ?? { ...project(projectId), unityVersion: this.currentUnityVersion, revision: this.currentProjectRevision };
        if (this.projectLaunchTimes.has(projectId)) value.lastModifiedAtMs = this.projectLaunchTimes.get(projectId)!;
        if (query.get("twoInstalled")) {
            value.lockedDependencies = [...value.lockedDependencies, { packageId: "com.example.remote", value: "1.2.3" }];
            value.directDependencies = [...value.directDependencies, { packageId: "com.example.remote", value: "^1.2.0" }];
        }
        if (["missing", "unavailable"].includes(query.get("remoteEvidence") ?? "")) value.directDependencies = [...value.directDependencies, { packageId: "com.example.remote", value: "^1.0.0" }];
        if (query.get("bulk257")) {
            const dependencies = Array.from({ length: 257 }, (_, index) => ({ packageId: `com.example.batch${String(index).padStart(3, "0")}`, value: "1.2.3" }));
            value.lockedDependencies = dependencies;
            value.directDependencies = dependencies;
        }
        return this.value({ project: value });
    }
    openProjectDirectory(projectId: string): ReturnType<GuiRpcClient["openProjectDirectory"]> { window.resourceRequests.push({ method: "openProjectDirectory", params: { projectId } }); return this.value(undefined); }
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
    selectDirectory(): ReturnType<GuiRpcClient["selectDirectory"]> {
        if (query.has("resource-lifecycle")) window.resourceRequests.push({ method: "selectDirectory", params: {} });
        return this.value(query.has("cancel-directory") ? undefined : "C:\\Fixture\\Avatar");
    }
    projectRegister(): ReturnType<GuiRpcClient["projectRegister"]> { return this.value({ project: project(), replayed: false }); }
    projectRefresh(): ReturnType<GuiRpcClient["projectRefresh"]> { return this.value({ project: project(), replayed: false }); }
    projectSetFavorite(_projectId: string, favorite: boolean): ReturnType<GuiRpcClient["projectSetFavorite"]> {
        if (this.mode === "favorite-error") return Promise.reject({ code: "internal_error" });
        if (this.mode === "favorite-conflict" && this.favoriteConflictPending) {
            this.favoriteConflictPending = false;
            return Promise.reject({ code: "revision_conflict" });
        }
        this.projectFavorites.set(_projectId, favorite);
        return this.value({ project: { ...project(), favorite, revision: 3 }, replayed: false });
    }
    projectUnregister(): ReturnType<GuiRpcClient["projectUnregister"]> { return this.value({ projectId: PROJECT_ID, revision: 3, unregistered: true, replayed: false }); }
    projectPlanCopy(_sourceProjectId: string, expectedRevision: number, targetParentPath: string, targetLeaf: string): ReturnType<GuiRpcClient["projectPlanCopy"]> {
        if (query.has("windows-paths")) window.h3Requests.push({ method: "projects.planCopy", params: { sourceProjectId: _sourceProjectId, expectedRevision, targetParentPath, targetLeaf } });
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
    projectApplyCopy(): ReturnType<GuiRpcClient["projectApplyCopy"]> { return this.value({ operationId: this.beginOperation("projects.copy", "00000000-0000-4000-8000-000000000062"), targetProjectId: "00000000-0000-4000-8000-000000000061", replayed: false }); }
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
        return this.value({ operationId: this.beginOperation("projects.deleteDirectory", "00000000-0000-4000-8000-000000000064"), projectId: PROJECT_ID, replayed: false });
    }

    async repositoriesInspect(source: Parameters<GuiRpcClient["repositoriesInspect"]>[0], credentials?: Parameters<GuiRpcClient["repositoriesInspect"]>[1]): ReturnType<GuiRpcClient["repositoriesInspect"]> {
        window.resourceRequests.push({ method: "repositories.inspect", params: { source, ...(credentials && { credentialsConfigured: true }) } });
        if (!query.has("resource-lifecycle")) return this.value({ repository: repository() });
        if (query.has("resource-invalid-source")) return Promise.reject({ code: "repository_document_invalid" });
        const { repositoryId: _id, revision: _revision, ...preview } = this.resourceRepository(source);
        return this.value({ repository: preview });
    }
    async repositoriesList(cursor?: { registeredAtMs: number; id: string }): ReturnType<GuiRpcClient["repositoriesList"]> {
        if (query.has("list-import")) window.resourceRequests.push({ method: "repositories.list", params: cursor ?? {} });
        if (query.has("resource-lifecycle")) {
            window.resourceRequests.push({ method: "repositories.list", params: cursor ?? {} });
            return this.value({ repositories: [...this.lifecycleRepositories.values()] });
        }
        if (query.has("holdRepositories")) {
            this.repositoriesReady ??= new Promise<void>((resolve, reject) => window.addEventListener("test-repositories-ready", (event) => {
                if (event instanceof CustomEvent && event.detail === "error") reject({ code: "daemon_unavailable" });
                else resolve();
            }, { once: true }));
            await this.repositoriesReady;
        }
        if (this.mode === "empty" || this.mode === "package-no-repositories") return this.value({ repositories: [] });
        if (query.has("package-refresh-parallel")) return this.value({ repositories: Array.from({ length: 66 }, (_, index) => ({
            ...repository(),
            repositoryId: `04000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
            name: `Repository ${index + 1}`
        })) });
        if (query.has("builtin-repositories")) return this.value({ repositories: [
            { ...repository(), repositoryId: "7e2233c8-0b3f-4cf2-aeb4-57d3d240b101", builtin: "official" as const, actions: { canRemove: false, canEditAuthentication: false }, source: { kind: "remote" as const, url: "https://vrchat.github.io/packages/index.json" }, name: "Official", refreshedAtMs: 0 },
            { ...repository(), repositoryId: "7e2233c8-0b3f-4cf2-aeb4-57d3d240b102", builtin: "curated" as const, actions: { canRemove: false, canEditAuthentication: false }, source: { kind: "remote" as const, url: "https://vrchat-community.github.io/vpm-listing-curated/index.json" }, name: "Curated" },
            repository(),
        ].map((record) => this.lifecycleRepositories.get(record.repositoryId) ?? record) });
        if (["package-multiple", "package-partial-failure"].includes(this.mode) || query.get("twoInstalled")) {
            return cursor === undefined
                ? this.value({ repositories: [this.refreshedRepository(repository())], nextCursor: { registeredAtMs: 1_690_000_000_000, id: REPOSITORY_ID } })
                : this.value({ repositories: [this.refreshedRepository(localRepository())] });
        }
        return this.value({ repositories: [this.refreshedRepository(repository())] });
    }
    private refreshedRepository(repository: RepositorySnapshot): RepositorySnapshot {
        const revision = repository.repositoryId === undefined ? undefined : this.refreshedRepositoryRevisions.get(repository.repositoryId);
        return revision === undefined ? repository : { ...repository, revision };
    }
    repositoryGet(repositoryId: string): ReturnType<GuiRpcClient["repositoryGet"]> {
        if (query.has("resource-lifecycle")) {
            window.resourceRequests.push({ method: "repositories.get", params: { repositoryId } });
            const record = this.lifecycleRepositories.get(repositoryId);
            return record === undefined ? Promise.reject({ code: "repository_not_registered" }) : this.value({ repository: record });
        }
        const value = repositoryId === LOCAL_REPOSITORY_ID ? localRepository() : repository();
        return this.value({ repository: this.refreshedRepository(this.mode === "package-revision-conflict" ? { ...value, revision: 3 } : value) });
    }
    async repositoryPackages(repositoryId: string, cursor?: Parameters<GuiRpcClient["repositoryPackages"]>[1]): ReturnType<GuiRpcClient["repositoryPackages"]> {
        window.repositoryPackageRequests.push({ repositoryId, ...(cursor === undefined ? {} : { cursor }) });
        if (query.has("resource-lifecycle")) return this.value({ packages: [{ packageId: cursor === undefined ? "com.example.first" : "com.example.second", version: "1.0.0", displayName: cursor === undefined ? (this.lifecycleRepositories.get(repositoryId)?.revision ?? 0) > 1 ? "Updated catalog package" : "First catalog package" : "Second catalog package", yanked: false, prerelease: false }], ...(cursor === undefined ? { nextCursor: { packageId: "com.example.first", version: "1.0.0" } } : {}) });
        if (query.get("metadata") === "error") return Promise.reject({ code: "repository_unavailable" });
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
                ...(query.has("raw-only-repository-package") ? [{ packageId: "com.example.raw-only", version: "legacy", displayName: "Raw-only package", yanked: false }] : []),
                ...(this.repositoryRefreshCompleted ? [{ packageId: "com.example.refreshed", version: "1.0.0", displayName: "Refreshed package", yanked: false }] : [])
            ];
        if (query.get("metadata") === "paged" && repositoryId === REPOSITORY_ID) {
            return this.value({
                packages: packages.filter((item) => cursor === undefined ? item.packageId !== "com.example.remote" : item.packageId === "com.example.remote").map((item) => ({ ...item, prerelease: false })),
                ...(cursor === undefined ? { nextCursor: { packageId: "com.example.avatar", version: "1.3.0" } } : {})
            });
        }
        return this.value({ packages: packages.map((item) => ({ ...item, prerelease: false, ...(query.has("package-link-error") && item.packageId === "com.example.avatar" && item.version === "1.2.3" ? { links: { documentation: { url: "https://fixture.example/docs" } } } : {}) })) });
    }

    openPackageLink(): ReturnType<GuiRpcClient["openPackageLink"]> {
        if (query.has("package-link-error")) return Promise.reject({ code: "permission_denied" });
        return this.value(undefined);
    }
    async repositoryRegister(source: Parameters<GuiRpcClient["repositoryRegister"]>[0], credentials?: Parameters<GuiRpcClient["repositoryRegister"]>[1]): ReturnType<GuiRpcClient["repositoryRegister"]> {
        window.resourceRequests.push({ method: "repositories.register", params: { source, ...(credentials && { credentialsConfigured: true }) } });
        if (query.has("resource-register-invalid")) throw { code: "repository_document_invalid" };
        if (!query.has("resource-lifecycle")) return this.value({ repository: repository(), replayed: false });
        const record = { ...this.resourceRepository(source), ...(credentials && { credentialsConfigured: true }) };
        if (this.lifecycleRepositories.has(record.repositoryId!)) return Promise.reject({ code: "repository_already_registered" });
        this.lifecycleRepositories.set(record.repositoryId!, record);
        return this.value({ repository: record, replayed: false });
    }
    async repositoriesRefreshAll(): ReturnType<GuiRpcClient["repositoriesRefreshAll"]> {
        window.resourceRequests.push({ method: "repositories.refreshAll", params: {} });
        if (query.has("package-refresh-parallel")) await new Promise<void>((resolve) => window.addEventListener("test-package-refresh-ready", () => resolve(), { once: true }));
        if (this.mode === "package-no-repositories") return this.value({ items: [] });
        this.repositoryRefreshCompleted = true;
        this.refreshedRepositoryRevisions.set(REPOSITORY_ID, this.mode === "package-revision-conflict" ? 4 : 3);
        const items: Awaited<ReturnType<GuiRpcClient["repositoriesRefreshAll"]>>["items"] = [{ status: "refreshed", repositoryId: REPOSITORY_ID }];
        if (this.mode === "package-partial-failure") items.push({ status: "failed", repositoryId: LOCAL_REPOSITORY_ID, code: "repository_unavailable" });
        else if (this.mode === "package-multiple") {
            items.push({ status: "refreshed", repositoryId: LOCAL_REPOSITORY_ID });
            this.refreshedRepositoryRevisions.set(LOCAL_REPOSITORY_ID, 3);
        }
        return this.value({ items });
    }
    repositorySetCredentials(repositoryId: string, expectedRevision: number, credentials: Parameters<GuiRpcClient["repositorySetCredentials"]>[2]): ReturnType<GuiRpcClient["repositorySetCredentials"]> {
        window.resourceRequests.push({ method: "repositories.setCredentials", params: { repositoryId, expectedRevision, credentialsConfigured: credentials !== null } });
        const current = this.lifecycleRepositories.get(repositoryId) ?? repository();
        const next = { ...current, revision: expectedRevision + 1, credentialsConfigured: credentials !== null };
        this.lifecycleRepositories.set(repositoryId, next);
        return this.value({ repository: next, replayed: false });
    }
    async repositorySetDisplayName(repositoryId: string, expectedRevision: number, displayName: string | null): ReturnType<GuiRpcClient["repositorySetDisplayName"]> {
        window.resourceRequests.push({ method: "repositories.setDisplayName", params: { repositoryId, expectedRevision, displayName } });
        const current = this.lifecycleRepositories.get(repositoryId) ?? (await this.repositoriesList()).repositories.find((record) => record.repositoryId === repositoryId);
        if (current === undefined) throw { code: "repository_not_registered" };
        if (current.revision !== expectedRevision) throw { code: "revision_conflict" };
        const next = { ...current, revision: expectedRevision + 1, displayName: displayName?.trim() || undefined };
        this.lifecycleRepositories.set(repositoryId, next);
        return this.value({ repository: next, replayed: false });
    }
    async repositoryRefresh(repositoryId: string, expectedRevision: number): ReturnType<GuiRpcClient["repositoryRefresh"]> {
        window.resourceRequests.push({ method: "repositories.refresh", params: { repositoryId, expectedRevision } });
        if (query.has("hold-resource-refresh")) await new Promise<void>((resolve) => window.addEventListener("test-resource-refresh-ready", () => resolve(), { once: true }));
        const attempts = (this.repositoryRefreshAttempts.get(repositoryId) ?? 0) + 1;
        this.repositoryRefreshAttempts.set(repositoryId, attempts);
        if (query.has("resource-lifecycle")) {
            const record = this.lifecycleRepositories.get(repositoryId);
            if (record === undefined) return Promise.reject({ code: "repository_not_registered" });
            if (record.revision !== expectedRevision) return Promise.reject({ code: "revision_conflict" });
            if (query.has("resource-refresh-error") && attempts === 1) return Promise.reject({ code: "repository_unavailable" });
            const next = { ...record, revision: expectedRevision + 1 };
            this.lifecycleRepositories.set(repositoryId, next);
            return this.value({ repository: next, replayed: false });
        }
        if (this.mode === "package-partial-failure" && repositoryId === LOCAL_REPOSITORY_ID) return Promise.reject({ code: "repository_unavailable" });
        if (this.mode === "package-revision-conflict" && repositoryId === REPOSITORY_ID && attempts === 1) return Promise.reject({ code: "revision_conflict" });
        if (this.mode === "package-revision-conflict" && repositoryId === REPOSITORY_ID && attempts > 2) return Promise.reject({ code: "unexpected_retry" });
        this.repositoryRefreshCompleted = true;
        this.refreshedRepositoryRevisions.set(repositoryId, expectedRevision + 1);
        return this.value({ repository: { ...(repositoryId === LOCAL_REPOSITORY_ID ? localRepository() : repository()), repositoryId, revision: expectedRevision + 1 }, replayed: false });
    }
    repositoryUnregister(repositoryId: string, expectedRevision: number): ReturnType<GuiRpcClient["repositoryUnregister"]> {
        window.resourceRequests.push({ method: "repositories.unregister", params: { repositoryId, expectedRevision } });
        if (query.has("resource-remove-error")) return Promise.reject({ code: "permission_denied" });
        if (query.has("resource-lifecycle")) {
            if (this.lifecycleRepositories.get(repositoryId)?.revision !== expectedRevision) return Promise.reject({ code: "revision_conflict" });
            this.lifecycleRepositories.delete(repositoryId);
        }
        return this.value({ repositoryId, revision: expectedRevision + 1, unregistered: true, replayed: false });
    }

    private resourceRepository(source: Parameters<GuiRpcClient["repositoryRegister"]>[0]): RepositorySnapshot {
        return { repositoryId: source.kind === "local" ? LOCAL_REPOSITORY_ID : REPOSITORY_ID, source, actions: { canRemove: true, canEditAuthentication: source.kind === "remote" }, declaredId: source.kind === "local" ? "local-example" : "example", name: source.kind === "local" ? "Local packages" : "Example packages", issues: [], revision: 1, refreshedAtMs: 1_700_000_000_000 };
    }

    async packageQueryProjectCandidates(params: Parameters<GuiRpcClient["packageQueryProjectCandidates"]>[0]): ReturnType<GuiRpcClient["packageQueryProjectCandidates"]> {
        window.candidateRequests.push(params);
        if (params.view.kind === "summary" && params.view.filter?.search
            && params.view.filter.search === query.get("deferCandidateFilter")) {
            await new Promise<void>((resolve) => { window.resumeCandidateFilter = resolve; });
        }
        if (query.has("deferCandidateSummary") && params.view.kind === "summary" && params.cursor?.offset === 1) {
            await new Promise<void>((resolve) => { window.resumeCandidateSummary = resolve; });
        }
        if (query.get("delayVersions") && params.view.kind === "versions" && !params.cursor) {
            await new Promise<void>((resolve) => { window.resumeCandidateVersions = resolve; });
        }
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
            const installed = params.view.packageId === "com.example.avatar" || (Boolean(query.get("twoInstalled")) && params.view.packageId === "com.example.remote");
            const relation = (value: PackageCandidateEvidence["relation"]): PackageCandidateEvidence["relation"] => installed ? value : "not_installed";
            const items = providers.flatMap((id) => [
                candidate("9.0.0", relation("newer"), id, ["all_yanked"]),
                candidate("2.0.0-beta.1", relation("newer"), id, this.settings.settings.packages.showPrerelease ? [] : ["prerelease_excluded"], "prerelease"),
                candidate("1.10.0", relation("newer"), id),
                candidate("1.2.3", relation("same_precedence"), id),
                candidate("1.1.0", relation("older"), id),
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
            if (packageId === "com.example.remote") {
                if (query.get("remoteEvidence") === "ambiguous") choice = { kind: "ambiguous", reason: "source_ambiguous" };
                if (query.get("remoteEvidence") === "unknown") choice = { kind: "unknown", reasons: ["classification_unknown"] };
                if (query.get("remoteEvidence") === "blocked") choice = { kind: "none", reasons: ["all_yanked"] };
                if (query.get("remoteEvidence") === "missing") choice = { kind: "none", reasons: ["no_visible_candidate"] };
                if (query.get("remoteEvidence") === "unavailable") choice = { kind: "unknown", reasons: ["catalog_incomplete"] };
            }
            if (query.get("noUpdate") && installed) target.relation = "same_precedence";
            const providers: PackageCandidateSummary["providers"] = [];
            if (visibleRemote && packageId !== "com.example.local" && !(packageId === "com.example.remote" && ["missing", "unavailable"].includes(query.get("remoteEvidence") ?? ""))) providers.push({ source: { kind: "repository", repository_id: REPOSITORY_ID }, sourceRevision: 2 });
            if (visibleLocal && (packageId === "com.example.local" || (packageId === "com.example.remote" && query.get("twoInstalled")) || (installed && (query.get("versions") || query.get("ambiguous"))))) providers.push({ source: { kind: "repository", repository_id: LOCAL_REPOSITORY_ID }, sourceRevision: 2 });
            if (packageId === "com.example.avatar" && this.mode === "package-user-source" && !this.settings.settings.packages.hideLocalUserPackages) providers.push({ source: { kind: "user_package", user_package_id: USER_PACKAGE_ID }, sourceRevision: 2 });
            let update: PackageCandidateSummary["update"] = choice.kind === "candidate" ? installed ? query.get("noUpdate") ? { kind: "no_update", reason: "same_precedence" } : { kind: "target", candidate: target } : { kind: "no_update", reason: "not_installed" } : { kind: "unavailable", reasons: choice.kind === "ambiguous" ? [choice.reason] : choice.reasons };
            if (query.get("knownExclusion") && packageId === "com.example.remote") { target.relation = "older"; update = { kind: "no_update", reason: "installed_newer" }; }
            const stableCandidate = { ...target };
            if (query.get("stableAlternate") && installed) { target.version = "2.0.0-beta.1"; target.classification = "prerelease"; }
            const stableChoice: CandidateChoice = query.get("stableAlternate") && installed ? { kind: "candidate", candidate: stableCandidate } : choice;
            const stableUpdate: PackageCandidateSummary["update"] = query.get("stableAlternate") && installed ? { kind: "target", candidate: stableCandidate } : update;
            const displayNames: Record<string, string> = {
                "com.example.avatar": "Avatar tools",
                "com.example.remote": "Remote tools",
                "com.example.local": "Local tools",
                "com.example.refreshed": "Refreshed package"
            };
            const hasFixtureName = !query.has("metadata") && (query.get("versions") !== "1" || packageId === "com.example.avatar");
            const displayName = providers.length > 0 && hasFixtureName ? displayNames[packageId] : undefined;
            const links = visibleRemote && query.has("package-link-error") && packageId === "com.example.avatar"
                ? { documentation: "https://fixture.example/docs" } : undefined;
            return { packageId, displayName, links, providers, direct: (installed && query.get("transitive") !== "1") || (packageId === "com.example.remote" && ["missing", "unavailable"].includes(query.get("remoteEvidence") ?? "")), installed: installed ? { kind: "locked", version: "1.2.3" } : { kind: "absent" }, latest: choice, latestStable: stableChoice, projectLatest: choice, projectLatestStable: stableChoice, update, stableUpdate };
        });
        if (query.has("reverse-candidate-order")) items.reverse();
        if (params.view.bulkPackageIds !== undefined) {
            const selected = new Set(params.view.bulkPackageIds);
            const selectedItems = items.filter((item) => selected.has(item.packageId));
            const completeSelection = base.catalogComplete && selected.size > 0 && selected.size <= 256 && selectedItems.length === selected.size;
            const intent = (item: PackageCandidateSummary, stable: boolean): CandidateBulkIntent[] => {
                const choice = stable ? item.projectLatestStable : item.projectLatest;
                const update = stable ? item.stableUpdate : item.update;
                const installed = item.installed.kind === "locked";
                const target = installed && update.kind === "target" ? update.candidate
                    : item.installed.kind === "absent" && choice.kind === "candidate" ? choice.candidate : undefined;
                if (target?.eligibility !== "eligible" || target.relation === "unknown") return [];
                return [{
                    kind: installed ? "upgrade" : "install", packageId: item.packageId,
                    versionRange: `=${target.version}`, source: target.source, includePrerelease: target.classification === "prerelease"
                }];
            };
            const updateAllFor = (stable: boolean): CandidateBulkAction => {
                const installed = items.filter((item) => item.installed.kind !== "absent");
                const intents = installed.flatMap((item) => intent(item, stable));
                const exclusions = installed.flatMap((item) => {
                    const update = stable ? item.stableUpdate : item.update;
                    return update.kind === "target" ? [] : [{ packageId: item.packageId, update }];
                });
                const blocked = installed.some((item) => {
                    const choice = stable ? item.projectLatestStable : item.projectLatest;
                    return item.installed.kind === "unknown" || choice.kind === "unknown" || choice.kind === "ambiguous";
                });
                return {
                    targetCount: intents.length, intents: base.catalogComplete && !blocked && intents.length <= 256 ? intents : [],
                    exclusions: exclusions.slice(0, 256), omittedExclusions: Math.max(0, exclusions.length - 256)
                };
            };
            const installUpdateFor = (stable: boolean): CandidateBulkAction => {
                const intents = selectedItems.flatMap((item) => intent(item, stable));
                return { targetCount: intents.length, intents: completeSelection && intents.length === selectedItems.length ? intents : [], exclusions: [], omittedExclusions: 0 };
            };
            const updateAll = updateAllFor(false);
            const updateAllStable = updateAllFor(true);
            const installUpdate = installUpdateFor(false);
            const installUpdateStable = installUpdateFor(true);
            const differs = (left: CandidateBulkAction, right: CandidateBulkAction) => left.targetCount !== right.targetCount || JSON.stringify(left.intents) !== JSON.stringify(right.intents);
            const sources = params.view.sources;
            const bulk: CandidateBulkActions = {
                updateAll, ...(differs(updateAll, updateAllStable) ? { updateAllStable } : {}),
                installUpdate, ...(differs(installUpdate, installUpdateStable) ? { installUpdateStable } : {}),
                remove: completeSelection && selectedItems.every((item) => item.direct)
                    ? selectedItems.map(({ packageId }) => ({ kind: "remove", packageId })) : [],
                reinstall: completeSelection && selectedItems.every((item) => item.installed.kind === "locked")
                    ? selectedItems.map(({ packageId }) => ({ kind: "reinstall", packageId, source: sources?.find((item) => item.packageId === packageId)?.source })) : []
            };
            return this.value({ ...base, view: "summary", items: [], bulk });
        }
        const filter = params.view.filter;
        const matches = filter === undefined ? items : items.filter((item) => {
            const providers = item.providers.map(({ source }) => source.kind === "user_package"
                ? { kind: "user-package", label: "Local avatar tools" }
                : source.repository_id === LOCAL_REPOSITORY_ID
                    ? { kind: "local", label: "Local packages" }
                    : { kind: "remote", label: "Example packages" });
            const installed = item.installed.kind !== "absent";
            const search = filter.search?.toLowerCase();
            const matchesSearch = search === undefined || [item.packageId, item.displayName ?? "", ...providers.map(({ label }) => label)]
                .some((name) => name.toLowerCase().includes(search));
            const matchesStatus = filter.status === undefined
                || (filter.status === "installed" && installed)
                || (filter.status === "available" && !installed && providers.length > 0)
                || (filter.status === "missing" && !installed && providers.length === 0 && item.latest.kind === "none");
            return matchesSearch && matchesStatus && (filter.source === undefined || providers.some(({ kind }) => kind === filter.source));
        });
        const offset = params.cursor?.offset ?? 0;
        const count = query.get("paged") ? 1 : params.limit ?? 64;
        return this.value({ ...base, nextCursor: offset + count < matches.length ? { snapshotFingerprint: fingerprint, queryFingerprint: HASH, offset: offset + count } : null, view: "summary", items: matches.slice(offset, offset + count) });
    }
    async packagePlanInstall(params: Parameters<GuiRpcClient["packagePlanInstall"]>[0]): ReturnType<GuiRpcClient["packagePlanInstall"]> {
        window.packageRequests.push({ method: "install", params });
        if (query.get("delayPackagePlan") === "1") await new Promise<void>((resolve) => { window.resumePackagePlan = resolve; });
        if (query.get("planError") === "1") return Promise.reject({ code: "package_source_ambiguous" });
        const plan = packagePlan("install");
        plan.changeSet.mutations = [{ kind: "replace", packageId: params.packageId, fromVersion: "1.2.3", toVersion: params.versionRange ?? "1.3.0" }];
        if (query.has("legacy-removals")) {
            plan.changeSet.formatVersion = 4;
            plan.changeSet.legacyRemovals = [
                { path: "Assets/OldTools", kind: "directory", packageId: params.packageId },
                { path: "Assets/OldTools.meta", kind: "file", packageId: params.packageId }
            ];
        }
        return this.value(plan);
    }
    packagePlanRemove(): ReturnType<GuiRpcClient["packagePlanRemove"]> { return this.value(packagePlan("remove")); }
    async packagePlanUpgrade(params: Parameters<GuiRpcClient["packagePlanUpgrade"]>[0]): ReturnType<GuiRpcClient["packagePlanUpgrade"]> {
        window.packageRequests.push({ method: "upgrade", params });
        if (query.get("delayPackagePlan") === "1") await new Promise<void>((resolve) => { window.resumePackagePlan = resolve; });
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
        return this.value({ operationId: this.beginOperation(), replayed: false });
    }

    userPackagesList(): ReturnType<GuiRpcClient["userPackagesList"]> {
        window.resourceRequests.push({ method: "packages.userPackages.list", params: {} });
        return this.value({ userPackages: this.userPackageEnrolled && !this.userPackageRemoved ? [this.currentUserPackage()] : [] });
    }
    async userPackageGet(userPackageId: string): ReturnType<GuiRpcClient["userPackageGet"]> {
        window.resourceRequests.push({ method: "packages.userPackages.get", params: { userPackageId } });
        if (!this.userPackageEnrolled || this.userPackageRemoved) throw { code: "user_package_not_found" };
        if (this.userDetailsUnavailable) throw { code: "store_unavailable" };
        const userPackage = this.currentUserPackage();
        if (query.has("hold-user-details")) await new Promise<void>((resolve) => window.addEventListener("test-user-details-ready", () => resolve(), { once: true }));
        return this.value({ userPackage });
    }
    userPackageEnroll(sourcePath: string): ReturnType<GuiRpcClient["userPackageEnroll"]> {
        window.resourceRequests.push({ method: "packages.userPackages.enroll", params: { sourcePath } });
        if (this.userPackageEnrolled && !this.userPackageRemoved) return Promise.reject({ code: "user_package_already_enrolled" });
        this.userPackageEnrolled = true; this.userPackageRemoved = false; this.userPackagePath = sourcePath; this.userPackageRevision = 1;
        return this.value({ userPackage: this.currentUserPackage(), replayed: false });
    }
    userPackageRefresh(userPackageId: string, expectedRevision: number): ReturnType<GuiRpcClient["userPackageRefresh"]> {
        window.resourceRequests.push({ method: "packages.userPackages.refresh", params: { userPackageId, expectedRevision } });
        if (query.has("user-refresh-error") && window.resourceRequests.filter((entry) => entry.method === "packages.userPackages.refresh").length === 1) return Promise.reject({ code: "store_unavailable" });
        if (expectedRevision !== this.userPackageRevision) return Promise.reject({ code: "revision_conflict" });
        this.userPackageRevision += 1;
        return this.value({ userPackage: this.currentUserPackage(), replayed: false });
    }
    userPackageRemove(userPackageId: string, expectedRevision: number): ReturnType<GuiRpcClient["userPackageRemove"]> {
        window.resourceRequests.push({ method: "packages.userPackages.remove", params: { userPackageId, expectedRevision } });
        if (query.has("resource-remove-error")) return Promise.reject({ code: "permission_denied" });
        if (expectedRevision !== this.userPackageRevision) return Promise.reject({ code: "revision_conflict" });
        this.userPackageRemoved = true;
        return this.value({ userPackageId, revision: expectedRevision + 1, removed: true, replayed: false });
    }
    private currentUserPackage() {
        return { ...userPackage(this.userPackageRevision), sourceRootPath: this.userPackagePath, ...(query.has("resource-lifecycle") && this.userPackageRevision > 1 ? { version: "1.3.0", archiveSha256: NEW_HASH } : {}) };
    }

    private registeredUnityInstallations() {
        return (query.has("h3-unity-discovery") && !this.unityDiscoveryComplete) || this.mode === "empty" || this.mode === "unity-zero"
            ? []
            : this.mode === "unity-multiple"
                ? [installation(), { ...installation(), installationId: "00000000-0000-4000-8000-000000000111", filesystemIdentity: "opaque-two", executablePath: query.has("windows-paths") ? "\\\\?\\UNC\\builds\\Editors\\Unity.exe" : "<private-editor-two>" }]
                : this.mode === "unity-migration"
                    ? [installation(), migrationInstallation()]
                : [installation()];
    }
    async unityInstallationsList(_cursor?: string, includeVersionOptions?: boolean): ReturnType<GuiRpcClient["unityInstallationsList"]> {
        if (query.has("holdUnity")) {
            this.unityReady ??= new Promise<void>((resolve) => window.addEventListener("test-unity-ready", () => resolve(), { once: true }));
            await this.unityReady;
        }
        const installations = this.registeredUnityInstallations();
        if (!includeVersionOptions) return this.value({ installations, replayed: false });
        const versions = [...new Set(installations.map((row) => row.unityVersion))].sort();
        const defaultVersion = versions.length === 1 ? versions[0] : undefined;
        return this.value({ installations, replayed: false, versionOptions: { versions, defaultVersion } });
    }
    unityInstallationGet(): ReturnType<GuiRpcClient["unityInstallationGet"]> { return this.value({ installation: installation(), replayed: false }); }
    unityInstallationRegister(executablePath: string): ReturnType<GuiRpcClient["unityInstallationRegister"]> {
        window.h3Requests.push({ method: "unity.installations.register", params: { executablePath } });
        if (query.has("h3-unity-register-error") && window.h3Requests.filter((request) => request.method === "unity.installations.register").length === 1) return Promise.reject({ code: "unity_installation_invalid" });
        return this.value({ installation: installation(), replayed: false });
    }
    unityInstallationRemove(): ReturnType<GuiRpcClient["unityInstallationRemove"]> { return this.value({ installationId: INSTALLATION_ID, removed: true, replayed: false }); }
    unityInstallationsRefresh(): ReturnType<GuiRpcClient["unityInstallationsRefresh"]> {
        window.unityRequests.push({ method: "unityInstallationsRefresh", params: {} });
        if (query.has("h3-unity-discovery-error") && window.unityRequests.filter((request) => request.method === "unityInstallationsRefresh").length === 1) return Promise.reject({ code: "unity_version_unverified" });
        if (query.get("hub-refresh") === "error") return Promise.reject({ code: "internal_error" });
        this.unityDiscoveryComplete = true;
        return this.value({ installations: [installation()], replayed: false });
    }
    unityProjectLaunchConfigGet(): ReturnType<GuiRpcClient["unityProjectLaunchConfigGet"]> {
        const defaultArguments = this.settings.settings.unity.defaultLaunchArguments ?? [];
        return this.value({ config: launchConfig(this.launchConfigRevision === 0 ? [] : this.launchArguments, this.launchConfigRevision), defaultArguments, effectiveArguments: this.launchConfigRevision === 0 ? defaultArguments : this.launchArguments });
    }
    unityProjectLaunchConfigSet(projectId: string, arguments_: string[], expectedRevision: number): ReturnType<GuiRpcClient["unityProjectLaunchConfigSet"]> {
        window.unityRequests.push({ method: "unity.projectLaunchConfig.set", params: { projectId, arguments: structuredClone(arguments_), expectedRevision } });
        if (expectedRevision !== this.launchConfigRevision) return Promise.reject({ code: "revision_conflict" });
        this.launchArguments = arguments_;
        this.launchConfigRevision += 1;
        return this.value({ config: launchConfig(this.launchArguments, this.launchConfigRevision), changed: true, replayed: false });
    }
    unityProjectLaunchConfigClear(projectId: string, expectedRevision: number): ReturnType<GuiRpcClient["unityProjectLaunchConfigClear"]> {
        window.unityRequests.push({ method: "unity.projectLaunchConfig.clear", params: { projectId, expectedRevision } });
        if (expectedRevision !== this.launchConfigRevision) return Promise.reject({ code: "revision_conflict" });
        this.launchArguments = [];
        const nextRevision = this.launchConfigRevision + 1;
        this.launchConfigRevision = 0;
        return this.value({ config: launchConfig(this.launchArguments, nextRevision), changed: true, replayed: false });
    }
    unityWriterState(): ReturnType<GuiRpcClient["unityWriterState"]> { return this.value({ projectId: PROJECT_ID, state: "not_observed", evidence: [], checkedAtMs: 1_700_000_000_000 }); }
    unityLaunchOptions(projectId: string, expectedProjectRevision: number, includeMigrationVersionOptions?: boolean): ReturnType<GuiRpcClient["unityLaunchOptions"]> {
        window.unityRequests.push({ method: "unity.launchOptions", params: { projectId, expectedProjectRevision } });
        if (query.has("unityOptionsError")) return Promise.reject({ code: "revision_conflict" });
        const exactMatchingInstallations = this.mode === "unity-zero"
            ? []
            : this.mode === "unity-multiple"
                ? [installation(), { ...installation(), installationId: "00000000-0000-4000-8000-000000000111", filesystemIdentity: "opaque-two", executablePath: query.has("windows-paths") ? "\\\\?\\UNC\\builds\\Editors\\Unity.exe" : "<private-editor-two>" }]
                : this.currentUnityVersion === migrationInstallation().unityVersion
                    ? [migrationInstallation()]
                    : [installation()];
        const result = { projectId, projectRevision: this.currentProjectRevision, projectUnityVersion: this.currentUnityVersion, exactMatchingInstallations };
        if (!includeMigrationVersionOptions) return this.value(result);
        const installed = this.registeredUnityInstallations();
        const versions = [...new Set(installed.map((row) => row.unityVersion))].filter((version) => version !== this.currentUnityVersion).sort();
        const migrationVersionOptions = versions.map((unityVersion) => {
            const installations = installed.filter((row) => row.unityVersion === unityVersion).map(({ installationId, architecture }) => ({ installationId, architecture }));
            return { unityVersion, installations, defaultInstallationId: installations.length === 1 ? installations[0]?.installationId : undefined };
        });
        return this.value({ ...result, migrationVersionOptions });
    }
    unityLaunch(projectId: string, installationId: string, expectedProjectRevision: number): ReturnType<GuiRpcClient["unityLaunch"]> {
        window.unityRequests.push({ method: "unity.launch", params: { projectId, installationId, expectedProjectRevision } });
        if (query.has("projectModified")) this.projectLaunchTimes.set(projectId, 1_700_216_000_000);
        if (query.has("unityLaunchError")) return Promise.reject({ code: query.get("unityLaunchError") === "unity_project_running" ? "unity_project_running" : "unity_launch_failed" });
        const acceptedLaunch = { ...launch(), projectId, installationId };
        return new Promise((resolve) => window.setTimeout(() => resolve({ launch: acceptedLaunch, replayed: false }), 100));
    }
    projectPlanUnityMigration(_projectId: string, targetInstallationId: string): ReturnType<GuiRpcClient["projectPlanUnityMigration"]> {
        window.unityRequests.push({ method: "projects.planUnityMigration", params: { projectId: _projectId, targetInstallationId } });
        const targetUnityVersion = targetInstallationId === MIGRATION_INSTALLATION_ID ? "2022.3.23f1" : "2022.3.22f1";
        return this.value({ kind: "planned", replayed: false, plan: { planId: PLAN_ID, projectId: PROJECT_ID, sourceUnityVersion: "2022.3.22f1", targetUnityVersion, targetInstallationId, classification: { kind: "patch_or_minor_upgrade", supportedForApply: true }, expiresAtMs: 1_800_000_000_000 } });
    }
    projectApplyUnityMigration(): ReturnType<GuiRpcClient["projectApplyUnityMigration"]> {
        this.pendingMigrationTarget = "2022.3.23f1";
        return this.value({ operationId: this.beginOperation(), replayed: false });
    }

async projectTemplatesList(cursor?: string): ReturnType<GuiRpcClient["projectTemplatesList"]> {
        window.templateRequests.push({ method: "projectTemplates.list", params: cursor === undefined ? {} : { cursor } });
        if (this.templatesUnavailable) throw { code: "internal_error" };
        if (query.has("holdTemplates") && window.heldTemplateReads === 0) {
            window.heldTemplateReads += 1;
            await new Promise<void>((resolve) => window.addEventListener("test-templates-ready", () => resolve(), { once: true }));
        }
        if (query.has("template-pages")) {
            if (cursor === undefined) return this.value({ templates: Array.from({ length: 100 }, (_, index) => catalogTemplate(index + 1)), nextCursor: TEMPLATE_PAGE_CURSOR });
            if (cursor !== TEMPLATE_PAGE_CURSOR) throw { code: "invalid_request" };
            this.templatePageReads += 1;
            if (query.has("template-page-error") && this.templatePageReads === 1) throw { code: "daemon_unavailable" };
            if (query.has("hold-template-page") && this.templatePageReads === 1) {
                await new Promise<void>((resolve) => window.addEventListener("test-template-page-ready", () => resolve(), { once: true }));
            }
            window.templatePageCompletions += 1;
            return this.value({ templates: [catalogTemplate(101)] });
        }
        return this.value({ templates: this.projectTemplates });
    }


    selectProjectTemplateFile(mode: "open" | "save", fileName?: string): ReturnType<GuiRpcClient["selectProjectTemplateFile"]> {
        window.templateRequests.push({ method: "selectProjectTemplateFile", params: { mode, fileName } });
        if (query.has("template-file-cancel")) return this.value(undefined);
        return this.value(mode === "open" ? "C:\\Fixture\\input.alcomdtemplate" : "C:\\Fixture\\" + fileName);
    }

    projectTemplatesSave(params: Parameters<GuiRpcClient["projectTemplatesSave"]>[0]): ReturnType<GuiRpcClient["projectTemplatesSave"]> {
        window.templateRequests.push({ method: "projectTemplates.save", params: structuredClone(params) });
        return this.beginTemplateOperation("projectTemplates.save", params.idempotencyKey, () => {
            const { source, expectedRevision } = params;
            if (query.has("template-package-error")) throw { code: "template_dependency_unsatisfied", packageId: "com.example.missing" };
            const archive = source.kind === "file"
                ? query.has("template-file-replace") ? { id: USER_TEMPLATE_ID, name: "Imported replacement" } : { id: "com.example.imported", name: "Imported template" }
                : source;
            const existing = this.projectTemplates.find((item) => item.id === archive.id);
            if (expectedRevision === undefined && existing !== undefined) throw { code: "template_conflict" };
            if (expectedRevision !== undefined && existing?.revision !== expectedRevision) throw { code: "revision_conflict" };
            if (existing?.locked) throw { code: "template_builtin_immutable" };
            if (source.kind === "project" && (source.projectId !== PROJECT_ID || source.expectedProjectRevision !== this.currentProjectRevision)) throw { code: "revision_conflict" };
            const editorVersion = source.kind === "manual" ? source.unityVersion : source.kind === "project" ? this.currentUnityVersion : "2022.3.22f1";
            const template = { id: archive.id, name: archive.name, editorVersion,
                revision: ++this.templateRevision, locked: false };
            this.projectTemplates = [...this.projectTemplates.filter((item) => item.id !== template.id), template];
            browserCoreEvents.emit("project-template", TEMPLATE_EVENT_ID, "project-template.changed", template.revision);
            return { template };
        });
    }

    projectTemplatesExport(params: Parameters<GuiRpcClient["projectTemplatesExport"]>[0]): ReturnType<GuiRpcClient["projectTemplatesExport"]> {
        window.templateRequests.push({ method: "projectTemplates.export", params: structuredClone(params) });
        return this.beginTemplateOperation("projectTemplates.export", params.idempotencyKey, () => {
            if (!this.projectTemplates.some((item) => item.id === params.id && item.revision === params.revision)) throw { code: "revision_conflict" };
            return { id: params.id, revision: params.revision };
        });
    }

    private beginTemplateOperation(kind: string, key: string, complete: () => object) {
        const previous = this.templateRequestsByKey.get(key);
        if (previous !== undefined) return this.value({ operationId: previous, replayed: true });
        const operationId = crypto.randomUUID();
        const operation: Operation = { ...runningOperation(), operationId, kind, revision: 1 };
        this.genericTasks.set(operationId, operation);
        this.templateRequestsByKey.set(key, operationId);
        browserCoreEvents.emit("operation", operationId, "operation.created", 1);
        setTimeout(() => {
            let next: Operation;
            try { next = { ...operation, state: "succeeded", revision: 2, result: complete() }; }
            catch (caught: unknown) {
                const failure = caught as { code?: string; packageId?: string };
                next = { ...operation, state: "failed", revision: 2, errorCode: failure.code ?? "internal_error",
                    ...(failure.packageId === undefined ? {} : { result: { packageId: failure.packageId } }) };
            }
            this.genericTasks.set(operationId, next);
            browserCoreEvents.emit("operation", operationId, `operation.${next.state}`, next.revision);
        }, 250);
        return query.has("template-operation-unknown") ? Promise.reject({ code: "daemon_unavailable" }) : this.value({ operationId, replayed: false });
    }

    projectTemplatesRemove(params: Parameters<GuiRpcClient["projectTemplatesRemove"]>[0]): ReturnType<GuiRpcClient["projectTemplatesRemove"]> {
        window.templateRequests.push({ method: "projectTemplates.remove", params });
        const existing = this.projectTemplates.find((item) => item.id === params.id);
        if (existing?.revision !== params.expectedRevision) return Promise.reject({ code: "revision_conflict" });
        if (existing.locked) return Promise.reject({ code: "template_builtin_immutable" });
        this.projectTemplates = this.projectTemplates.filter((item) => item !== existing);
        const revision = ++this.templateRevision;
        browserCoreEvents.emit("project-template", TEMPLATE_EVENT_ID, "project-template.changed", revision);
        return this.value({ id: params.id, revision });
    }

    async projectsCreate(params: Parameters<GuiRpcClient["projectsCreate"]>[0]): ReturnType<GuiRpcClient["projectsCreate"]> {
        window.templateRequests.push({ method: "projects.create", params: structuredClone(params) });
        const { idempotencyKey, ...input } = params;
        const fingerprint = JSON.stringify(input);
        const previous = this.creationRequests.get(idempotencyKey);
        if (previous !== undefined) {
            if (previous.fingerprint !== fingerprint) throw { code: "idempotency_conflict" };
            return this.value({ operationId: previous.operationId, projectId: previous.projectId, replayed: true });
        }
        if (query.has("hold-create-request")) {
            await new Promise<void>((resolve) => window.addEventListener("test-create-request-ready", () => resolve(), { once: true }));
        }
        if (this.mode === "create-error") throw { code: "project_create_target_exists" };
        if (input.template !== undefined) {
            const requested = input.template;
            const record = this.projectTemplates.find(({ id }) => id === requested.id)
                ?? (query.has("template-pages") ? Array.from({ length: 101 }, (_, index) => catalogTemplate(index + 1)).find(({ id }) => id === requested.id) : undefined);
            if (record?.revision !== requested.revision) throw { code: "revision_conflict" };
            if (record.editorVersion !== undefined && record.editorVersion !== input.unityVersion) throw { code: "invalid_request" };
        }
        const projectId = this.creationRequests.size === 0 ? CREATED_PROJECT_ID : crypto.randomUUID();
        const created = project(projectId, input.name);
        created.unityVersion = input.unityVersion;
        const operationId = this.beginOperation("projects.create", undefined, created);
        this.creationRequests.set(idempotencyKey, { fingerprint, projectId, operationId });
        if (query.has("create-unknown")) throw { code: "daemon_unavailable" };
        return this.value({ operationId, projectId, replayed: false });
    }

    async backupsList(projectId?: string, cursor?: string): ReturnType<GuiRpcClient["backupsList"]> {
        if (query.has("h3")) {
            window.h3Requests.push({ method: "backups.list", params: { projectId, cursor } });
            if (query.has("h3-list-error") && window.h3Requests.filter((request) => request.method === "backups.list").length === 1) throw { code: "daemon_unavailable" };
            return this.value({ backups: query.has("h3-create") && window.h3Operation.state !== "succeeded" ? [] : [{ ...backup(), backupId: cursor === undefined ? BACKUP_ID : "00000000-0000-4000-8000-000000000611" }], ...(cursor === undefined && query.has("h3-pages") ? { nextCursor: "older-backups" } : {}) });
        }
        if (query.has("holdBackups")) await new Promise<void>((resolve) => window.addEventListener("test-backups-ready", () => resolve(), { once: true }));
        return this.value({ backups: this.mode === "empty" ? [] : [backup()] });
    }
    backupGet(backupId: string): ReturnType<GuiRpcClient["backupGet"]> { return this.value({ ...backup(), backupId }); }
    backupCreate(projectId: string, expectedRevision: number, compressionMode?: "store" | "fast" | "maximum", excludeVpmPackages?: boolean, includeLibrary?: boolean): ReturnType<GuiRpcClient["backupCreate"]> {
        window.h3Requests.push({ method: "backups.create", params: { projectId, expectedRevision, compressionMode, excludeVpmPackages, includeLibrary } });
        if (query.has("h3-backup-create-error")) return Promise.reject({ code: query.get("h3-backup-create-error") });
        return this.value({ operationId: query.has("h3") ? OPERATION_ID : this.beginOperation("backups.create"), backupId: BACKUP_ID, replayed: false });
    }
    backupPlanRestore(backupId: string, targetParent: string, targetLeaf: string): ReturnType<GuiRpcClient["backupPlanRestore"]> {
        window.h3Requests.push({ method: "backups.planRestore", params: { backupId, targetParent, targetLeaf } });
        if (this.mode === "restore-error") return Promise.reject({ code: "backup_target_exists" });
        this.pendingProject = project(RESTORED_PROJECT_ID, targetLeaf);
        return this.value({ planId: PLAN_ID, projectId: RESTORED_PROJECT_ID, backupId, target: { parent: targetParent, leaf: targetLeaf, mustBeAbsent: true }, archiveSha256: HASH, packagesRequireResolve: query.has("h3-excluded-packages"), excludedPackages: [], planFingerprint: HASH });
    }
    backupApplyRestore(planId: string): ReturnType<GuiRpcClient["backupApplyRestore"]> {
        window.h3Requests.push({ method: "backups.applyRestore", params: { planId } });
        if (query.has("h3-restore-unknown")) return Promise.reject({ code: "daemon_unavailable" });
        return this.value({ operationId: query.has("h3") ? OPERATION_ID : this.beginOperation("backups.restore"), projectId: RESTORED_PROJECT_ID, replayed: false });
    }

    extensionsList(): ReturnType<GuiRpcClient["extensionsList"]> { return this.value({ extensions: this.mode === "empty" ? [] : [extension(DISCORD_ID), extension(MCP_ID)] }); }
    extensionPermissionsList(extensionId: string): ReturnType<GuiRpcClient["extensionPermissionsList"]> { return this.value({ extensionId, grantRevision: 3, requiredPermissions: ["projects.read"], optionalPermissions: ["background.run"], grants: [] }); }
    extensionGet(extensionId: string): ReturnType<GuiRpcClient["extensionGet"]> { return this.value({ extension: extension(extensionId) }); }
    extensionPlanInstall(): ReturnType<GuiRpcClient["extensionPlanInstall"]> { return this.value({ plan: extensionPlan("install") }); }
    extensionApplyInstall(): ReturnType<GuiRpcClient["extensionApplyInstall"]> { return this.value({ operationId: this.beginOperation("extensions.install"), replayed: false }); }
    extensionEnable(extensionId: string): ReturnType<GuiRpcClient["extensionEnable"]> { return this.value({ extension: { ...extension(extensionId), desiredState: "enabled" } }); }
    extensionDisable(extensionId: string): ReturnType<GuiRpcClient["extensionDisable"]> { return this.value({ extension: { ...extension(extensionId), desiredState: "installed_disabled" } }); }
    extensionPlanUninstall(): ReturnType<GuiRpcClient["extensionPlanUninstall"]> { return this.value({ plan: extensionPlan("uninstall") }); }
    extensionApplyUninstall(): ReturnType<GuiRpcClient["extensionApplyUninstall"]> { return this.value({ operationId: this.beginOperation("extensions.uninstall"), replayed: false }); }
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
const TEMPLATE_ID = "com.vrclearn.alcomd.templates.vrchat.avatars";
const WORLD_TEMPLATE_ID = "com.vrclearn.alcomd.templates.vrchat.worlds";
const USER_TEMPLATE_ID = "com.example.fixture";
const TEMPLATE_EVENT_ID = "00000000-0000-4000-8000-000000000301";
const NEW_HASH = "b".repeat(64);
const TEMPLATE_PAGE_CURSOR = "opaque template cursor:100/+=";
const BACKUP_ID = "00000000-0000-4000-8000-000000000104";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";
const PLAN_ID = "00000000-0000-4000-8000-000000000106";
const DISCORD_ID = "com.vrclearn.discord";
const MCP_ID = "com.vrclearn.mcp-management";
const HASH = "a".repeat(64);

function projectListPage(projects: ProjectSnapshot[], cursor: RegistryCursor | undefined, view: ProjectListView | undefined, expectedViewSequence: number | undefined, paginate: boolean): ProjectsListResult {
    const viewSequence = view === undefined ? undefined : 1;
    if (view !== undefined) {
        if (cursor !== undefined && expectedViewSequence !== viewSequence) throw { code: "revision_conflict" };
        const search = view.search?.toLowerCase() ?? "";
        projects = projects.filter((item) => (item.displayName ?? "").toLowerCase().includes(search)).sort((left, right) => {
            const favorite = Number(right.favorite === true) - Number(left.favorite === true);
            if (favorite !== 0) return favorite;
            let order = 0;
            switch (view.sort) {
                case "name": order = (left.displayName ?? "").toLowerCase().localeCompare((right.displayName ?? "").toLowerCase()); break;
                case "type": order = left.projectType.localeCompare(right.projectType); break;
                case "unity": order = left.unityVersion.localeCompare(right.unityVersion); break;
                case "added": order = (left.registeredAtMs ?? 0) - (right.registeredAtMs ?? 0); break;
                case "modified": {
                    if (left.lastModifiedAtMs === undefined && right.lastModifiedAtMs !== undefined) return 1;
                    if (right.lastModifiedAtMs === undefined && left.lastModifiedAtMs !== undefined) return -1;
                    order = (left.lastModifiedAtMs ?? 0) - (right.lastModifiedAtMs ?? 0);
                    break;
                }
            }
            return (view.descending ? -order : order) || (left.projectId ?? "").localeCompare(right.projectId ?? "");
        });
    }
    if (!paginate) return { projects, viewSequence };
    const offset = cursor === undefined ? 0 : projects.findIndex((item) => item.projectId === cursor.id) + 1;
    const item = projects[offset];
    const nextCursor = projects.length > offset + 1 && item?.projectId && item.registeredAtMs !== undefined
        ? { id: item.projectId, registeredAtMs: item.registeredAtMs } : undefined;
    return { projects: item === undefined ? [] : [item], viewSequence, nextCursor };
}

function project(projectId = PROJECT_ID, name = "Sample", favorite = false) {
    const rootPath = query.has("windows-paths") ? `\\\\?\\C:\\Projects\\${name}` : projectId === PROJECT_ID && name === "Sample" ? "<private-project>" : `C:\\Fixture\\${name}`;
    const displayName = rootPath === "<private-project>" ? rootPath : name;
    return { projectId, registeredAtMs: 1_690_000_000_000, lastModifiedAtMs: 1_700_000_000_000, favorite, rootPath, displayName, projectType: "avatars", unityVersion: "2022.3.22f1", vpmManifest: "valid", upmManifest: "valid", directDependencies: [{ packageId: "com.example.avatar", value: "^1.2.0" }], lockedDependencies: [{ packageId: "com.example.avatar", value: "1.2.3" }], issues: [], revision: 2 };
}
function repository() { return { repositoryId: REPOSITORY_ID, source: { kind: "remote" as const, url: "https://packages.example.invalid/index.json" }, actions: { canRemove: true, canEditAuthentication: true }, declaredId: "example", name: "Example packages", declaredUrl: "https://packages.example.invalid/index.json", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function localRepository() { return { repositoryId: LOCAL_REPOSITORY_ID, source: { kind: "local" as const, path: "<private-local-repository>" }, actions: { canRemove: true, canEditAuthentication: false }, declaredId: "local-example", name: "Local packages", issues: [], revision: 2, refreshedAtMs: 1_700_000_000_000 }; }
function userPackage(revision = 1) { return { userPackageId: USER_PACKAGE_ID, sourceRootPath: "<private-user-package>", packageId: "com.example.avatar", version: "1.2.3", displayName: "Local avatar tools", revision, archiveSha256: HASH, createdAtMs: 1_700_000_000_000, updatedAtMs: revision === 1 ? 1_700_000_000_000 : 1_700_000_001_000 }; }
function installation() { return { installationId: INSTALLATION_ID, executablePath: query.has("windows-paths") ? "\\\\?\\C:\\Program Files\\Unity\\Hub\\Editor\\2022.3.22f1\\Editor\\Unity.exe" : "<private-editor>", filesystemIdentity: "opaque", unityVersion: "2022.3.22f1", architecture: "x86_64", sourceKind: "manual", revision: 2, observedAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 }; }
function migrationInstallation() { return { ...installation(), installationId: MIGRATION_INSTALLATION_ID, executablePath: "<private-editor-migration>", filesystemIdentity: "opaque-migration", unityVersion: "2022.3.23f1" }; }
function launchConfig(arguments_: string[], revision: number) { return { projectId: PROJECT_ID, arguments: arguments_, revision, updatedAtMs: 1_700_000_000_000 }; }
function launch() { return { launchId: "00000000-0000-4000-8000-000000000107", projectId: PROJECT_ID, installationId: INSTALLATION_ID, state: "opening", spawnAccepted: true, createdAtMs: 1_700_000_000_000 }; }
function builtinTemplates(): ProjectTemplateRecord[] {
    const versions = ["2022.3.22f1", "2019.4.31f1"];
    return [false, true].flatMap((worlds) => versions.map((editorVersion, index) => {
        const id = worlds ? WORLD_TEMPLATE_ID : TEMPLATE_ID;
        const year = editorVersion.slice(0, 4);
        return { id: id + "." + year,
            name: `${worlds ? "VRChat Worlds" : "VRChat Avatars"} ${year}`, editorVersion,
            revision: 2 + index + (worlds ? versions.length : 0), locked: true };
    }));
}

function userTemplate(id: string, name: string): ProjectTemplateRecord {
    return { id, name, editorVersion: "2022.3.22f1", revision: 8, locked: false };
}

function catalogTemplate(index: number) {
    const suffix = String(index).padStart(3, "0");
    return { ...userTemplate("com.example.catalog-" + suffix, "Catalog template " + suffix), revision: 10 + index };
}

function backup() { return { backupId: BACKUP_ID, sourceProjectId: PROJECT_ID, archiveSha256: HASH, archiveBytes: 4096, formatVersion: 1, createdAtMs: 1_700_000_000_000, compressionMode: "fast", excludeVpmPackages: true }; }
function runningOperation(): Operation { return { operationId: OPERATION_ID, kind: "packages.apply", state: "running", revision: 3, createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000, progress: { phase: "extracting" } }; }
function packagePlan(action: PackagePlan["action"]): PackagePlan { return { planId: PLAN_ID, action, state: "unapplied", projectId: PROJECT_ID, projectRevision: 2, changeSetFingerprint: HASH, changeSet: { formatVersion: 3, mutations: [{ kind: action === "remove" ? "remove" : "install", packageId: "com.example.avatar", ...(action === "remove" ? { fromVersion: "1.2.3", toVersion: null } : { fromVersion: null, toVersion: "1.2.3" }) }], dependencyEdges: [], vpmManifestSha256: HASH } }; }
function extension(extensionId: string): ExtensionRecord { return { extensionId, version: "1.0.0", apiMajor: 1, packageDigest: HASH, publisherFingerprint: `ed25519-sha256:${HASH}`, trustDecision: "official", desiredState: "enabled", quarantineState: "clear", runtimeState: "running", grantRevision: 3, lifecycleGeneration: 2, revision: 4, ui: { protocol: "portable-v1" } }; }
function extensionPlan(action: string) { return { planId: PLAN_ID, action, state: "unapplied", sourceKind: "local_owner_selected", extensionId: DISCORD_ID, version: "1.0.0", apiMajor: 1, profileVersion: 1, packageDigest: HASH, publisherFingerprint: `ed25519-sha256:${HASH}`, trustDecision: "official", dataDisposition: "retain_data", planFingerprint: HASH, uiProtocol: "portable-v1", requiredPermissions: ["projects.read"], optionalPermissions: ["background.run"] }; }

const root = document.getElementById("root");
if (root === null) throw new Error("Missing #root element");
const baseClient = new DeterministicGuiClient(mode);
const managementClient = query.has("h4-management") ? withH4ManagementClient(baseClient, query) : baseClient;
const browserClient = query.has("h4-portable") ? withH4PortableClient(managementClient, query) : managementClient;
const utilitiesClient = query.has("h5-utilities") ? withH5UtilitiesClient(browserClient, query) : browserClient;
const settingsClient = query.has("h5-settings") ? withH5SettingsClient(utilitiesClient, query) : utilitiesClient;
window.fixtureCore = withBrowserCoreEvents(settingsClient);
if (query.has("vcc-settings-fixture")) {
    const { mockIPC, mockWindows } = await import("@tauri-apps/api/mocks");
    mockWindows("main");
    mockIPC((command) => {
        if (command === "gui_vcc_registration_available") return true;
        if (command === "gui_register_vcc_links" || command === "gui_window_state_configure") return undefined;
        throw new Error(`Unexpected native fixture command: ${command}`);
    });
    Object.defineProperty(window, "isTauri", { value: true });
}
const pendingRepositoryLinks = query.has("repository-link-fixture") ? [
    "vcc://vpm/addRepo?url=https%3A%2F%2Frepository.test%2Findex.json&headers%5B%5D=Authorization%3ABearer%20fixture-link-token"
] : undefined;
const repositoryLinks = pendingRepositoryLinks ? {
    take: async () => pendingRepositoryLinks.shift() ?? null,
    listen: async (available: () => void) => {
        const receive = (event: Event) => {
            if (event instanceof CustomEvent && typeof event.detail === "string") {
                pendingRepositoryLinks.push(event.detail);
                available();
            }
        };
        window.addEventListener("fixture-repository-link", receive);
        return () => window.removeEventListener("fixture-repository-link", receive);
    },
} : undefined;
ReactDOM.createRoot(root).render(
    <React.StrictMode>
        {materialEvidence
            ? <MaterialFoundationEvidence />
            : operationLogEvidence ? <OperationLogEvidence />
            : <App client={window.fixtureCore} repositoryLinks={repositoryLinks} />}
    </React.StrictMode>
);
