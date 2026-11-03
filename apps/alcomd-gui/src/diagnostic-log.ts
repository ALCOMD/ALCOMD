import type { DiagnosticLogCursor, DiagnosticLogItem, DiagnosticLogReadParams, DiagnosticLogReadResult, RpcError } from "@alcomd/sdk";
import { normalizeRequestError, requestErrorMessage } from "./request-errors.ts";

export interface DiagnosticLogSource {
    diagnosticLogRead(params: DiagnosticLogReadParams): Promise<DiagnosticLogReadResult>;
    diagnosticLogCancel(): Promise<void>;
}

export interface DiagnosticLogSnapshot {
    readonly items: readonly DiagnosticLogItem[];
    readonly notifications: readonly DiagnosticLogItem[];
    readonly gap: boolean;
    readonly error?: RpcError;
}

const EMPTY: DiagnosticLogSnapshot = { items: [], notifications: [], gap: false };
const MAX_RETAINED = 1024;

export function diagnosticLogKey(item: Pick<DiagnosticLogItem, "instanceId" | "sequence">): string {
    return `${item.instanceId}:${item.sequence}`;
}

export function diagnosticLogMessage(code: string): string {
    switch (code) {
        case "diagnostic.trace": return "Trace information recorded.";
        case "diagnostic.debug": return "Debug information recorded.";
        case "diagnostic.info": return "Information recorded.";
        case "diagnostic.warning": return "A warning was reported in the background.";
        case "diagnostic.error": return "An error occurred in the background.";
        default: return requestErrorMessage(code);
    }
}

function retryWait(signal: AbortSignal): Promise<void> {
    return new Promise((resolve) => {
        if (signal.aborted) { resolve(); return; }
        const finish = () => { clearTimeout(timer); signal.removeEventListener("abort", finish); resolve(); };
        const timer = setTimeout(finish, 1000);
        signal.addEventListener("abort", finish, { once: true });
    });
}

/** A single application-owned reader; its cache is a disposable view of Core's log. */
export class DiagnosticLogFeed {
    private readonly source: DiagnosticLogSource;
    private readonly wait: (signal: AbortSignal) => Promise<void>;
    private readonly listeners = new Set<() => void>();
    private snapshot: DiagnosticLogSnapshot = EMPTY;
    private cursor?: DiagnosticLogCursor;
    private generation = 0;
    private running = false;
    private retry?: AbortController;

    constructor(source: DiagnosticLogSource, wait = retryWait) {
        this.source = source;
        this.wait = wait;
    }

    readonly getSnapshot = (): DiagnosticLogSnapshot => this.snapshot;
    readonly subscribe = (listener: () => void): (() => void) => {
        this.listeners.add(listener);
        return () => { this.listeners.delete(listener); };
    };

    start(): void {
        if (this.running) return;
        this.running = true;
        void this.read(++this.generation);
    }

    async stop(): Promise<void> {
        this.running = false;
        this.generation += 1;
        this.retry?.abort();
        try { await this.source.diagnosticLogCancel(); } catch { /* Cancellation must not create another log notification. */ }
    }

    readonly retryNow = (): void => { this.retry?.abort(); };

    readonly dismiss = (key: string): void => {
        const notifications = this.snapshot.notifications.filter((item) => diagnosticLogKey(item) !== key);
        if (notifications.length !== this.snapshot.notifications.length) this.update({ ...this.snapshot, notifications });
    };

    private update(snapshot: DiagnosticLogSnapshot): void {
        this.snapshot = snapshot;
        this.listeners.forEach((listener) => listener());
    }

    private current(generation: number): boolean { return this.running && this.generation === generation; }

    private async read(generation: number): Promise<void> {
        while (this.current(generation)) {
            try {
                const result = await this.source.diagnosticLogRead({ cursor: this.cursor, limit: 200, waitMs: this.cursor === undefined ? 0 : 20000 });
                if (!this.current(generation)) return;
                this.accept(result);
            } catch (caught: unknown) {
                if (!this.current(generation)) return;
                this.update({ ...this.snapshot, error: normalizeRequestError(caught) });
                this.retry = new AbortController();
                await this.wait(this.retry.signal);
                this.retry = undefined;
            }
        }
    }

    private accept(result: DiagnosticLogReadResult): void {
        const previous = this.cursor;
        const newInstance = previous === undefined || result.reset || previous.instanceId !== result.nextCursor.instanceId;
        const baseline = newInstance || result.gap;
        if (!baseline && result.nextCursor.sequence < previous.sequence) return;
        // A repeated page must not republish a record, including after dismissal or reconnect.
        const seen = new Set<number>();
        const fresh = result.items.filter((item) => {
            if (item.instanceId !== result.nextCursor.instanceId || seen.has(item.sequence)
                || item.sequence > result.nextCursor.sequence || !baseline && item.sequence <= previous.sequence) return false;
            seen.add(item.sequence);
            return true;
        });
        const retained = baseline ? [] : this.snapshot.items;
        const items = [...retained, ...fresh].slice(-MAX_RETAINED);
        const notifications = newInstance ? [] : result.gap ? this.snapshot.notifications
            : [...this.snapshot.notifications, ...fresh.filter((item) => item.notify && (item.level === "error" || item.level === "warning"))].slice(-MAX_RETAINED);
        this.cursor = result.nextCursor;
        if (baseline || fresh.length !== 0 || this.snapshot.error !== undefined || result.gap && !this.snapshot.gap) {
            this.update({ items, notifications, gap: baseline ? result.gap : this.snapshot.gap || result.gap });
        }
    }
}
