import type { DiagnosticLogHealth, DiagnosticLogItem, DiagnosticLogQuery, DiagnosticLogReadParams, DiagnosticLogReadResult, DiagnosticLogHistoryParams, DiagnosticLogHistoryResult, DiagnosticLogLevel, FrontendException } from "@alcomd/sdk";
export const DIAGNOSTIC_LOG_FIXTURE_IDS = { coreA: "00000000-0000-4000-8000-000000009100", coreB: "00000000-0000-4000-8000-000000009101", historic: "00000000-0000-4000-8000-000000009200" } as const;
export interface TestLogInput { source?: "core" | "gui"; message?: string; level?: DiagnosticLogLevel; target?: string; diagnosticId?: string; path?: string; frontendError?: FrontendException }
declare global { interface Window {
    diagnosticLogRequests: DiagnosticLogReadParams[];
    diagnosticLogCancels: number;
    diagnosticLogHistoryRequests: DiagnosticLogHistoryParams[];
    localLogRequests: DiagnosticLogReadParams[];
    localLogHistoryRequests: DiagnosticLogHistoryParams[];
    localLogCancels: number;
} }
function containsText(value: unknown, needle: string): boolean {
    if (value !== null && typeof value === "object") return Object.entries(value).some(([key, nested]) => key.toLowerCase().includes(needle) || containsText(nested, needle));
    return value !== null && value !== undefined && String(value).toLowerCase().includes(needle);
}
/** Controlled authority: change stored records/health first, then wake consumers. */
export class BrowserDiagnosticLog {
    private failed = false;
    private held = false;
    private historyHeld = false;
    private historyFailure?: string;
    private historyPending: Array<() => void> = [];
    private readonly items: DiagnosticLogItem[] = [];
    private pending?: { params: DiagnosticLogReadParams; resolve(value: DiagnosticLogReadResult): void; reject(error: unknown): void };
    private healthState: DiagnosticLogHealth;
    constructor(private readonly enabled: boolean, initialFailure = false, savedMode = false, historyFailure = false, private readonly source: "core" | "gui" = "core") {
        this.failed = initialFailure; this.historyFailure = historyFailure ? "log_history_unavailable" : undefined;
        this.healthState = { sourceId: source, sessionId: DIAGNOSTIC_LOG_FIXTURE_IDS.coreA, revision: 0, state: "ready", queueDropped: 0, oversizeRejected: 0, writeFailed: 0, rotationFailed: 0, flushFailed: 0, hasGap: false };
        if (source === "core") { window.diagnosticLogRequests = []; window.diagnosticLogHistoryRequests = []; window.diagnosticLogCancels = 0; }
        else { window.localLogRequests = []; window.localLogHistoryRequests = []; window.localLogCancels = 0; }
        if (enabled && source === "core") this.append({ message: "Core logging initialized.", level: "INFO" }, false);
        if (savedMode && source === "core") for (let n = 1; n <= 3; n++) {
            const item = this.append({ message: `Previous session entry ${n}`, level: "ERROR" }, false);
            item.sessionId = "00000000-0000-4000-8000-000000009099";
        }
        window.addEventListener("test-diagnostic-log", ((event: CustomEvent<TestLogInput>) => { if ((event.detail.source ?? "core") === source) this.append(event.detail); }) as EventListener);
        window.addEventListener("test-log-health", ((event: CustomEvent<Partial<DiagnosticLogHealth> & { source?: string }>) => {
            if ((event.detail.source ?? "core") !== source) return;
            this.healthState = { ...this.healthState, ...event.detail, revision: this.healthState.revision + 1 }; this.flush();
        }) as EventListener);
        window.addEventListener("test-log-hold", ((event: CustomEvent<{ source?: string; held: boolean }>) => { if ((event.detail.source ?? "core") === source) { this.held = event.detail.held; this.flush(); } }) as EventListener);
        window.addEventListener("test-log-history-hold", ((event: CustomEvent<boolean>) => { this.historyHeld = event.detail; }) as EventListener);
        window.addEventListener("test-log-history-release", () => { this.historyHeld = false; this.historyPending.splice(0).forEach((resolve) => resolve()); });
        window.addEventListener("test-log-history-error", ((event: CustomEvent<string>) => { if (source === "core") this.historyFailure = event.detail; }) as EventListener);
        window.addEventListener("test-diagnostic-log-failure", () => { if (source === "core") { this.failed = true; this.pending?.reject({ code: "daemon_unavailable" }); this.pending = undefined; } });
        window.addEventListener("test-diagnostic-log-reset", () => { this.failed = false; this.healthState = { ...this.healthState, sessionId: DIAGNOSTIC_LOG_FIXTURE_IDS.coreB, revision: this.healthState.revision + 1 }; this.flush(); });
    }
    append(input: TestLogInput, notify = true): DiagnosticLogItem {
        const item: DiagnosticLogItem = { sourceId: this.source, sessionId: this.healthState.sessionId, segmentId: "fixture-segment", offset: this.items.length,
            record: { timestamp: new Date(1_700_000_000_000 + this.items.length * 1000).toISOString(), level: input.level ?? "ERROR", target: input.target ?? "alcomd::fixture",
                fields: { message: input.message ?? "A real fixture failure.", ...(input.path ? { path: input.path } : {}), ...(input.diagnosticId ? { diagnostic_id: input.diagnosticId } : {}),
                    ...(input.frontendError ? { name: input.frontendError.name, error: input.frontendError.message, stack: input.frontendError.stack ?? "", location: input.frontendError.location ?? "" } : {}) } } };
        this.items.push(item); if (notify) this.flush(); return item;
    }
    health(): Promise<DiagnosticLogHealth> { return Promise.resolve(structuredClone(this.healthState)); }
    private matches(item: DiagnosticLogItem, query: DiagnosticLogQuery = {}) {
        return (!query.levels?.length || query.levels.includes(item.record.level!)) && (!query.target || item.record.target?.includes(query.target))
            && (!query.search || containsText(item.record, query.search.toLowerCase()))
            && (!query.diagnosticId || item.record.fields?.diagnostic_id === query.diagnosticId);
    }
    read(params: DiagnosticLogReadParams): Promise<DiagnosticLogReadResult> {
        (this.source === "core" ? window.diagnosticLogRequests : window.localLogRequests).push(structuredClone(params));
        if (!this.enabled) return Promise.reject({ code: "method_not_found" });
        if (this.failed) return Promise.reject({ code: "daemon_unavailable" });
        const result = this.result(params);
        if (params.cursor === undefined || !params.follow || !this.held && this.changed(params)) return Promise.resolve(result);
        return new Promise((resolve, reject) => { this.pending = { params, resolve, reject }; });
    }
    private cursor(offset = this.items.length) { return JSON.stringify({ offset, health: this.healthState.revision }); }
    private changed(params: DiagnosticLogReadParams) { const p = JSON.parse(params.cursor!) as { offset: number; health: number }; return p.offset < this.items.length || p.health !== this.healthState.revision; }
    private result(params: DiagnosticLogReadParams): DiagnosticLogReadResult {
        const from = params.cursor === undefined ? this.items.length : (JSON.parse(params.cursor) as { offset: number }).offset;
        const scanned = this.items.slice(from, from + (params.limit ?? 100));
        return { items: structuredClone(scanned.filter((item) => this.matches(item, params.query))), nextCursor: this.cursor(from + scanned.length), baseline: params.cursor === undefined,
            health: structuredClone(this.healthState), issues: [], hasMore: from + scanned.length < this.items.length, scannedBytes: scanned.length * 100 };
    }
    async history(params: DiagnosticLogHistoryParams): Promise<DiagnosticLogHistoryResult> {
        (this.source === "core" ? window.diagnosticLogHistoryRequests : window.localLogHistoryRequests).push(structuredClone(params));
        if (!this.enabled) throw { code: "method_not_found" };
        if (this.failed) throw { code: "daemon_unavailable" };
        if (this.historyFailure) { const code = this.historyFailure; this.historyFailure = undefined; throw { code }; }
        if (this.historyHeld) await new Promise<void>((resolve) => this.historyPending.push(resolve));
        const before = params.cursor ? Number(params.cursor) : params.before ? (JSON.parse(params.before) as { offset: number }).offset : this.items.length;
        const start = Math.max(0, before - Math.min(2, params.limit ?? 100));
        const scanned = this.items.slice(start, before).reverse();
        return { items: structuredClone(scanned.filter((item) => this.matches(item, params.query))), ...(start > 0 ? { nextCursor: String(start) } : {}), health: structuredClone(this.healthState), issues: [], hasMore: start > 0, scannedBytes: scanned.length * 100 };
    }
    async cancel() { if (this.source === "core") window.diagnosticLogCancels++; else window.localLogCancels++; this.pending?.reject({ code: "daemon_unavailable" }); this.pending = undefined; }
    private flush() { const pending = this.pending; if (!pending || this.held || !this.changed(pending.params)) return; this.pending = undefined; pending.resolve(this.result(pending.params)); }
}
