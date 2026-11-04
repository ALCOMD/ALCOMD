import type { DiagnosticLogCursor, DiagnosticLogItem, DiagnosticLogReadParams, DiagnosticLogReadResult, RpcError } from "@alcomd/sdk";
import { normalizeRequestError } from "./request-errors.ts";

export interface DiagnosticLogSource {
    diagnosticLogRead(params: DiagnosticLogReadParams): Promise<DiagnosticLogReadResult>;
    diagnosticLogCancel(): Promise<void>;
}

export interface DiagnosticLogSnapshot {
    readonly items: readonly SessionLogEntry[];
    readonly notifications: readonly DiagnosticLogItem[];
    readonly gap: boolean;
    readonly error?: RpcError;
}

/** GUI records are ephemeral session logs, never Core business state or RPC DTOs. */
export interface SessionLogEntry extends DiagnosticLogItem {
    readonly source?: "gui";
    readonly displayMessage?: string;
    readonly title?: string;
    readonly tone?: "success" | "info" | "warning" | "error";
}

export interface GuiLogInput {
    message: string;
    title?: string;
    tone: "success" | "info" | "warning" | "error";
    code?: string;
    diagnosticId?: string;
    notify?: boolean;
}

const EMPTY: DiagnosticLogSnapshot = { items: [], notifications: [], gap: false };
const MAX_RETAINED = 1024;

function boundedMessage(value: string): string {
    const result: string[] = [];
    let bytes = 0;
    const encoder = new TextEncoder();
    for (const character of value) {
        const length = encoder.encode(character).length;
        if (bytes + length > 4096) {
            while (bytes > 4093) bytes -= encoder.encode(result.pop()!).length;
            result.push("…");
            break;
        }
        result.push(character);
        bytes += length;
    }
    return result.join("");
}

export function diagnosticLogKey(item: Pick<DiagnosticLogItem, "instanceId" | "sequence">): string {
    return `${item.instanceId}:${item.sequence}`;
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
    private readonly guiInstance = crypto.randomUUID();
    private guiSequence = 0;
    private readonly guiIdentities = new WeakMap<object, DiagnosticLogItem>();

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

    /** Commit a GUI feedback record before a floating view is allowed to exist. */
    recordGui(input: GuiLogInput, identity?: object): SessionLogEntry {
        const prior = identity === undefined ? undefined : this.guiIdentities.get(identity);
        const existing = this.find(prior) ?? (input.diagnosticId === undefined ? undefined
            : this.snapshot.items.find((item) => item.diagnosticId === input.diagnosticId));
        if (prior !== undefined && existing !== undefined) return existing;
        const level = input.tone === "success" ? "info" : input.tone;
        const message = boundedMessage(input.message);
        const metadata = { displayMessage: message, title: input.title?.slice(0, 512), tone: input.tone };
        const entry: SessionLogEntry = existing === undefined ? {
            instanceId: this.guiInstance, sequence: ++this.guiSequence, occurredAtMs: Date.now(),
            level, code: input.code?.slice(0, 96) ?? `gui.${level}`, diagnosticId: input.diagnosticId,
            notify: input.notify ?? true, source: "gui", message, target: "alcomd_gui", ...metadata,
        } : { ...existing, ...metadata };
        if (identity !== undefined) this.guiIdentities.set(identity, entry);
        const items = existing === undefined ? [...this.snapshot.items, entry].slice(-MAX_RETAINED)
            : this.snapshot.items.map((item) => item === existing ? entry : item);
        // A request may already have arrived on the Core log connection. Its
        // contextual view replaces the generic popup, rather than duplicating it.
        const notifications = existing === undefined ? this.snapshot.notifications
            : this.snapshot.notifications.filter((item) => diagnosticLogKey(item) !== diagnosticLogKey(existing));
        this.update({ ...this.snapshot, items, notifications });
        return entry;
    }

    find(reference?: DiagnosticLogItem): SessionLogEntry | undefined {
        if (reference === undefined) return undefined;
        return this.snapshot.items.find((item) => diagnosticLogKey(item) === diagnosticLogKey(reference))
            ?? (reference.diagnosticId === undefined ? undefined
                : this.snapshot.items.find((item) => item.diagnosticId === reference.diagnosticId));
    }

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
                const error = normalizeRequestError(caught);
                if (this.snapshot.error?.code !== error.code) this.recordGui({
                    tone: "warning", code: error.code, notify: false,
                    message: "New logs could not be loaded. The last loaded entries are still shown.",
                });
                this.update({ ...this.snapshot, error });
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
        const retained = baseline ? this.snapshot.items.filter((item) => item.source === "gui" || item.tone !== undefined) : this.snapshot.items;
        const items: SessionLogEntry[] = [...retained];
        const handled = new Set<string>();
        for (const item of fresh) {
            const index = items.findIndex((entry) => diagnosticLogKey(entry) === diagnosticLogKey(item)
                || item.diagnosticId !== undefined && entry.source === "gui" && entry.diagnosticId === item.diagnosticId);
            const gui = index < 0 ? undefined : items[index];
            if (gui === undefined) items.push(item);
            else {
                items[index] = { ...item, displayMessage: gui.displayMessage, title: gui.title, tone: gui.tone };
                handled.add(diagnosticLogKey(item));
            }
        }
        const notifications = newInstance ? [] : result.gap ? this.snapshot.notifications
            : [...this.snapshot.notifications, ...fresh.filter((item) => !handled.has(diagnosticLogKey(item)) && item.notify && (item.level === "error" || item.level === "warning"))].slice(-MAX_RETAINED);
        this.cursor = result.nextCursor;
        if (baseline || fresh.length !== 0 || this.snapshot.error !== undefined || result.gap && !this.snapshot.gap) {
            this.update({ items: items.slice(-MAX_RETAINED), notifications, gap: baseline ? result.gap : this.snapshot.gap || result.gap });
        }
    }
}
