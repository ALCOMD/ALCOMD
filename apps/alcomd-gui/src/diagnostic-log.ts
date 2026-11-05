import type { DiagnosticLogHealth, DiagnosticLogHistoryParams, DiagnosticLogHistoryResult, DiagnosticLogItem, DiagnosticLogIssue, DiagnosticLogQuery, DiagnosticLogReadParams, DiagnosticLogReadResult, RpcError } from "@alcomd/sdk";
import { normalizeRequestError } from "./request-errors.ts";

export interface DiagnosticLogSource {
    read(params: DiagnosticLogReadParams): Promise<DiagnosticLogReadResult>;
    history(params: DiagnosticLogHistoryParams): Promise<DiagnosticLogHistoryResult>;
    cancel(): Promise<void>;
}
export interface DiagnosticLogSnapshot {
    readonly items: readonly DiagnosticLogItem[];
    readonly health?: DiagnosticLogHealth;
    readonly issues: readonly DiagnosticLogIssue[];
    readonly error?: RpcError;
    readonly loading: boolean;
    readonly hasMore: boolean;
    readonly scannedBytes: number;
    readonly limited: boolean;
}
const EMPTY: DiagnosticLogSnapshot = { items: [], issues: [], loading: false, hasMore: false, scannedBytes: 0, limited: false };
export function diagnosticLogKey(item: DiagnosticLogItem): string {
    return `${item.sourceId}:${item.sessionId}:${item.segmentId}:${item.offset}`;
}
export function logTimestamp(item: DiagnosticLogItem): number { return Date.parse(item.record.timestamp ?? "") || 0; }

/** Bounded disposable cache of real file records. It cannot manufacture a log. */
export class DiagnosticLogFeed {
    private snapshot: DiagnosticLogSnapshot = EMPTY;
    private readonly listeners = new Set<() => void>();
    private generation = 0;
    private running = false;
    private historyPending = false;
    private historyCursor?: string;
    private baseline?: string;
    private cursor?: string;
    private readonly source: DiagnosticLogSource;
    private readonly query: DiagnosticLogQuery;
    constructor(source: DiagnosticLogSource, query: DiagnosticLogQuery = {}) { this.source = source; this.query = query; }
    readonly getSnapshot = () => this.snapshot;
    readonly subscribe = (listener: () => void) => { this.listeners.add(listener); return () => { this.listeners.delete(listener); }; };
    private update(next: DiagnosticLogSnapshot) { this.snapshot = next; this.listeners.forEach((listener) => listener()); }
    private current(generation: number) { return this.running && generation === this.generation; }
    start() {
        if (this.running) return;
        this.running = true;
        const generation = ++this.generation;
        this.update({ ...this.snapshot, loading: true, error: undefined });
        void this.begin(generation);
    }
    async stop() {
        this.running = false; ++this.generation;
        this.historyPending = false;
        try { await this.source.cancel(); } catch { /* Cancellation does not produce another record. */ }
    }
    private async begin(generation: number) {
        try {
            const page = await this.source.read({ limit: 100, query: this.query });
            if (!this.current(generation)) return;
            this.baseline = page.nextCursor; this.cursor = page.nextCursor;
            this.accept(page.items, page.health, page.issues, page.scannedBytes);
            void this.loadHistory(generation);
            await this.follow(generation);
        } catch (error: unknown) { this.fail(error, generation); }
    }
    private async follow(generation: number) {
        while (this.current(generation)) {
            const page = await this.source.read({ cursor: this.cursor, limit: 200, follow: true, query: this.query });
            if (!this.current(generation)) return;
            this.accept(page.items, page.health, page.issues, page.scannedBytes);
            this.cursor = page.nextCursor;
        }
    }
    private fail(error: unknown, generation: number) {
        if (this.current(generation)) this.update({ ...this.snapshot, error: normalizeRequestError(error), loading: false });
    }
    private accept(incoming: DiagnosticLogItem[], health: DiagnosticLogHealth, issues: DiagnosticLogIssue[], scannedBytes: number) {
        const items = new Map(this.snapshot.items.map((item) => [diagnosticLogKey(item), item]));
        incoming.forEach((item) => items.set(diagnosticLogKey(item), item));
        const ordered = [...items.values()].sort((a, b) => logTimestamp(b) - logTimestamp(a) || diagnosticLogKey(a).localeCompare(diagnosticLogKey(b)));
        const kept: DiagnosticLogItem[] = []; let bytes = 0;
        const encoder = new TextEncoder();
        for (const item of ordered) {
            const size = encoder.encode(JSON.stringify(item)).length;
            if (kept.length >= 1000 || bytes + size > 4 * 1024 * 1024) break;
            bytes += size; kept.push(item);
        }
        const issueMap = new Map([...this.snapshot.issues, ...issues].map((issue) => [JSON.stringify(issue), issue]));
        const previousHealth = this.snapshot.health;
        const currentHealth = previousHealth?.sourceId === health.sourceId && previousHealth.sessionId === health.sessionId && previousHealth.revision > health.revision ? previousHealth : health;
        this.update({ ...this.snapshot, items: kept, health: currentHealth, issues: [...issueMap.values()].slice(-100), scannedBytes: this.snapshot.scannedBytes + scannedBytes,
            limited: this.snapshot.limited || kept.length < ordered.length, loading: this.historyPending });
    }
    private async loadHistory(generation: number) {
        if (!this.current(generation) || this.historyPending || this.baseline === undefined || this.snapshot.limited) return;
        this.historyPending = true; this.update({ ...this.snapshot, loading: true });
        try {
            const page = await this.source.history({ cursor: this.historyCursor, before: this.baseline, limit: 100, query: this.query });
            if (!this.current(generation)) return;
            this.historyCursor = page.nextCursor;
            this.accept(page.items, page.health, page.issues, page.scannedBytes);
            this.update({ ...this.snapshot, hasMore: page.hasMore && page.nextCursor !== undefined });
        } catch (error: unknown) { this.fail(error, generation); }
        finally { if (this.current(generation)) { this.historyPending = false; this.update({ ...this.snapshot, loading: false }); } }
    }
    readonly loadMore = () => { if (this.snapshot.hasMore) void this.loadHistory(this.generation); };
    readonly retry = async () => { await this.stop(); this.historyCursor = undefined; this.start(); };
}
