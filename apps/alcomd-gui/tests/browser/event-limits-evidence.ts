import { CoreEvents, EVENT_LIMITS } from "../../src/core-events";
import type { Operation } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";
import { browserCoreEvents } from "./core-events-client";

interface PendingRead { id: string; resolve(): void }
const authority = new Map<string, Operation>();
const reads: string[] = [];
const pending: PendingRead[] = [];
const pendingLists: Array<{ key: string; resolve(): void }> = [];
const listReads: string[] = [];
const listOutcomes: string[] = [];
let heldLists = false;
const outcomes: Array<{ id: string; state: string; revision?: number; code?: string }> = [];
const stopAtResponse = new Set<string>();
let holdNextClose = false;
let holdNextOpen = false;
let releaseClose: (() => void) | undefined;
let releaseOpen: (() => void) | undefined;
let stopFinished = false;
const closedGenerations: number[] = [];
let held = true;
let active = 0;
let activeNormal = 0;
let maxActive = 0;
let maxNormal = 0;
let categoryResult: Operation[] = [];
let categoryReads = 0;
let discardedIdReads = 0;

function id(index: number) { return `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`; }
function store(index: number, revision = 1, state = "running") {
    const operation: Operation = { operationId: id(index), kind: "state.check", state, revision,
        createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_000_000 + revision };
    authority.set(operation.operationId, operation);
    return operation;
}

const source = {
    async subscribeEvents(request: Parameters<GuiRpcClient["subscribeEvents"]>[0], receive: Parameters<GuiRpcClient["subscribeEvents"]>[1]) {
        const subscription = await browserCoreEvents.subscribe(request, receive);
        if (holdNextOpen) {
            holdNextOpen = false;
            await new Promise<void>((resolve) => { releaseOpen = resolve; });
        }
        return { acknowledge: subscription.acknowledge, close: async () => {
            if (holdNextClose) {
                holdNextClose = false;
                await new Promise<void>((resolve) => { releaseClose = resolve; });
            }
            closedGenerations.push(request.generation);
            await subscription.close();
        } };
    },
    async operationGet(operationId: string) {
        const value = authority.get(operationId);
        if (!value) throw { code: "operation_not_found", message: "" };
        // Take the same immutable response snapshot that a Core read would take.
        const snapshot = structuredClone(value);
        const normal = value.state !== "succeeded";
        reads.push(operationId); active++; if (normal) activeNormal++;
        maxActive = Math.max(maxActive, active); maxNormal = Math.max(maxNormal, activeNormal);
        try {
            if (held) await new Promise<void>((resolve) => pending.push({ id: operationId, resolve }));
            return snapshot;
        } finally {
            active--; if (normal) activeNormal--;
            if (stopAtResponse.delete(operationId)) queueMicrotask(() => queueMicrotask(() => { void coordinator.stop(); }));
        }
    },
    async operationsList(cursor?: Parameters<GuiRpcClient["operationsList"]>[0], activeOnly = false) {
        categoryReads++;
        const all = structuredClone([...authority.values()]);
        const operations = all.filter((item) => cursor === undefined || item.createdAtMs < cursor.createdAtMs || item.createdAtMs === cursor.createdAtMs && item.operationId < cursor.operationId)
            .filter((item) => !activeOnly || !["succeeded", "failed", "cancelled"].includes(item.state));
        const key = cursor?.operationId ?? "head";
        const normal = !all.some((item) => item.state === "succeeded");
        listReads.push(key); active++; if (normal) activeNormal++;
        maxActive = Math.max(maxActive, active); maxNormal = Math.max(maxNormal, activeNormal);
        try {
            if (heldLists) await new Promise<void>((resolve) => pendingLists.push({ key, resolve }));
            return { operations, activeCount: all.filter((item) => !["succeeded", "failed", "cancelled"].includes(item.state)).length };
        } finally { active--; if (normal) activeNormal--; }
    }
} as GuiRpcClient;
const coordinator = new CoreEvents(source);

const evidence = {
    coordinator,
    limits: EVENT_LIMITS,
    id,
    get reads() { return [...reads]; },
    get active() { return active; },
    get activeNormal() { return activeNormal; },
    get maxActive() { return maxActive; },
    get maxNormal() { return maxNormal; },
    get pending() { return pending.map((item) => item.id); },
    get pendingLists() { return pendingLists.map((item) => item.key); },
    get listReads() { return [...listReads]; },
    get listOutcomes() { return [...listOutcomes]; },
    get categoryReads() { return categoryReads; },
    get categoryResult() { return categoryResult; },
    get discardedIdReads() { return discardedIdReads; },
    get outcomes() { return [...outcomes]; },
    get closeWaiting() { return releaseClose !== undefined; },
    get openWaiting() { return releaseOpen !== undefined; },
    get stopFinished() { return stopFinished; },
    get closedGenerations() { return [...closedGenerations]; },
    restartDuringClose() {
        holdNextClose = true; holdNextOpen = true;
        void coordinator.stop().then(() => { stopFinished = true; });
        coordinator.start();
    },
    releaseClose() { const release = releaseClose; releaseClose = undefined; release?.(); },
    releaseOpen() { const release = releaseOpen; releaseOpen = undefined; release?.(); },
    seed(index: number) { store(index); },
    holdLists() { heldLists = true; },
    releasePendingLists() { pendingLists.splice(0).forEach((item) => item.resolve()); },
    releaseLists() { heldLists = false; pendingLists.splice(0).forEach((item) => item.resolve()); },
    startListRead(index: number) {
        void coordinator.client.operationsList({ createdAtMs: 1_700_000_000_000, operationId: id(index) })
            .then(() => listOutcomes.push(id(index)));
    },
    recover(index: number) {
        const operation = store(index, (authority.get(id(index))?.revision ?? 0) + 1, "recovering");
        browserCoreEvents.emit("operation", operation.operationId, "operation.state_changed", operation.revision);
    },
    read(index: number) { return coordinator.client.operationGet(id(index)); },
    startRead(index: number) { void coordinator.client.operationGet(id(index)).then((value) => {
        outcomes.push({ id: value.operationId, state: value.state, revision: value.revision });
    }, (error: { code: string }) => outcomes.push({ id: id(index), state: "rejected", code: error.code })); },
    stopAtResponse(index: number) { stopAtResponse.add(id(index)); },
    progress(index: number, count = 1) {
        for (let change = 0; change < count; change++) {
            const value = store(index, (authority.get(id(index))?.revision ?? 0) + 1);
            browserCoreEvents.emit("operation", value.operationId, "operation.progress", value.revision);
        }
    },
    releaseIfPending(index: number) {
        const position = pending.findIndex((item) => item.id === id(index));
        if (position !== -1) pending.splice(position, 1)[0]!.resolve();
    },
    release(index: number) {
        const position = pending.findIndex((item) => item.id === id(index));
        if (position === -1) throw new Error("Missing evidence read");
        pending.splice(position, 1)[0]!.resolve();
    },
    releaseAll() {
        held = false;
        pending.splice(0).forEach((item) => item.resolve());
    },
    complete(index: number) {
        const operation = store(index, (authority.get(id(index))?.revision ?? 0) + 1, "succeeded");
        browserCoreEvents.emit("operation", operation.operationId, "operation.succeeded", operation.revision);
    },
    observe(count: number) {
        held = false;
        for (let index = 1; index <= count; index++) coordinator.observeOperation(id(index), () => {}, store(index));
    },
    progressAll(count: number) {
        for (let index = 1; index <= count; index++) {
            const operation = store(index, 2);
            browserCoreEvents.emit("operation", operation.operationId, "operation.progress", operation.revision);
        }
    },
    registerOverflowRecovery() {
        // Register after delivery: the bounded ID set must already have promoted
        // itself to operations invalidation for this reader to become dirty.
        coordinator.register("limits:category", ["operations"], async () => {
            categoryResult = (await coordinator.client.operationsList()).operations;
        });
        coordinator.register("limits:discarded-id", [`operation:${id(1)}`], async () => { discardedIdReads++; });
    }
};

declare global { interface Window { eventLimits: typeof evidence } }
window.eventLimits = evidence;
coordinator.start();
window.addEventListener("pagehide", () => { void coordinator.stop(); }, { once: true });
