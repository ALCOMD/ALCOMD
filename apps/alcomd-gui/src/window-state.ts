import type { RpcError } from "@alcomd/sdk";

export type WindowGeometry = readonly [width: number, height: number, maximized: boolean];
type Unlisten = () => void;
export interface WindowStateHost {
    configure(enabled: boolean, geometry?: WindowGeometry): Promise<WindowGeometry | undefined>;
    listen(changed: (geometry: unknown) => void, failed: (error: RpcError) => void): Promise<Unlisten>;
}

export function windowGeometry(value: unknown): WindowGeometry | undefined {
    if (!Array.isArray(value) || value.length !== 3) return undefined;
    const [width, height, maximized] = value as unknown[];
    return typeof width === "number" && Number.isFinite(width) && width >= 320 && width <= 32768
        && typeof height === "number" && Number.isFinite(height) && height >= 480 && height <= 32768
        && typeof maximized === "boolean" ? [width, height, maximized] : undefined;
}

/** The switch remains Core-owned. Only disposable window geometry is cached here. */
export class WindowStateMemory {
    private enabled = false;
    private setting?: boolean;
    private initialized = false;
    private disposed = false;
    private generation = 0;
    private queue = Promise.resolve();
    private listening?: Promise<Unlisten>;
    private reported?: string;

    private readonly host: WindowStateHost;
    private readonly storage: () => Storage;
    private readonly key: string;
    private readonly feedback: (error: RpcError | undefined) => void;

    constructor(host: WindowStateHost, storage: () => Storage, key: string, feedback: (error: RpcError | undefined) => void) {
        this.host = host; this.storage = storage; this.key = key; this.feedback = feedback;
    }

    apply(enabled: boolean): Promise<void> {
        if (this.disposed || this.setting === enabled) return this.queue;
        this.setting = enabled;
        this.enabled = enabled;
        const generation = ++this.generation;
        const restore = !this.initialized && enabled ? this.read() : undefined;
        this.initialized = true;
        if (!enabled) {
            // A disabled preference must not produce startup warnings about an unused cache.
            try { this.storage().removeItem(this.key); } catch { /* Disposable view state only. */ }
            this.reported = undefined; this.feedback(undefined);
        }
        this.queue = this.queue.then(async () => {
            if (this.disposed || generation !== this.generation) return;
            if (enabled) await this.listen();
            const current = await this.host.configure(enabled, restore);
            if (enabled && !this.disposed && generation === this.generation && current !== undefined) this.write(current);
        }).catch((error: unknown) => {
            if (!this.disposed && generation === this.generation) this.report(error);
        });
        return this.queue;
    }

    startupReadFailed(): Promise<void> {
        if (this.initialized || this.disposed) return this.queue;
        this.initialized = true;
        // Keep an unread cache, but never resize a window on a later reconnect.
        this.queue = this.queue.then(async () => { if (!this.disposed) await this.host.configure(false); }).catch(() => {});
        return this.queue;
    }

    dispose(): void {
        this.disposed = true;
        this.enabled = false;
        void this.listening?.then((unlisten) => unlisten()).catch(() => {});
    }

    private listen(): Promise<Unlisten> {
        this.listening ??= this.host.listen((value) => {
            if (this.enabled && !this.disposed) {
                const geometry = windowGeometry(value);
                if (geometry !== undefined) this.write(geometry);
            }
        }, (error) => { if (this.enabled && !this.disposed) this.report(error); }).catch((error: unknown) => {
            this.listening = undefined;
            throw error;
        });
        return this.listening;
    }

    private read(): WindowGeometry | undefined {
        try {
            const text = this.storage().getItem(this.key);
            if (text === null || text.length > 256) return undefined;
            return windowGeometry(JSON.parse(text));
        } catch { return undefined; }
    }

    private write(geometry: WindowGeometry): void {
        const valid = windowGeometry(geometry);
        if (valid === undefined) return;
        try {
            this.storage().setItem(this.key, JSON.stringify(valid));
            this.reported = undefined; this.feedback(undefined);
        } catch {
            this.report({ code: "window_state_cache_unavailable", message: "" });
        }
    }

    private report(error: unknown): void {
        const code = typeof error === "object" && error !== null && "code" in error && error.code === "window_state_cache_unavailable"
            ? "window_state_cache_unavailable" : "window_state_unavailable";
        if (this.reported === code) return;
        this.reported = code;
        this.feedback({ code, message: "" });
    }
}
