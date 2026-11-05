import { useCallback, useSyncExternalStore } from "react";

export type UnityWindowState = "closed" | "opening" | "open" | "ready" | "unavailable";

export interface UnityWindowTarget {
    projectId: string;
    revision: number;
}

export interface UnityWindowSnapshot {
    observing: boolean;
    version: number;
    states: [string, UnityWindowState][];
}

export interface GuiUnityWindows {
    start(token: string, targets: UnityWindowTarget[]): Promise<UnityWindowSnapshot>;
    next(token: string, after: number): Promise<UnityWindowSnapshot>;
    stop(token: string): Promise<void>;
    focus(projectId: string, revision: number): Promise<void>;
}

const TARGET_LIMIT = 128;

/** One outstanding native change read for all mounted launch actions. */
class UnityWindowObservation {
    private readonly listeners = new Map<() => void, UnityWindowTarget>();
    private states = new Map<string, UnityWindowState>();
    private token: string | undefined;
    private key = "";
    private scheduled = false;
    private changing = false;

    constructor(private readonly client: GuiUnityWindows) {}

    subscribe(target: UnityWindowTarget, notify: () => void): () => void {
        this.listeners.set(notify, target);
        this.schedule();
        return () => {
            this.listeners.delete(notify);
            this.schedule();
        };
    }

    state(projectId: string): UnityWindowState {
        return this.states.get(projectId) ?? "unavailable";
    }

    private targets(): UnityWindowTarget[] {
        const projects = new Map<string, UnityWindowTarget>();
        for (const target of this.listeners.values()) {
            if ((projects.get(target.projectId)?.revision ?? 0) < target.revision) projects.set(target.projectId, target);
        }
        return [...projects.values()].sort((a, b) => a.projectId.localeCompare(b.projectId)).slice(0, TARGET_LIMIT);
    }

    private schedule() {
        if (this.scheduled || this.changing) return;
        this.scheduled = true;
        queueMicrotask(() => {
            this.scheduled = false;
            void this.reconcile();
        });
    }

    private async reconcile() {
        const targets = this.targets();
        const key = JSON.stringify(targets);
        if (key === this.key) return;
        this.key = key;
        this.changing = true;
        const previous = this.token;
        this.token = undefined;
        try {
            if (previous !== undefined) await this.client.stop(previous).catch(() => {});
            if (targets.length === 0) {
                this.publish([]);
                return;
            }
            const token = crypto.randomUUID();
            this.token = token;
            const initial = await this.client.start(token, targets);
            if (key === JSON.stringify(this.targets())) {
                this.publish(initial.states);
                if (initial.observing) void this.consume(token, initial.version);
            }
        } catch {
            this.publish([]);
            if (this.token !== undefined) await this.client.stop(this.token).catch(() => {});
        } finally {
            this.changing = false;
            if (key !== JSON.stringify(this.targets())) this.schedule();
        }
    }

    private async consume(token: string, version: number) {
        while (this.token === token) {
            try {
                const next = await this.client.next(token, version);
                if (this.token !== token) return;
                this.publish(next.states);
                version = next.version;
            } catch {
                if (this.token === token) {
                    this.publish([]);
                    void this.client.stop(token).catch(() => {});
                }
                return;
            }
        }
    }

    private publish(states: UnityWindowSnapshot["states"]) {
        this.states = new Map(states);
        for (const notify of this.listeners.keys()) notify();
    }
}

const observations = new WeakMap<GuiUnityWindows, UnityWindowObservation>();

export function useUnityWindowState(client: GuiUnityWindows | undefined, projectId: string | undefined, revision: number | undefined): UnityWindowState {
    let observation = client === undefined ? undefined : observations.get(client);
    if (client !== undefined && observation === undefined) {
        observation = new UnityWindowObservation(client);
        observations.set(client, observation);
    }
    const active = projectId === undefined || revision === undefined ? undefined : observation;
    const subscribe = useCallback((notify: () => void) => {
        if (active === undefined || projectId === undefined || revision === undefined) return () => {};
        return active.subscribe({ projectId, revision }, notify);
    }, [active, projectId, revision]);
    const snapshot = useCallback(() => projectId === undefined ? "unavailable" : active?.state(projectId) ?? "unavailable", [active, projectId]);
    return useSyncExternalStore(subscribe, snapshot, snapshot);
}
