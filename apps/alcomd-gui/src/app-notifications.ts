import { TASK_TITLES, type AcceptedGuiTask, type CoreEvents } from "./core-events";
import type { Operation } from "./core-models";
import type { RpcError } from "@alcomd/sdk";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";
import type { RepositoryListImportResult, RepositoryRefreshAllResult } from "@alcomd/sdk";

const controllers = new WeakMap<GuiRpcClient, AppNotifications>();

interface ActionFeedback { action: string; success?: boolean }
const ACTIONS = {
    projectRegister: { action: "Add existing project", success: true },
    projectRefresh: { action: "Refresh", success: true },
    projectUnregister: { action: "Remove project from list", success: true },
    projectSetFavorite: { action: "Change project favorite" },
    repositoryRegister: { action: "Add repository", success: true },
    repositoryUnregister: { action: "Remove repository", success: true },
    repositoryRefresh: { action: "Refresh", success: true },
    repositoriesRefreshAll: { action: "Refresh", success: true },
    repositorySetDisplayName: { action: "Save repository name" },
    repositorySetCredentials: { action: "Repository authentication", success: true },
    repositoryListImportFile: { action: "Import list", success: true },
    repositoryListExportFile: { action: "Export list", success: true },
    repositoryListInspectFile: { action: "Import list" },
    repositoryListPreviewExport: { action: "Export list" },
    repositoryListCancelRegistration: { action: "Stop import" },
    userPackageEnroll: { action: "Add user package", success: true },
    userPackageRefresh: { action: "Refresh", success: true },
    userPackageRemove: { action: "Remove user package", success: true },
    unityInstallationRegister: { action: "Register", success: true },
    unityInstallationRemove: { action: "Remove Unity installation", success: true },
    unityInstallationsRefresh: { action: "Discover and refresh", success: true },
    unityProjectLaunchConfigSet: { action: "Edit launch arguments", success: true },
    unityProjectLaunchConfigClear: { action: "Restore defaults", success: true },
    projectTemplatesRemove: { action: "Remove template", success: true },
    extensionEnable: { action: "Enable", success: true },
    extensionDisable: { action: "Disable", success: true },
    extensionSetGrant: { action: "Grant", success: true },
    extensionRevokeGrant: { action: "Revoke", success: true },
    openProjectDirectory: { action: "Open Project Directory" },
    openPackageLink: { action: "Open link" },
    unityLaunch: { action: "Open Unity" },
    unityHubOpen: { action: "Open Unity Hub" },
    openUnityHubDownload: { action: "Download Unity Hub" },
    selectDirectory: { action: "Choose directory" },
    selectFile: { action: "Select" },
    selectUnityHub: { action: "Select" },
    selectProjectTemplateFile: { action: "Select" },
    selectRepositoryListFile: { action: "Select" },
    packagePlanInstall: { action: "Package changes" },
    packagePlanRemove: { action: "Package changes" },
    packagePlanUpgrade: { action: "Package changes" },
    packagePlanDowngrade: { action: "Package changes" },
    packagePlanResolve: { action: "Package changes" },
    packagePlanReinstall: { action: "Package changes" },
    packagePlanBulk: { action: "Package changes" },
    projectPlanCopy: { action: "Copy project" },
    projectPlanDeleteDirectory: { action: "Delete project directory" },
    projectPlanUnityMigration: { action: "Unity migration" },
    backupPlanRestore: { action: "Restore project" }
} satisfies Partial<Record<keyof GuiRpcClient, ActionFeedback>>;
const TEMPLATE_TASKS = { projectTemplatesSave: "Save template", projectTemplatesExport: "Export template" };

export interface AppNotification {
    id: number;
    key: string;
    action: string;
    outcome: "success" | "error" | "cancelled";
    method?: string;
    error?: RpcError;
    summary?: { succeeded: number; failed: number };
    operationId?: string;
    origin?: HTMLElement;
}

/** Disposable presentation state. Core responses and Operations remain authoritative. */
export class AppNotifications {
    readonly client: GuiRpcClient;
    private sequence = 0;
    private queue: AppNotification[] = [];
    private readonly listeners = new Set<() => void>();
    private readonly tasks = new Map<string, { task: AcceptedGuiTask; release(): void; replay: boolean }>();
    private readonly seenTasks = new Set<string>();
    private releaseTasks?: () => void;
    private releaseConnection?: () => void;
    private active = false;
    private readyOnce = false;
    private readonly pending = new Set<ReturnType<typeof setTimeout>>();

    constructor(private readonly events: CoreEvents) {
        const methods = new Map<PropertyKey, unknown>();
        this.client = new Proxy(events.client, { get: (target, key, receiver) => {
            const value: unknown = Reflect.get(target, key, receiver);
            if (typeof value !== "function") return value;
            if (!methods.has(key)) {
                const method = String(key);
                const feedback: ActionFeedback | undefined = Object.hasOwn(ACTIONS, method) ? ACTIONS[method as keyof typeof ACTIONS] : undefined;
                const taskTitle = Object.hasOwn(TASK_TITLES, method) ? TASK_TITLES[method as keyof typeof TASK_TITLES]
                    : Object.hasOwn(TEMPLATE_TASKS, method) ? TEMPLATE_TASKS[method as keyof typeof TEMPLATE_TASKS] : undefined;
                methods.set(key, !feedback && !taskTitle ? value.bind(target) : async (...args: unknown[]) => {
                    const origin = activeNotificationDialog();
                    try {
                        const result: unknown = await Reflect.apply(value, target, args);
                        if (Object.hasOwn(TEMPLATE_TASKS, method) && typeof result === "object" && result !== null && "operationId" in result && typeof result.operationId === "string") {
                            this.track({ operationId: result.operationId, title: taskTitle! }, "replayed" in result && result.replayed === true);
                        }
                        this.clear(method);
                        if (feedback?.success) {
                            let summary: AppNotification["summary"];
                            if (method === "repositoriesRefreshAll" || method === "repositoryListImportFile") {
                                const items = (result as RepositoryRefreshAllResult | RepositoryListImportResult).items;
                                const failed = items.filter((item) => ["failed", "unknown", "unprocessed"].includes(item.status)).length;
                                summary = { succeeded: items.length - failed, failed };
                            }
                            this.defer({ key: method, method, action: feedback.action, outcome: summary?.failed ? "error" : "success", origin, summary });
                        }
                        return result;
                    } catch (caught: unknown) {
                        this.defer({ key: method, method, action: feedback?.action ?? taskTitle!, outcome: "error", origin, error: normalizeRequestError(caught) });
                        throw caught;
                    }
                });
            }
            return methods.get(key);
        } });
        controllers.set(this.client, this);
    }

    async action<T>(action: string, work: () => Promise<T>, success = true): Promise<T> {
        const origin = activeNotificationDialog();
        try {
            const result = await work();
            this.clear(action);
            if (success) this.defer({ key: action, action, outcome: "success", origin });
            return result;
        } catch (caught) {
            this.defer({ key: action, action, outcome: "error", origin, error: normalizeRequestError(caught) });
            throw caught;
        }
    }

    failure(action: string, error: RpcError) {
        this.defer({ key: action, action, outcome: "error", origin: activeNotificationDialog(), error });
    }
    clear(key: string) {
        const queue = this.queue.filter((item) => item.key !== key);
        if (queue.length !== this.queue.length) { this.queue = queue; this.emit(); }
    }

    start() {
        if (this.active) return;
        this.active = true;
        this.releaseTasks = this.events.subscribeAcceptedTasks((task) => this.track(task, task.replayed === true));
        this.releaseConnection = this.events.subscribe(() => {
            const state = this.events.getSnapshot().kind;
            if (state === "ready") this.readyOnce = true;
            else if (this.readyOnce && ["connecting", "disconnected", "failed"].includes(state)) {
                this.tasks.forEach((entry) => { entry.replay = true; });
            }
        });
    }
    stop() {
        this.active = false;
        this.releaseTasks?.(); this.releaseConnection?.();
        this.tasks.forEach((entry) => entry.release()); this.tasks.clear();
        this.pending.forEach(clearTimeout); this.pending.clear();
        this.queue = []; this.emit();
    }
    subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
    getSnapshot = () => this.queue;
    dismiss = (id: number) => { this.queue = this.queue.filter((item) => item.id !== id); this.emit(); };
    private emit() { this.listeners.forEach((listener) => listener()); }
    private defer(item: Omit<AppNotification, "id">) {
        if (!this.active) return;
        // Allow the originating form to commit its own error/result before admission.
        const timer = setTimeout(() => {
            this.pending.delete(timer);
            if (!this.active) return;
            const next = { ...item, id: ++this.sequence };
            this.queue = [...this.queue.filter((entry) => entry.key !== next.key), next];
            this.emit();
        }, 0);
        this.pending.add(timer);
    }
    private track(task: AcceptedGuiTask, replay: boolean) {
        if (!this.active || this.seenTasks.has(task.operationId)) return;
        this.seenTasks.add(task.operationId);
        const entry = { task, replay, release: () => {} };
        this.tasks.set(task.operationId, entry);
        const changed = () => {
            const operation: Operation | undefined = this.events.operationSnapshot(task.operationId).value;
            if (!operation) return;
            if (!["succeeded", "failed", "cancelled"].includes(operation.state)) { entry.replay = false; return; }
            entry.release(); this.tasks.delete(task.operationId);
            if (entry.replay) return;
            this.defer({ key: task.operationId, action: task.title, operationId: task.operationId,
                outcome: operation.state === "failed" ? "error" : operation.state === "cancelled" ? "cancelled" : "success",
                ...(operation.state === "failed" ? { error: { code: operation.errorCode ?? "internal_error", message: "" } } : {}) });
        };
        entry.release = this.events.observeOperation(task.operationId, changed);
        changed();
    }
}

export function activeNotificationDialog(): HTMLElement | undefined {
    return [...document.querySelectorAll<HTMLElement>("md-dialog[open], dialog[open]")].at(-1);
}

export function notificationAlreadyPresented(item: AppNotification): boolean {
    const dialog = activeNotificationDialog();
    const visible = (node: HTMLElement) => node.getClientRects().length > 0
        && !node.closest('[data-dialog-closing="true"]') && (!dialog || dialog.contains(node));
    if (item.operationId && [...document.querySelectorAll<HTMLElement>("[data-notification-task]")]
        .some((node) => node.dataset.notificationTask === item.operationId && visible(node))) return true;
    if (item.error && !item.operationId && [...document.querySelectorAll<HTMLElement>("[data-notification-error]")]
        .some((node) => node.dataset.notificationError === item.error!.code && visible(node))) return true;
    return item.method !== undefined && item.origin !== undefined && [...item.origin.querySelectorAll<HTMLElement>("[data-notification-result]")]
        .some((node) => node.dataset.notificationResult?.split(" ").includes(item.method!) && node.getClientRects().length > 0);
}

/** Explicit read-only user action; automatic reads never enter this path. */
export function withActionNotification<T>(client: GuiRpcClient, action: string, work: () => Promise<T>, success = true): Promise<T> {
    return controllers.get(client)?.action(action, work, success) ?? work();
}
