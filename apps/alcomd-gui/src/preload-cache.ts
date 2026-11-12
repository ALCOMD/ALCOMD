export const PRELOAD_IDLE_MS = 3000;
export const PRELOAD_HOVER_MS = 50;
const MAX_ENTRIES = 16;
const MAX_BYTES = 2 * 1024 * 1024;
const MAX_RUNNING = 2;
const WORK = Symbol("preload-work");
const claimedSignals = new WeakSet<AbortSignal>();
export function isPreloadClaimed(signal: AbortSignal) { return claimedSignals.has(signal); }
export function withPreloadWork<T>(value: T, work: Promise<unknown>) { return { [WORK]: true as const, value, work }; }
function hasWork(value: unknown): value is ReturnType<typeof withPreloadWork<unknown>> { return typeof value === "object" && value !== null && WORK in value; }
export function preloadWork(value: unknown) { return hasWork(value) ? value.work : undefined; }

interface Entry {
    key: string;
    owners: Set<string>;
    run(signal: AbortSignal): Promise<unknown>;
    discard(): void;
    weight?(): number;
    valid?(): boolean;
    controller: AbortController;
    promise: Promise<unknown>;
    resolve(value: unknown): void;
    reject(error: unknown): void;
    started: boolean;
    settled: boolean;
    claimed: boolean;
    bytes: number;
    touched: number;
    timer?: ReturnType<typeof setTimeout>;
}

const aborted = () => new DOMException("Preload discarded", "AbortError");

/** Disposable view data only. Running discarded reads keep their concurrency slot. */
export class PreloadCache {
    private entries = new Map<string, Entry>();
    private handoffs = new Map<string, Map<string, Entry>>();
    private running = 0;
    private disposed = false;
    constructor(private readonly idleMs = PRELOAD_IDLE_MS) {}
    activate() { this.disposed = false; this.pump(); }

    retain<T>(key: string, owner: string, run: (signal: AbortSignal) => Promise<T | ReturnType<typeof withPreloadWork<T>>>, discard: () => void = () => {}, weight?: () => number, valid?: () => boolean): Promise<T> {
        const handedOff = this.handoffs.get(key)?.get(owner);
        if (handedOff) {
            if (!handedOff.valid || handedOff.valid()) return handedOff.promise as Promise<T>;
            this.releaseHandoff(key, owner);
        }
        let entry = this.entries.get(key);
        if (entry?.valid && !entry.valid()) { this.drop(key); entry = undefined; }
        if (!entry) {
            let resolve!: Entry["resolve"];
            let reject!: Entry["reject"];
            const promise = new Promise<unknown>((yes, no) => { resolve = yes; reject = no; });
            void promise.catch(() => {});
            entry = { key, owners: new Set(), run, discard, weight, valid, controller: new AbortController(), promise, resolve, reject, started: false, settled: false, claimed: false, bytes: 0, touched: performance.now() };
            this.entries.set(key, entry);
        }
        globalThis.clearTimeout(entry.timer);
        entry.owners.add(owner);
        entry.touched = performance.now();
        this.trim();
        this.pump();
        return entry.promise as Promise<T>;
    }

    release(key: string, owner: string) {
        this.releaseHandoff(key, owner);
        const entry = this.entries.get(key);
        if (!entry) return;
        entry.owners.delete(owner);
        if (entry.owners.size === 0) {
            try { entry.bytes = entry.weight?.() ?? entry.bytes; }
            catch { entry.bytes = MAX_BYTES + 1; }
            globalThis.clearTimeout(entry.timer);
            entry.timer = globalThis.setTimeout(() => this.drop(key), this.idleMs);
            this.trim();
        }
    }

    forget(key: string, owner: string) {
        this.releaseHandoff(key, owner);
        const entry = this.entries.get(key);
        if (!entry) return;
        entry.owners.delete(owner);
        if (entry.owners.size === 0) this.drop(key, false);
    }

    /** Move ownership to a visible page; never clear the page through the old trigger. */
    take<T>(key: string): Promise<T> | undefined {
        const entry = this.entries.get(key);
        if (!entry || entry.controller.signal.aborted) return undefined;
        if (entry.valid && !entry.valid()) { this.drop(key); return undefined; }
        this.entries.delete(key);
        globalThis.clearTimeout(entry.timer);
        entry.claimed = true;
        claimedSignals.add(entry.controller.signal);
        // The outgoing trigger can still receive focus/pointer events before it unmounts.
        // Only those same interaction owners reuse the handoff; a new visit reads afresh.
        if (entry.owners.size > 0) {
            const owners = this.handoffs.get(key) ?? new Map<string, Entry>();
            for (const owner of entry.owners) owners.set(owner, entry);
            this.handoffs.set(key, owners);
        }
        entry.discard = () => {};
        if (!entry.started) this.start(entry);
        return entry.promise as Promise<T>;
    }

    foreground<T>(key: string, owner: string, run: (signal: AbortSignal) => Promise<T | ReturnType<typeof withPreloadWork<T>>>, discard: () => void, weight?: () => number, valid?: () => boolean): Promise<T> {
        const result = this.retain(key, owner, run, discard, weight, valid);
        const entry = this.entries.get(key);
        if (entry && entry.promise === result && !entry.started) this.start(entry);
        return result;
    }

    drop(key: string, notify = true) {
        const entry = this.entries.get(key);
        if (!entry) return;
        this.entries.delete(key);
        globalThis.clearTimeout(entry.timer);
        entry.controller.abort();
        entry.reject(aborted());
        if (notify) entry.discard();
        this.pump();
    }

    dispose() {
        this.disposed = true;
        this.handoffs.clear();
        for (const key of this.entries.keys()) this.drop(key, false);
    }

    dropInvalid() {
        for (const entry of this.entries.values()) if (entry.valid && !entry.valid()) this.drop(entry.key);
    }

    private releaseHandoff(key: string, owner: string) {
        const owners = this.handoffs.get(key);
        owners?.delete(owner);
        if (owners?.size === 0) this.handoffs.delete(key);
    }

    private trim() {
        let bytes = [...this.entries.values()].reduce((sum, entry) => sum + entry.bytes, 0);
        for (const entry of [...this.entries.values()].sort((a, b) => a.touched - b.touched)) {
            if (this.entries.size <= MAX_ENTRIES && bytes <= MAX_BYTES) break;
            if (entry.owners.size > 0) continue;
            bytes -= entry.bytes;
            this.drop(entry.key);
        }
    }

    private pump() {
        if (this.disposed) return;
        for (const entry of this.entries.values()) {
            if (this.running >= MAX_RUNNING) break;
            if (!entry.started && entry.owners.size > 0) this.start(entry);
        }
    }

    private start(entry: Entry) {
        entry.started = true;
        this.running++;
        void Promise.resolve().then(() => {
            if (this.disposed || entry.controller.signal.aborted) throw aborted();
            return entry.run(entry.controller.signal);
        }).then(async (prepared) => {
            const value = hasWork(prepared) ? prepared.value : prepared;
            const work = hasWork(prepared) ? prepared.work : undefined;
            if (entry.controller.signal.aborted) { await work?.catch(() => {}); return; }
            entry.settled = true;
            try { entry.bytes = entry.weight?.() ?? (JSON.stringify(value)?.length ?? 0) * 2; }
            catch { entry.bytes = MAX_BYTES + 1; }
            entry.resolve(value);
            if (!entry.claimed) this.trim();
            // A page may claim its metadata while the independent list is pending.
            // Keep unfinished speculative work bounded without holding back that page.
            await work?.catch(() => {});
            if (!entry.controller.signal.aborted && !entry.claimed) {
                try { entry.bytes = entry.weight?.() ?? (JSON.stringify(value)?.length ?? 0) * 2; }
                catch { entry.bytes = MAX_BYTES + 1; }
                this.trim();
            }
        }, (error: unknown) => {
            entry.reject(error);
            if (!entry.claimed && this.entries.get(entry.key) === entry) this.drop(entry.key);
        }).finally(() => { this.running--; this.pump(); });
    }
}
