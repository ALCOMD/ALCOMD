import type { CoreEvent, EventsBatch, EventsCursor, RpcError } from "@alcomd/sdk";
import type { Operation } from "./core-models";
import type { GuiEventMessage, GuiEventSubscription } from "./event-types";
import type { GuiRpcClient } from "./rpc";
import { normalizeRequestError } from "./request-errors";

export const EVENT_LIMITS = { mergeMs: 100, intervalMs: 250, maxWaitMs: 500, concurrency: 4, normalConcurrency: 3, dirtyKeys: 512,
    cachedRows: 5_000, cachedBytes: 8 * 1024 * 1024 } as const;
export type ChangeScope = "projects" | "operations" | "activity" | "diagnostics" | "repositories" | "user-packages" | "templates" | "unity" | "extensions" | "backups";
export interface EventConnectionState { kind: "connecting" | "ready" | "resyncing" | "disconnected" | "failed"; error?: RpcError; retained?: boolean }
interface Job { key: string; scopes: readonly string[]; run(): Promise<void>; dirty: boolean; blocked: boolean; urgent: boolean; version: number;
    first: number; last: number; running: boolean; retired: boolean }
interface OperationEntry { value?: Operation; error?: RpcError; version: number; subscribers: Set<() => void>; release?: () => void }
interface ViewRecord { data?: unknown; rows: number; bytes: number; visit: number; metadata: Record<string, unknown> }

const terminal = (value: Operation) => ["succeeded", "failed", "cancelled"].includes(value.state);
const uuid = (value: unknown) => typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value);
const aggregateKinds = new Set(["activity", "operation", "project", "repository", "unity-installation", "project-editor-preference", "project-unity-launch-config", "template", "user-package"]);
const READS = new Set(["operationsList", "operationGet", "activityList", "activityGet", "projectsList", "projectGet", "repositoriesList",
    "repositoryGet", "repositoryPackages", "userPackagesList", "userPackageGet", "unityInstallationsList", "unityInstallationGet", "unityProjectLaunchConfigGet",
    "unityWriterState", "unityLaunchOptions", "templatesList", "templateGet", "backupsList", "backupGet", "extensionsList", "extensionGet", "extensionPermissionsList", "packageQueryProjectCandidates", "systemStatus", "settingsGet"]);

/** One disposable notification/read coordinator per GUI connection. It owns no business state. */
export class CoreEvents {
    readonly client: GuiRpcClient;
    private readonly consumerId = crypto.randomUUID();
    private generation = 0;
    private active = false;
    private connectedBefore = false;
    private connection?: GuiEventSubscription;
    private connecting?: Promise<GuiEventSubscription>;
    private accepted?: EventsCursor;
    private subscriptionId?: string;
    private instanceId?: string;
    private receivedBatch = 0;
    private eventSequence = 0;
    private resourcesRevision = 0;
    private retryTimer?: ReturnType<typeof setTimeout>;
    private retryDelay = 250;
    private wakeTimer?: ReturnType<typeof setTimeout>;
    private readyResolve?: () => void;
    private readyReject?: (error: RpcError) => void;
    private readyPromise: Promise<void>;
    private ready = false;
    private state: EventConnectionState = { kind: "connecting" };
    private displayed = false;
    private readonly listeners = new Set<() => void>();
    private readonly jobs = new Map<string, Job>();
    private readonly dirty = new Set<string>();
    private readonly categoryVersions = new Map<string, number>();
    private running = 0;
    private operationJobs = 0;
    private pendingOperationObservers = false;
    private registeringObservers = false;
    private overflowSyncPending = false;
    private normalRunning = 0;
    private readsRunning = 0;
    private normalReads = 0;
    private readonly readQueue: Array<{ key: string; readonly urgent: boolean; start(): void; cancel(): void }> = [];
    private readonly reading = new Map<string, Promise<unknown>>();
    private readonly readCancellations = new Map<string, () => void>();
    private readonly delayedReads = new Map<string, () => void>();
    private readSequence = 0;
    private readonly operationReads = new Map<string, { version: number; readers: number }>();
    private readonly urgentOperations = new Map<string, number>();
    private readonly urgentCollections = new Map<string, number>();
    private readonly operations = new Map<string, OperationEntry>();
    private readonly operationListeners = new Set<(value: Operation) => void>();
    private readonly lookups = new Set<string>();
    private readonly consumedTerminal = new Set<string>();
    private readonly views = new Map<string, ViewRecord>();
    private visit = 0;

    constructor(private readonly source: GuiRpcClient) {
        this.readyPromise = this.newReadyPromise();
        const methods = new Map<PropertyKey, unknown>();
        this.client = new Proxy(source, { get: (target, key, receiver) => {
            const value: unknown = Reflect.get(target, key, receiver);
            if (typeof value !== "function") return value;
            if (!methods.has(key)) methods.set(key, READS.has(String(key))
                ? (...args: unknown[]) => !this.active && (key === "systemStatus" || key === "settingsGet")
                    ? Reflect.apply(value, target, args) : this.read(String(key), args, () => Reflect.apply(value, target, args))
                : value.bind(target));
            return methods.get(key);
        } });
    }

    private newReadyPromise() {
        const promise = new Promise<void>((resolve, reject) => { this.readyResolve = resolve; this.readyReject = reject; });
        // Connection failure may precede the first page read.
        void promise.catch(() => {});
        return promise;
    }
    getSnapshot = () => this.state;
    subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
    private publish(state: EventConnectionState) { this.state = { ...state, retained: this.displayed }; this.listeners.forEach((listener) => listener()); }
    noteDisplayedData() { if (!this.displayed) { this.displayed = true; this.publish(this.state); } }
    start() {
        if (this.active) return;
        this.active = true;
        document.addEventListener("visibilitychange", this.visible);
        void this.connect();
    }
    async stop() {
        if (!this.active) return;
        this.active = false;
        this.generation++;
        clearTimeout(this.retryTimer); clearTimeout(this.wakeTimer);
        document.removeEventListener("visibilitychange", this.visible);
        this.ready = false;
        this.readyReject?.({ code: "daemon_unavailable", message: "" });
        this.readQueue.splice(0).forEach((read) => read.cancel());
        [...this.readCancellations.values()].forEach((cancel) => cancel());
        const current = this.connection;
        const connecting = this.connecting;
        this.connection = undefined;
        await current?.close().catch(() => {});
        await connecting?.then((connection) => connection.close()).catch(() => {});
    }
    retry = () => {
        if (!this.active) return;
        clearTimeout(this.retryTimer);
        this.retryDelay = 250;
        void this.connect();
    };
    private async connect() {
        if (!this.active) return;
        const generation = ++this.generation;
        const prior = this.connection;
        this.connection = undefined;
        await prior?.close().catch(() => {});
        if (!this.active || generation !== this.generation) return;
        this.ready = false;
        if (this.connectedBefore) this.readyPromise = this.newReadyPromise();
        this.connectedBefore = true;
        this.subscriptionId = undefined;
        this.instanceId = undefined;
        this.receivedBatch = 0;
        this.publish({ kind: this.accepted === undefined ? "connecting" : "disconnected" });
        let opened: Promise<GuiEventSubscription>;
        opened = this.source.subscribeEvents({ consumerId: this.consumerId, generation, cursor: this.accepted }, (message) => {
            queueMicrotask(() => { void this.receive(message, opened).catch(() => this.failed({ code: "event_protocol_error", message: "" }, false)); });
        });
        this.connecting = opened;
        try {
            const connection = await opened;
            if (!this.active || generation !== this.generation) { await connection.close(); return; }
            this.connection = connection;
        } catch (caught: unknown) {
            if (this.active && generation === this.generation) {
                const error = normalizeRequestError(caught);
                this.failed(error, ["daemon_unavailable", "store_unavailable", "event_consumer_timeout", "event_subscription_replaced"].includes(error.code));
            }
        } finally { if (this.connecting === opened) this.connecting = undefined; }
    }

    private async receive(message: GuiEventMessage, opened: Promise<GuiEventSubscription>) {
        if (!this.active || message.consumerId !== this.consumerId || message.generation !== this.generation) return;
        if (message.type === "error") { this.failed(message.error, message.retryable); return; }
        if (message.type === "closed") {
            if (message.closed.subscriptionId !== this.subscriptionId || message.closed.instanceId !== this.instanceId) {
                this.failed({ code: "event_protocol_error", message: "" }, false); return;
            }
            this.failed(message.closed.error, message.closed.retryable); return;
        }
        if (message.type === "ready") {
            if (this.subscriptionId !== undefined) { this.failed({ code: "invalid_request", message: "" }, false); return; }
            this.subscriptionId = message.ready.subscriptionId;
            this.instanceId = message.ready.instanceId;
            if (typeof message.ready.resyncRequired !== "boolean" || !uuid(message.ready.subscriptionId) || !uuid(message.ready.instanceId)
                || message.ready.baselineCursor.instanceId !== this.instanceId || !Number.isSafeInteger(message.ready.baselineCursor.sequence)
                || message.ready.baselineCursor.sequence < 0 || !Number.isSafeInteger(message.ready.baselineCursor.resourcesRevision)
                || message.ready.baselineCursor.resourcesRevision < 0) { this.failed({ code: "event_protocol_error", message: "" }, false); return; }
            this.eventSequence = message.ready.baselineCursor.sequence;
            this.resourcesRevision = message.ready.baselineCursor.resourcesRevision;
            if (message.ready.resyncRequired) {
                this.accepted = message.ready.baselineCursor;
                this.operations.forEach((entry) => { entry.error = undefined; });
            }
            this.ready = true;
            this.readyResolve?.();
            this.retryDelay = 250;
            this.publish({ kind: this.displayed ? "resyncing" : "connecting" });
            this.invalidateAll();
            this.schedule();
            this.finishResync();
            return;
        }
        if (message.type !== "batch") { this.failed({ code: "event_protocol_error", message: "" }, false); return; }
        const batch = message.batch;
        if (batch.subscriptionId === this.subscriptionId && batch.instanceId === this.instanceId && batch.batchId === this.receivedBatch
            && batch.nextCursor.sequence === this.eventSequence && batch.nextCursor.resourcesRevision === this.resourcesRevision) {
            await (await opened).acknowledge(batch).catch(() => {});
            return;
        }
        if (batch.subscriptionId !== this.subscriptionId || batch.batchId !== this.receivedBatch + 1 || batch.events.length === 0 && batch.invalidatedScopes.length === 0
            || batch.instanceId !== this.instanceId || batch.instanceId !== batch.nextCursor.instanceId || batch.nextCursor.sequence < this.eventSequence) {
            this.failed({ code: "invalid_request", message: "" }, false); return;
        }
        if (!Number.isSafeInteger(batch.nextCursor.resourcesRevision) || batch.nextCursor.resourcesRevision < this.resourcesRevision || batch.invalidatedScopes.length > 0 && batch.nextCursor.resourcesRevision <= this.resourcesRevision
            || batch.invalidatedScopes.length === 0 && batch.nextCursor.resourcesRevision !== this.resourcesRevision
            || batch.invalidatedScopes.some((scope) => scope !== "extensions")) { this.failed({ code: "event_protocol_error", message: "" }, false); return; }
        let last = this.eventSequence;
        for (const event of batch.events) {
            if (!Number.isSafeInteger(event.sequence) || event.sequence <= last || !uuid(event.eventId) || !uuid(event.aggregateId)
                || !aggregateKinds.has(event.aggregateKind) || !Number.isSafeInteger(event.aggregateRevision) || event.aggregateRevision < 1
                || typeof event.kind !== "string" || event.kind.length === 0 || event.kind.length > 128) { this.failed({ code: "event_protocol_error", message: "" }, false); return; }
            last = event.sequence;
        }
        if (last !== batch.nextCursor.sequence) { this.failed({ code: "invalid_request", message: "" }, false); return; }
        for (const event of batch.events) this.changed(event);
        for (const scope of batch.invalidatedScopes) this.invalidate(scope);
        this.receivedBatch = batch.batchId;
        this.eventSequence = last;
        this.resourcesRevision = batch.nextCursor.resourcesRevision;
        try {
            const connection = await opened;
            await connection.acknowledge(batch);
            if (this.active && message.generation === this.generation) this.accepted = batch.nextCursor;
        } catch (caught: unknown) {
            if (this.active && message.generation === this.generation) {
                const error = normalizeRequestError(caught);
                this.failed(error, ["daemon_unavailable", "store_unavailable", "event_consumer_timeout", "event_subscription_replaced"].includes(error.code));
            }
        }
    }
    private failed(error: RpcError, retryable: boolean) {
        if (error.code === "event_consumer_timeout") this.accepted = undefined;
        this.ready = false;
        this.generation++;
        const current = this.connection;
        this.connection = undefined;
        void current?.close().catch(() => {});
        this.readyReject?.(error);
        this.readQueue.splice(0).forEach((read) => read.cancel());
        [...this.readCancellations.values()].forEach((cancel) => cancel());
        this.publish({ kind: retryable ? "disconnected" : "failed", error });
        clearTimeout(this.retryTimer);
        if (retryable && this.active) {
            this.retryTimer = setTimeout(() => void this.connect(), this.retryDelay);
            this.retryDelay = Math.min(30_000, this.retryDelay * 2);
        }
    }

    private changed(event: CoreEvent) {
        if (event.aggregateKind === "operation") {
            const id = event.aggregateId;
            const progress = event.kind === "operation.progress";
            const urgent = /created|completed|succeeded|failed|cancelled/.test(event.kind);
            if (urgent && this.urgentOperations.size < EVENT_LIMITS.dirtyKeys) this.urgentOperations.set(id, event.aggregateRevision);
            this.invalidate(`operation:${id}`, urgent);
            const observed = this.operations.get(id);
            if (observed && observed.subscribers.size > 0 && observed.release === undefined) {
                this.overflowSyncPending = true;
                this.publish({ kind: "resyncing" });
                this.invalidate("operations-resync");
            }
            if (!progress) {
                this.invalidate("operations", urgent);
                if (urgent) this.urgentCollections.set("operationsList", this.categoryVersions.get("operations") ?? 0);
                this.lookupResourceImpact(id);
            }
            this.pumpReads();
            return;
        }
        const scopes: Record<string, ChangeScope> = { project: "projects", repository: "repositories", template: "templates",
            "user-package": "user-packages", "unity-installation": "unity", "project-editor-preference": "projects", "project-unity-launch-config": "projects" };
        const scope = scopes[event.aggregateKind];
        if (scope !== undefined) this.invalidate(scope, /registered|removed|unregistered|deleted/.test(event.kind));
        if (event.kind === "activity.changed") this.invalidate("activity");
    }
    private lookupResourceImpact(id: string) {
        if (this.jobs.has(`operation:${id}`) || this.lookups.has(id)) return;
        const scopes = ["extensions", "backups", "templates"].filter((scope) => [...this.jobs.values()].some((job) => job.scopes.includes(scope)));
        if (!scopes.length) return;
        if (this.lookups.size >= EVENT_LIMITS.dirtyKeys) { scopes.forEach((scope) => this.invalidate(scope)); return; }
        this.lookups.add(id);
        let release: (() => void) | undefined;
        release = this.register(`impact:${id}`, [`operation:${id}`], async () => {
            try { await this.client.operationGet(id); }
            catch { scopes.forEach((scope) => this.invalidate(scope)); }
            finally { this.lookups.delete(id); release?.(); }
        }, true);
        if (!release) { this.lookups.delete(id); scopes.forEach((scope) => this.invalidate(scope)); }
    }
    invalidate(scope: string, urgent = false) {
        if (scope.startsWith("operation:")) {
            const reading = this.operationReads.get(scope.slice("operation:".length));
            if (reading) reading.version++;
        }
        const versionKey = scope.startsWith("operation:") ? "operation-progress" : scope;
        this.categoryVersions.set(versionKey, (this.categoryVersions.get(versionKey) ?? 0) + 1);
        let promoted: string | undefined;
        if (this.dirty.size < EVENT_LIMITS.dirtyKeys || this.dirty.has(scope)) this.dirty.add(scope);
        else {
            const category = scope.startsWith("operation:") ? "operations" : scope.split(":")[0] ?? "activity";
            for (const key of this.dirty) if (key.startsWith(category === "operations" ? "operation:" : category + ":")) this.dirty.delete(key);
            this.dirty.add(category);
            promoted = category;
        }
        for (const job of this.jobs.values()) {
            if (job.scopes.includes(scope) || promoted !== undefined && (job.scopes.includes(promoted) || promoted === "operations" && job.key.startsWith("operation:"))) this.mark(job, urgent);
        }
        this.schedule();
    }
    private invalidateAll() {
        for (const job of this.jobs.values()) this.mark(job, true);
    }
    retryRead(key: string) {
        const job = this.jobs.get(key);
        if (!job) return false;
        this.mark(job, true); this.schedule(); return true;
    }
    private mark(job: Job, urgent: boolean) {
        if (!job.dirty) job.first = Date.now();
        job.dirty = true; job.blocked = false; job.urgent ||= urgent; job.version++;
    }
    register(key: string, scopes: readonly string[], run: () => Promise<void>, initial = false) {
        const entity = key.startsWith("operation:");
        if (!this.jobs.has(key) && (this.jobs.size >= EVENT_LIMITS.cachedRows || entity && this.operationJobs >= EVENT_LIMITS.cachedRows - 64)) {
            if (entity) this.pendingOperationObservers = true;
            else { scopes.forEach((scope) => this.invalidate(scope, true)); this.publish({ kind: "resyncing", error: { code: "resource_busy", message: "" } }); }
            return undefined;
        }
        const job: Job = { key, scopes, run, dirty: false, blocked: false, urgent: false, version: 0, first: 0, last: -Infinity, running: false, retired: false };
        const previous = this.jobs.get(key);
        if (previous) previous.retired = true;
        if (!previous && entity) this.operationJobs++;
        this.jobs.set(key, job);
        if (initial || scopes.some((scope) => this.dirty.has(scope))) this.mark(job, true);
        this.schedule();
        return () => {
            job.retired = true;
            if (this.jobs.get(key) === job) {
                this.jobs.delete(key); if (entity) this.operationJobs--;
                this.registerWaitingOperations();
            }
        };
    }
    private visible = () => { if (document.visibilityState === "visible") { this.invalidateAll(); this.schedule(); } };
    private schedule() {
        clearTimeout(this.wakeTimer);
        if (!this.active || !this.ready || document.visibilityState !== "visible") return;
        const now = Date.now();
        const candidates = [...this.jobs.values()].filter((job) => job.dirty && !job.blocked && !job.running && !job.retired)
            .sort((a, b) => Number(b.urgent || now - b.first >= EVENT_LIMITS.maxWaitMs) - Number(a.urgent || now - a.first >= EVENT_LIMITS.maxWaitMs) || a.first - b.first);
        let next = Infinity;
        for (const job of candidates) {
            const due = job.urgent ? now : Math.min(job.first + EVENT_LIMITS.maxWaitMs, Math.max(job.first + EVENT_LIMITS.mergeMs, job.last + EVENT_LIMITS.intervalMs));
            if (due > now) { next = Math.min(next, due); continue; }
            if (this.running >= EVENT_LIMITS.concurrency || (!job.urgent && this.normalRunning >= EVENT_LIMITS.normalConcurrency)) continue;
            const version = job.version;
            const urgent = job.urgent;
            job.running = true; job.dirty = false; job.urgent = false; job.last = now;
            this.running++; if (!urgent) this.normalRunning++;
            void job.run().then(() => {
                if (job.version === version) job.dirty = false;
            }, () => {
                if (job.version === version) { job.dirty = true; job.blocked = true; }
            }).finally(() => {
                job.running = false; this.running--; if (!urgent) this.normalRunning--;
                this.schedule(); this.finishResync();
            });
        }
        if (next < Infinity) this.wakeTimer = setTimeout(() => this.schedule(), Math.max(1, next - now));
    }
    private finishResync() {
        if (![...this.jobs.values()].some((job) => job.scopes.includes("operations") && (job.dirty || job.running))) this.urgentCollections.delete("operationsList");
        for (const scope of this.dirty) {
            const matching = [...this.jobs.values()].filter((job) => job.scopes.includes(scope));
            if (matching.length && matching.every((job) => !job.dirty && !job.running)) this.dirty.delete(scope);
        }
        if (this.ready && !this.overflowSyncPending && ["resyncing", "connecting"].includes(this.state.kind) && ![...this.jobs.values()].some((job) => job.dirty || job.running)) this.publish({ kind: "ready" });
    }

    private async read(method: string, args: unknown[], produce: () => Promise<unknown>) {
        await this.readyPromise;
        if (!this.active || !this.ready) throw { code: "daemon_unavailable", message: "" };
        const createdGeneration = this.generation;
        // Settings verification after a write must never share a pre-write read.
        const key = `${createdGeneration}:${method}:${JSON.stringify(args)}${method === "settingsGet" ? ":" + ++this.readSequence : ""}`;
        const previous = this.reading.get(key);
        if (previous) return previous;
        if (this.reading.size >= EVENT_LIMITS.dirtyKeys || this.readQueue.length >= EVENT_LIMITS.dirtyKeys) throw { code: "resource_busy", message: "" };
        const operationId = method === "operationGet" ? String(args[0]) : undefined;
        const operationRead = operationId === undefined ? undefined : this.operationReads.get(operationId) ?? { version: 0, readers: 0 };
        if (operationRead && operationId) { operationRead.readers++; this.operationReads.set(operationId, operationRead); }
        let released = false;
        const releaseVersion = () => {
            if (released) return;
            released = true;
            if (operationRead && operationId && --operationRead.readers === 0 && this.operationReads.get(operationId) === operationRead) this.operationReads.delete(operationId);
        };
        const isUrgent = () => method === "operationGet" && this.urgentOperations.has(String(args[0])) || this.urgentCollections.has(method);
        const result = new Promise<unknown>((resolve, reject) => {
            let finished = false;
            let timer: ReturnType<typeof setTimeout> | undefined;
            let lastStart = -Infinity;
            const clean = () => {
                if (finished) return;
                finished = true; clearTimeout(timer); this.delayedReads.delete(key); this.readCancellations.delete(key);
                const queued = this.readQueue.findIndex((read) => read.key === key);
                if (queued !== -1) this.readQueue.splice(queued, 1);
                if (this.reading.get(key) === result) this.reading.delete(key);
                releaseVersion();
            };
            const cancel = () => { if (!finished) { clean(); reject({ code: "daemon_unavailable", message: "" }); } };
            this.readCancellations.set(key, cancel);
            const enqueue = (retry = false) => {
                if (finished) return;
                if (!this.active || !this.ready || createdGeneration !== this.generation) { cancel(); return; }
                const delay = retry && !isUrgent() ? Math.max(0, lastStart + EVENT_LIMITS.intervalMs - Date.now()) : 0;
                if (delay > 0) {
                    const wake = () => {
                        if (this.delayedReads.get(key) !== promote) return;
                        clearTimeout(timer); this.delayedReads.delete(key); enqueue(true); this.pumpReads();
                    };
                    const promote = () => { if (isUrgent()) wake(); };
                    timer = setTimeout(wake, delay);
                    this.delayedReads.set(key, promote);
                    return;
                }
                this.readQueue.push({ key, get urgent() { return isUrgent(); }, cancel, start: () => {
                if (finished || !this.active || !this.ready || createdGeneration !== this.generation) { cancel(); return; }
                const urgent = isUrgent();
                this.readsRunning++; if (!urgent) this.normalReads++;
                lastStart = Date.now();
                const generation = createdGeneration;
                const version = operationRead?.version ?? this.readVersion(method);
                let superseded = false;
                void produce().then((value) => {
                    if (finished || !this.active || generation !== this.generation) throw { code: "daemon_unavailable" };
                    // Keep one logical read per resource. A reopened consumer can
                    // wait for an older read, but cannot accept its stale value.
                    if (version !== (operationRead?.version ?? this.readVersion(method))) { superseded = true; return; }
                    if (method === "operationGet") {
                        if ((value as Operation).operationId !== String(args[0])) throw { code: "internal_error", message: "" };
                        this.seedOperation(value as Operation);
                    }
                    resolve(value);
                }).catch(reject).finally(() => {
                    this.readsRunning--; if (!urgent) this.normalReads--;
                    if (superseded && !finished && this.active && this.ready && generation === this.generation) enqueue(true);
                    else { if (!finished && superseded) cancel(); else clean(); }
                    this.pumpReads();
                });
                } });
            };
            enqueue();
        });
        this.reading.set(key, result);
        this.pumpReads();
        return result;
    }
    private readVersion(method: string) {
        const scopes = method === "settingsGet" ? [] : method === "systemStatus" ? ["diagnostics"] : method === "packageQueryProjectCandidates" ? ["projects", "repositories", "user-packages"]
            : method === "operationGet" ? ["operation-progress"] : method.startsWith("operations") ? ["operations"]
            : method.startsWith("activity") ? ["activity"] : method.startsWith("diagnostics") ? ["diagnostics"]
            : method.startsWith("project") ? ["projects"] : method.startsWith("repositor") ? ["repositories"]
            : method.startsWith("userPackage") ? ["user-packages"] : method.startsWith("unity") ? ["unity", "projects"]
            : method.startsWith("template") ? ["templates"] : method.startsWith("backup") ? ["backups"] : ["extensions"];
        return scopes.map((scope) => this.categoryVersions.get(scope) ?? 0).join(":");
    }
    private pumpReads() {
        if (!this.active || !this.ready) return;
        [...this.delayedReads.values()].forEach((promote) => promote());
        while (this.readsRunning < EVENT_LIMITS.concurrency) {
            let index = this.readQueue.findIndex((read) => read.urgent);
            if (index === -1) {
                if (this.normalReads >= EVENT_LIMITS.normalConcurrency) return;
                index = this.readQueue.length === 0 ? -1 : 0;
            }
            if (index === -1) return;
            this.readQueue.splice(index, 1)[0]!.start();
        }
    }

    /** A changed resource invalidates the in-flight snapshot before it can reach a setter. */
    async consistentRead<T>(scopes: readonly string[], load: () => Promise<T>): Promise<T> {
        while (true) {
            await this.readyPromise;
            if (!this.active) throw { code: "daemon_unavailable" };
            const generation = this.generation;
            const versions = scopes.map((scope) => this.categoryVersions.get(scope) ?? 0);
            const value = await load();
            if (generation === this.generation && scopes.every((scope, index) => (this.categoryVersions.get(scope) ?? 0) === versions[index])) return value;
            // This delay exists only because a real invalidation superseded this read.
            await new Promise<void>((resolve) => setTimeout(resolve, EVENT_LIMITS.intervalMs));
        }
    }

    private operation(id: string) {
        let entry = this.operations.get(id);
        if (!entry) { entry = { version: 0, subscribers: new Set() }; this.operations.set(id, entry); }
        return entry;
    }
    operationSnapshot = (id: string) => this.operation(id);
    seedOperation(value: Operation) {
        const urgentRevision = this.urgentOperations.get(value.operationId);
        if (urgentRevision !== undefined && value.revision >= urgentRevision) this.urgentOperations.delete(value.operationId);
        const entry = this.operation(value.operationId);
        if (entry.value !== undefined && (entry.value.revision > value.revision || entry.value.revision === value.revision && entry.error === undefined)) return;
        const stateChanged = entry.value?.state !== value.state && (entry.value !== undefined || urgentRevision !== undefined);
        entry.value = value; entry.error = undefined; entry.version++;
        entry.subscribers.forEach((listener) => listener());
        this.operationListeners.forEach((listener) => listener(value));
        while (this.operations.size > EVENT_LIMITS.cachedRows) {
            const removable = [...this.operations].find(([id, item]) => id !== value.operationId && item.subscribers.size === 0);
            if (!removable) break;
            this.operations.delete(removable[0]);
        }
        if (stateChanged && terminal(value)) {
            if (value.kind.startsWith("extensions.")) this.invalidate("extensions", terminal(value));
            if (value.kind.startsWith("backups.")) this.invalidate("backups", terminal(value));
            if (value.kind.startsWith("templates.")) this.invalidate("templates", terminal(value));
        }
        if (terminal(value)) {
            this.urgentOperations.delete(value.operationId);
        }
        if (entry.subscribers.size === 0) this.operations.delete(value.operationId);
    }
    observeOperation(id: string, listener: () => void, initial?: Operation) {
        const entry = this.operation(id);
        entry.subscribers.add(listener);
        if (initial) this.seedOperation(initial);
        if (!entry.release) this.registerOperation(id, entry, initial === undefined);
        return () => {
            entry.subscribers.delete(listener);
            if (entry.subscribers.size === 0) {
                entry.release?.(); entry.release = undefined;
                if (this.operations.get(id) === entry) this.operations.delete(id);
            }
        };
    }
    private registerOperation(id: string, entry: OperationEntry, initial: boolean) {
        entry.release = this.register(`operation:${id}`, [`operation:${id}`], async () => {
            const generation = this.generation;
            try { await this.client.operationGet(id); }
            catch (caught: unknown) {
                if (generation === this.generation) { entry.error = normalizeRequestError(caught); entry.version++; entry.subscribers.forEach((callback) => callback()); }
                throw caught;
            }
        }, initial);
    }
    private registerWaitingOperations() {
        if (!this.pendingOperationObservers || this.registeringObservers) return;
        this.registeringObservers = true;
        queueMicrotask(() => {
            this.registeringObservers = false;
            if (!this.active) return;
            this.pendingOperationObservers = false;
            for (const [id, entry] of this.operations) if (entry.subscribers.size && !entry.release) {
                if (this.operationJobs >= EVENT_LIMITS.cachedRows - 64) { this.pendingOperationObservers = true; break; }
                this.registerOperation(id, entry, true);
            }
        });
    }
    overflowVersion() { return this.categoryVersions.get("operations-resync") ?? 0; }
    completeOverflowSync(version: number) { if (this.overflowVersion() === version) { this.overflowSyncPending = false; this.finishResync(); } }
    subscribeOperations(listener: (value: Operation) => void) { this.operationListeners.add(listener); return () => { this.operationListeners.delete(listener); }; }
    consumeTerminal(id: string) {
        if (this.consumedTerminal.has(id)) return false;
        this.consumedTerminal.add(id);
        if (this.consumedTerminal.size > 1_024) this.consumedTerminal.delete(this.consumedTerminal.values().next().value!);
        return true;
    }
    metadata<T>(view: "projects" | "operations" | "activity", key: string, initial: T): T {
        const record = this.view(view);
        return Object.hasOwn(record.metadata, key) ? record.metadata[key] as T : initial;
    }
    setMetadata(view: "projects" | "operations" | "activity", key: string, value: unknown) { this.view(view).metadata[key] = value; }
    private view(key: string) {
        let record = this.views.get(key);
        if (!record) { record = { rows: 0, bytes: 0, visit: ++this.visit, metadata: {} }; this.views.set(key, record); }
        record.visit = ++this.visit;
        return record;
    }
    readView<T>(key: "projects" | "operations" | "activity"): T | undefined { return this.view(key).data as T | undefined; }
    writeView(key: "projects" | "operations" | "activity", data: unknown, rows: number) {
        const record = this.view(key);
        record.data = data; record.rows = rows; record.bytes = new TextEncoder().encode(JSON.stringify(data)).length;
        const totals = () => [...this.views.values()].reduce((sum, item) => ({ rows: sum.rows + item.rows, bytes: sum.bytes + item.bytes }), { rows: 0, bytes: 0 });
        while (totals().rows > EVENT_LIMITS.cachedRows || totals().bytes > EVENT_LIMITS.cachedBytes) {
            const oldest = [...this.views.values()].filter((item) => item.data !== undefined).sort((a, b) => a.visit - b.visit)[0];
            if (!oldest) break;
            oldest.data = undefined; oldest.rows = 0; oldest.bytes = 0;
        }
    }
}
