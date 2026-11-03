import type { ActivityCursor, ActivityItem, DiagnosticCursor, DiagnosticItem, Operation } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";

export const H5_INSTALL_ID = "00000000-0000-4000-8000-000000000701";
export const H5_CHECK_ID = "00000000-0000-4000-8000-000000000702";
export const H5_UNINSTALL_ID = "00000000-0000-4000-8000-000000000703";
export const H5_FAILED_ID = "00000000-0000-4000-8000-000000000704";
export const H5_DIAGNOSTIC_ID = "h5-diagnostic-".padEnd(255, "x");
export const H5_CORE_VERSION = "9.8.7-h5-core";
export const H5_SECRET = "h5-secret-response-value";
export const H5_ACTIVITY_CURSOR: ActivityCursor = { occurredAtMs: 1_700_000_001_000, sourceRank: 0, stableId: H5_CHECK_ID };
export const H5_DIAGNOSTIC_CURSOR: DiagnosticCursor = { occurredAtMs: 1_700_000_002_500, operationId: H5_FAILED_ID };

interface UtilityRequest { method: string; params: Record<string, unknown> }
interface PendingUtilityRequest { method: string; resolve(): void; reject(code: string): void }
interface H5UtilitiesFixture {
    requests: UtilityRequest[];
    failNext(method: string, code: string): void;
    deny(method: string, code?: string): void;
    hold(method: string, value: boolean): void;
    pending(method: string): number;
    release(method: string, code?: string): number;
    complete(operationId: string, state: "succeeded" | "failed" | "cancelled", errorCode?: string): void;
    recordActivity(item: ActivityItem): void;
}

declare global { interface Window { h5Utilities: H5UtilitiesFixture } }

/** Opt-in public read-model fixture. It never connects to a native Core or reads its data. */
export function withH5UtilitiesClient(base: GuiRpcClient, query: URLSearchParams): GuiRpcClient {
    const tasks = new Map<string, Operation>([
        [H5_INSTALL_ID, operation(H5_INSTALL_ID, "extensions.install", "running", 2_000)],
        [H5_CHECK_ID, operation(H5_CHECK_ID, "state.check", "succeeded", 1_000)],
        [H5_UNINSTALL_ID, { ...operation(H5_UNINSTALL_ID, "extensions.uninstall", "failed", 500), errorCode: "extension_recovery_required" }],
        [H5_FAILED_ID, { ...operation(H5_FAILED_ID, "extensions.install", "failed", 2_500), errorCode: "extension_signature_invalid", diagnosticId: H5_DIAGNOSTIC_ID }]
    ]);
    const held = new Set<string>();
    const denied = new Map<string, string>();
    const failures = new Map<string, string[]>();
    const pending: PendingUtilityRequest[] = [];
    const activityEntries: ActivityItem[] = [];
    if (query.has("h5-operations-slow")) held.add("operations.list");
    if (query.has("h5-status-offline")) denied.set("system.status", "daemon_unavailable");
    const fixture: H5UtilitiesFixture = {
        requests: [],
        recordActivity(item) { activityEntries.push(structuredClone(item)); },
        failNext(method, code) { failures.set(method, [...(failures.get(method) ?? []), code]); },
        deny(method, code) { if (code === undefined) denied.delete(method); else denied.set(method, code); },
        hold(method, value) { if (value) held.add(method); else held.delete(method); },
        pending(method) { return pending.filter((request) => request.method === method).length; },
        release(method, code) {
            const selected = pending.filter((request) => request.method === method);
            for (const request of selected) {
                pending.splice(pending.indexOf(request), 1);
                if (code === undefined) request.resolve(); else request.reject(code);
            }
            return selected.length;
        },
        complete(operationId, state, errorCode) {
            const previous = tasks.get(operationId);
            if (previous === undefined) throw new Error("Unknown H5 fixture operation");
            tasks.set(operationId, { ...previous, state, revision: previous.revision + 1,
                updatedAtMs: previous.updatedAtMs + 1_000, completedAtMs: previous.updatedAtMs + 1_000,
                ...(errorCode === undefined ? {} : { errorCode }) });
        }
    };
    window.h5Utilities = fixture;

    function operation(operationId: string, kind: string, state: string, offset: number): Operation {
        return { operationId, kind, state, revision: 1, createdAtMs: 1_700_000_000_000,
            updatedAtMs: 1_700_000_000_000 + offset,
            ...(state === "running" ? { progress: { phase: "source_verified" } } : { completedAtMs: 1_700_000_000_000 + offset }) };
    }
    function error(code: string) { return { code, message: H5_SECRET, diagnosticId: "h5-request-diagnostic" }; }
    async function response<T>(method: string, params: Record<string, unknown>, produce: () => T | Promise<T>): Promise<T> {
        fixture.requests.push({ method, params: structuredClone(params) });
        const queue = failures.get(method);
        const code = denied.get(method) ?? queue?.shift();
        if (code !== undefined) throw error(code);
        const value = structuredClone(await produce());
        if (!held.has(method)) return value;
        return new Promise((resolve, reject) => pending.push({ method, resolve: () => resolve(value), reject: (failure) => reject(error(failure)) }));
    }
    function activity(operationId: string): ActivityItem {
        const task = tasks.get(operationId)!;
        return { occurredAtMs: task.updatedAtMs, type: "operation", summaryCode: `operation.${task.kind}.${task.state}`,
            operationId, state: task.state };
    }
    function diagnostic(operationId: string): DiagnosticItem {
        const task = tasks.get(operationId)!;
        return { occurredAtMs: task.updatedAtMs, severity: "error", subsystem: "extensions", code: task.errorCode!,
            diagnosticId: task.diagnosticId ?? "h5-uninstall-diagnostic", operationId,
            summary: "The operation failed. Use the diagnostic ID when requesting support." };
    }
    const methods: Partial<GuiRpcClient> = {
        systemStatus() {
            return response("system.status", {}, async () => {
                const status = await base.systemStatus();
                return { ...status, daemonVersion: query.get("h5-core-version") ?? H5_CORE_VERSION,
                    capabilities: status.capabilities.filter((capability) => !query.has("h5-no-operations") || capability !== "operations.v1") };
            });
        },
        settingsGet() {
            return response<Awaited<ReturnType<GuiRpcClient["settingsGet"]>>>("settings.get", {}, async () => {
                const settings = await base.settingsGet();
                const locale = query.get("h5-locale");
                return locale === "zh-CN" || locale === "ja-JP" ? { ...settings, settings: { ...settings.settings, locale } } : settings;
            });
        },
        activityList(cursor) {
            if (query.has("h5-activity-live")) return response("activity.list", cursor === undefined ? {} : { cursor }, () => {
                const position = (item: ActivityItem): ActivityCursor => ({ occurredAtMs: item.occurredAtMs, sourceRank: item.type === "event" ? 1 : 0,
                    stableId: item.type === "event" ? String(item.eventSequence).padStart(20, "0") : item.operationId! });
                const compare = (left: ActivityCursor, right: ActivityCursor) => left.occurredAtMs - right.occurredAtMs
                    || left.sourceRank - right.sourceRank || left.stableId.localeCompare(right.stableId);
                const all = [activity(H5_INSTALL_ID), activity(H5_CHECK_ID), activity(H5_UNINSTALL_ID), ...activityEntries]
                    .filter((item) => cursor === undefined || compare(position(item), cursor) < 0)
                    .sort((left, right) => compare(position(right), position(left)));
                const items = all.slice(0, 2);
                return { items, ...(all.length <= 2 ? {} : { nextCursor: position(items.at(-1)!) }) };
            });
            return response("activity.list", cursor === undefined ? {} : { cursor }, () => cursor === undefined
                ? { items: [activity(H5_INSTALL_ID), activity(H5_CHECK_ID)], nextCursor: H5_ACTIVITY_CURSOR }
                : { items: [activity(H5_UNINSTALL_ID)] });
        },
        diagnosticsList(cursor) {
            return response("diagnostics.list", cursor === undefined ? {} : { cursor }, () => cursor === undefined
                ? { items: [diagnostic(H5_FAILED_ID)], nextCursor: H5_DIAGNOSTIC_CURSOR }
                : { items: [diagnostic(H5_UNINSTALL_ID)] });
        },
        operationsList(cursor) {
            return response("operations.list", cursor === undefined ? {} : { cursor }, () => ({ operations: [...tasks.values()] }));
        },
        operationGet(operationId) {
            return response("operations.get", { operationId }, () => {
                const task = tasks.get(operationId);
                if (task === undefined) throw error("operation_not_found");
                return task;
            });
        },
        stateCheck() {
            return response("state.check", {}, () => {
                tasks.set(H5_CHECK_ID, operation(H5_CHECK_ID, "state.check", "running", 3_000));
                return { operationId: H5_CHECK_ID, replayed: false };
            });
        }
    };
    return new Proxy(base, {
        get(target, property, receiver) {
            if (property in methods) return methods[property as keyof GuiRpcClient];
            const value: unknown = Reflect.get(target, property, receiver);
            return typeof value === "function" ? value.bind(target) : value;
        }
    });
}
