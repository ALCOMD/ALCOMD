import type { CoreEvent, EventsBatch, EventsCursor } from "@alcomd/sdk";
import type { GuiEventMessage, GuiEventPayload, GuiEventSubscription, GuiEventsRequest } from "../../src/event-types";
import type { GuiRpcClient } from "../../src/rpc";

interface Reader { request: GuiEventsRequest; receive(message: GuiEventMessage): void; subscriptionId: string; cursor: number; resourcesRevision: number; batchId: number; pending?: EventsBatch;
    timer?: ReturnType<typeof setTimeout>; ackPromise?: Promise<void>; heldAck?: { resolve(): void; reject(error: unknown): void } }

/** In-memory Core transport. State owners publish only after changing their read models. */
export class BrowserCoreEvents {
    private instance = crypto.randomUUID();
    private sequence = 0;
    private resourcesRevision = 0;
    private extensionsRevision = 0;
    private logRevision = 0;
    private readers = new Set<Reader>();
    private history: CoreEvent[] = [];
    private held = false;
    private ackHeld = false;
    private flushQueued = false;
    readonly requests: Array<{ method: string; params: unknown }> = [];
    readonly rpcRequests: Array<{ method: string; args: unknown[] }> = [];
    get connections() { return this.readers.size; }
    get pending() { return [...this.readers].filter((reader) => reader.pending !== undefined).length; }
    get instanceId() { return this.instance; }

    subscribe = async (request: GuiEventsRequest, receive: (message: GuiEventMessage) => void): Promise<GuiEventSubscription> => {
        this.requests.push({ method: "subscribe", params: structuredClone(request) });
        const resume = request.cursor?.instanceId === this.instance && request.cursor.sequence <= this.sequence && request.cursor.resourcesRevision === this.resourcesRevision;
        const cursor = resume ? request.cursor!.sequence : this.sequence;
        const reader: Reader = { request, receive, subscriptionId: crypto.randomUUID(), cursor, resourcesRevision: this.resourcesRevision, batchId: 0 };
        this.readers.add(reader);
        queueMicrotask(() => {
            if (!this.readers.has(reader)) return;
            this.deliver(reader, { type: "ready", ready: { subscriptionId: reader.subscriptionId, instanceId: this.instance,
                baselineCursor: this.cursor(cursor), resyncRequired: !resume } });
            this.flush(reader);
        });
        return {
            acknowledge: async (batch) => {
                this.requests.push({ method: "ack", params: { subscriptionId: batch.subscriptionId, batchId: batch.batchId } });
                if (!this.readers.has(reader) || batch.subscriptionId !== reader.subscriptionId || batch.batchId !== reader.pending?.batchId) return;
                if (this.ackHeld) {
                    reader.ackPromise ??= new Promise<void>((resolve, reject) => { reader.heldAck = { resolve, reject }; });
                    await reader.ackPromise;
                }
                if (!this.readers.has(reader) || reader.pending?.batchId !== batch.batchId) return;
                clearTimeout(reader.timer);
                reader.cursor = batch.nextCursor.sequence;
                reader.resourcesRevision = batch.nextCursor.resourcesRevision;
                reader.pending = undefined;
                reader.ackPromise = undefined; reader.heldAck = undefined;
                queueMicrotask(() => this.flush(reader));
            },
            close: async () => { this.closeReader(reader); }
        };
    };
    emit(aggregateKind: CoreEvent["aggregateKind"], aggregateId: string, kind: string, aggregateRevision = 1, payload: Record<string, unknown> = {}) {
        const event: CoreEvent = { eventId: crypto.randomUUID(), sequence: ++this.sequence, aggregateKind, aggregateId, kind,
            aggregateRevision, occurredAtMs: Date.now(), payload };
        this.history.push(event);
        this.requestFlush();
        return event;
    }
    hold(value: boolean) { this.held = value; if (!value) for (const reader of this.readers) this.flush(reader); }
    invalidateExtensions() { this.extensionsRevision = ++this.resourcesRevision; this.requestFlush(); }
    invalidateOperationLog() { this.logRevision = ++this.resourcesRevision; this.requestFlush(); }
    holdAcknowledgement(value: boolean) { this.ackHeld = value; if (!value) for (const reader of this.readers) { reader.heldAck?.resolve(); reader.heldAck = undefined; } }
    duplicate() {
        for (const reader of this.readers) {
            const batch = reader.pending;
            if (batch) this.deliver(reader, { type: "batch", batch: structuredClone(batch) });
        }
    }
    disconnect(retryable = true, code = "daemon_unavailable") {
        for (const reader of [...this.readers]) {
            this.closeReader(reader);
            this.deliver(reader, { type: "error", error: { code, message: "" }, retryable });
        }
    }
    close(retryable = false, code = "permission_denied") {
        for (const reader of [...this.readers]) {
            this.closeReader(reader, code);
            this.deliver(reader, { type: "closed", closed: { subscriptionId: reader.subscriptionId, instanceId: this.instance,
                error: { code, message: "" }, retryable } });
        }
    }
    restart() { this.instance = crypto.randomUUID(); this.sequence = 0; this.resourcesRevision = 0; this.extensionsRevision = 0; this.logRevision = 0; this.history = []; this.disconnect(); }
    private cursor(sequence: number): EventsCursor { return { instanceId: this.instance, sequence, resourcesRevision: this.resourcesRevision }; }
    private requestFlush() {
        if (this.flushQueued) return;
        this.flushQueued = true;
        queueMicrotask(() => { this.flushQueued = false; for (const reader of this.readers) this.flush(reader); });
    }
    private closeReader(reader: Reader, code = "daemon_unavailable") {
        this.readers.delete(reader); clearTimeout(reader.timer);
        reader.heldAck?.reject({ code, message: "" }); reader.heldAck = undefined; reader.ackPromise = undefined;
    }
    private deliver(reader: Reader, message: GuiEventPayload) {
        reader.receive({ ...message, consumerId: reader.request.consumerId, generation: reader.request.generation } as GuiEventMessage);
    }
    private flush(reader: Reader) {
        if (this.held || reader.pending || !this.readers.has(reader)) return;
        const events = this.history.filter((event) => event.sequence > reader.cursor).slice(0, 100);
        const invalidatedScopes: EventsBatch["invalidatedScopes"] = [];
        if (this.extensionsRevision > reader.resourcesRevision) invalidatedScopes.push("extensions");
        if (this.logRevision > reader.resourcesRevision) invalidatedScopes.push("operation-log");
        if (!events.length && !invalidatedScopes.length) return;
        const batch: EventsBatch = { subscriptionId: reader.subscriptionId, instanceId: this.instance, batchId: ++reader.batchId,
            events: structuredClone(events), invalidatedScopes, nextCursor: this.cursor(events.at(-1)?.sequence ?? reader.cursor) };
        reader.pending = batch;
        reader.timer = setTimeout(() => {
            if (!this.readers.has(reader) || reader.pending !== batch) return;
            this.closeReader(reader, "event_consumer_timeout");
            this.deliver(reader, { type: "error", error: { code: "event_consumer_timeout", message: "" }, retryable: true });
        }, 10_000);
        this.deliver(reader, { type: "batch", batch });
    }
}

export const browserCoreEvents = new BrowserCoreEvents();
declare global { interface Window { coreEvents: BrowserCoreEvents } }
if (typeof window !== "undefined") window.coreEvents = browserCoreEvents;

export function withBrowserCoreEvents(client: GuiRpcClient): GuiRpcClient {
    const methods = new Map<PropertyKey, unknown>();
    const changes: Record<string, [CoreEvent["aggregateKind"], string]> = {
        projectRegister: ["project", "project.registered"], projectRefresh: ["project", "project.refreshed"], projectSetFavorite: ["project", "project.favorite_changed"], projectUnregister: ["project", "project.unregistered"],
        repositoryRegister: ["repository", "repository.registered"], repositoryRefresh: ["repository", "repository.refreshed"], repositoryUnregister: ["repository", "repository.unregistered"],
        repositorySetDisplayName: ["repository", "repository.display-name-changed"],
        userPackageEnroll: ["user-package", "user_package.enrolled"], userPackageRefresh: ["user-package", "user_package.refreshed"], userPackageRemove: ["user-package", "user_package.removed"],
        unityInstallationRegister: ["unity-installation", "unity_installation.registered"], unityInstallationRemove: ["unity-installation", "unity_installation.removed"], unityInstallationsRefresh: ["unity-installation", "unity_installations.refreshed"],
        unityProjectLaunchConfigSet: ["project-unity-launch-config", "unity.project-launch-config.set"], unityProjectLaunchConfigClear: ["project-unity-launch-config", "unity.project-launch-config.cleared"]
    };
    const extensionChanges = new Set(["extensionEnable", "extensionDisable", "extensionSetGrant", "extensionRevokeGrant"]);
    return new Proxy(client, { get(target, key, receiver) {
        if (key === "subscribeEvents") return browserCoreEvents.subscribe;
        if (key === "subscribeFileDrag") return target.subscribeFileDrag?.bind(target);
        const value: unknown = Reflect.get(target, key, receiver);
        if (typeof value !== "function") return value;
        if (!methods.has(key)) methods.set(key, async (...args: unknown[]) => {
            browserCoreEvents.rpcRequests.push({ method: String(key), args: structuredClone(args) });
            const result: unknown = await Reflect.apply(value, target, args);
            if (extensionChanges.has(String(key))) { browserCoreEvents.invalidateExtensions(); return result; }
            if (!changes[String(key)]) return result;
            const [aggregate, kind] = changes[String(key)]!;
            const record = result !== null && typeof result === "object" ? result as Record<string, unknown> : {};
            const nested = Object.values(record).find((item) => item !== null && typeof item === "object" && ["projectId", "repositoryId", "templateId", "userPackageId", "installationId"].some((id) => id in item)) as Record<string, unknown> | undefined;
            const entity = nested ?? record;
            if (key === "unityInstallationsRefresh" && Array.isArray(record.installations)) {
                for (const installation of record.installations as Array<{ installationId: string; revision: number }>) browserCoreEvents.emit("unity-installation", installation.installationId, kind!, installation.revision);
                return result;
            }
            const id = ["projectId", "repositoryId", "templateId", "userPackageId", "installationId"].map((key) => entity[key]).find((value) => typeof value === "string") ?? args[0] ?? "00000000-0000-4000-8000-000000000101";
            browserCoreEvents.emit(aggregate!, String(id), kind!, typeof entity.revision === "number" ? entity.revision : 1);
            return result;
        });
        return methods.get(key);
    } });
}
