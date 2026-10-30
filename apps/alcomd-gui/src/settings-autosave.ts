import type { OfficialSettings, SettingsGetResult, SettingsUpdate } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import type { ThemePreferences } from "@alcomd/ui";

type OrdinaryUpdate = Omit<SettingsUpdate, "appearance"> & {
    appearance?: Pick<NonNullable<SettingsUpdate["appearance"]>, "density" | "motion">;
};

interface SaveState {
    accepted?: SettingsGetResult;
    settings?: OfficialSettings;
    saving: boolean;
    failed: boolean;
}

export class UnityHubConflictError extends Error {}

/** UI intent and RPC ordering only; the daemon remains the settings authority. */
export class SettingsAutosave {
    private accepted?: SettingsGetResult;
    private pending: OrdinaryUpdate = {};
    private inFlight: OrdinaryUpdate = {};
    private failed = false;
    private stopped = false;
    private scheduled = false;
    private jobs = 0;
    private tail = Promise.resolve();
    private listeners = new Set<() => void>();
    private state: SaveState = { saving: false, failed: false };
    private client: Pick<GuiRpcClient, "settingsGet" | "settingsUpdate">;
    private onApplied: (snapshot: SettingsGetResult) => void;

    constructor(client: Pick<GuiRpcClient, "settingsGet" | "settingsUpdate">, onApplied: (snapshot: SettingsGetResult) => void) {
        this.client = client;
        this.onApplied = onApplied;
    }

    getSnapshot = (): SaveState => this.state;
    subscribe = (listener: () => void): (() => void) => {
        this.listeners.add(listener);
        return () => { this.listeners.delete(listener); };
    };

    seed(snapshot: SettingsGetResult): void {
        if (this.stopped || (this.accepted !== undefined && snapshot.revision < this.accepted.revision)) return;
        this.accepted = snapshot;
        this.publish();
    }

    change(update: OrdinaryUpdate): void {
        if (this.stopped) return;
        this.pending = merge(this.pending, structuredClone(update));
        this.publish();
        this.schedule();
    }

    retry(): Promise<void> {
        if (this.jobs > 0 || this.stopped) return Promise.resolve();
        return this.enqueue(async () => {
            try {
                this.apply(await this.client.settingsGet());
                this.failed = false;
                await this.drain();
            } catch {
                this.failed = true;
                this.publish();
            }
        });
    }

    saveTheme(theme: ThemePreferences): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            if (this.failed) throw new Error("Settings save must be retried first.");
            await this.drain();
            this.requireActive();
            if (this.failed) throw new Error("Settings save must be retried first.");
            if (this.accepted === undefined) this.apply(await this.client.settingsGet());
            this.requireActive();
            const next = await this.client.settingsUpdate(this.accepted!.revision, { appearance: theme });
            this.apply(next);
            return next;
        });
    }

    saveUnityHub(path: string | null, retry?: { previousPath: string | null }): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            if (this.failed) throw new Error("Settings save must be retried first.");
            await this.drain();
            this.requireActive();
            if (this.failed) throw new Error("Settings save must be retried first.");
            const redetect = path === null && this.accepted?.settings.unity.hubPath === null;
            if (retry !== undefined || this.accepted === undefined || redetect) this.apply(await this.client.settingsGet());
            this.requireActive();
            const current = this.accepted!;
            if (current.settings.unity.hubPath === path) return current;
            if (redetect && retry === undefined) {
                throw new UnityHubConflictError("Unity Hub location changed before automatic detection.");
            }
            if (retry !== undefined && current.settings.unity.hubPath !== retry.previousPath) {
                throw new UnityHubConflictError("Unity Hub location changed while the save outcome was unknown.");
            }
            const next = await this.client.settingsUpdate(current.revision, { unity: { hubPath: path } });
            this.requireActive();
            this.apply(next);
            return next;
        });
    }

    reload(): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            const snapshot = await this.client.settingsGet();
            this.apply(snapshot);
            return snapshot;
        });
    }

    stop(): void {
        this.stopped = true;
        this.pending = {};
        this.listeners.clear();
    }

    private apply(snapshot: SettingsGetResult): void {
        if (this.stopped) return;
        this.seed(snapshot);
        this.onApplied(snapshot);
    }

    private requireActive(): void {
        if (this.stopped) throw new Error("Settings connection changed.");
    }

    private publish(): void {
        if (this.stopped) return;
        const desired = merge(this.inFlight, this.pending);
        this.state = {
            accepted: this.accepted,
            settings: this.accepted === undefined ? undefined : {
                ...this.accepted.settings,
                ...desired,
                appearance: { ...this.accepted.settings.appearance, ...desired.appearance },
                packages: { ...this.accepted.settings.packages, ...desired.packages },
                unity: { ...this.accepted.settings.unity, ...desired.unity }
            },
            saving: this.jobs > 0,
            failed: this.failed
        };
        for (const listener of this.listeners) listener();
    }

    private enqueue<T>(task: () => Promise<T>): Promise<T> {
        this.jobs += 1;
        this.publish();
        const job = this.tail.then(() => {
            this.requireActive();
            return task();
        });
        this.tail = job.then(() => undefined, () => undefined);
        const finish = () => { this.jobs -= 1; this.publish(); };
        void job.then(finish, finish);
        return job;
    }

    private schedule(): void {
        if (this.scheduled || this.failed || this.stopped || Object.keys(this.pending).length === 0) return;
        this.scheduled = true;
        const finish = () => { this.scheduled = false; this.schedule(); };
        void this.enqueue(() => this.drain()).then(finish, finish);
    }

    private async drain(): Promise<void> {
        while (!this.stopped && !this.failed && Object.keys(this.pending).length > 0) {
            const intent = this.pending;
            this.pending = {};
            this.inFlight = intent;
            this.publish();
            try {
                if (this.accepted === undefined) this.apply(await this.client.settingsGet());
                this.requireActive();
                const update = difference(intent, this.accepted!.settings);
                if (Object.keys(update).length > 0) this.apply(await this.client.settingsUpdate(this.accepted!.revision, update));
                this.inFlight = {};
                this.publish();
            } catch {
                this.pending = merge(intent, this.pending);
                this.inFlight = {};
                this.failed = true;
                this.publish();
            }
        }
    }
}

function merge(first: OrdinaryUpdate, second: OrdinaryUpdate): OrdinaryUpdate {
    return {
        ...first, ...second,
        ...(first.appearance === undefined && second.appearance === undefined ? {} : { appearance: { ...first.appearance, ...second.appearance } }),
        ...(first.unity === undefined && second.unity === undefined ? {} : { unity: { ...first.unity, ...second.unity } }),
        ...(first.packages === undefined && second.packages === undefined ? {} : { packages: { ...first.packages, ...second.packages } })
    };
}

function difference(intent: OrdinaryUpdate, accepted: OfficialSettings): OrdinaryUpdate {
    const result: OrdinaryUpdate = {};
    if (intent.locale !== undefined && intent.locale !== accepted.locale) result.locale = intent.locale;
    if (intent.appearance !== undefined) {
        const appearance = Object.fromEntries(Object.entries(intent.appearance).filter(([key, value]) => value !== accepted.appearance[key as keyof typeof intent.appearance]));
        if (Object.keys(appearance).length > 0) result.appearance = appearance;
    }
    if (intent.packages !== undefined) {
        const packages = Object.fromEntries(Object.entries(intent.packages).filter(([key, value]) => JSON.stringify(value) !== JSON.stringify(accepted.packages[key as keyof typeof intent.packages])));
        if (Object.keys(packages).length > 0) result.packages = packages;
    }
    if (intent.unity?.hubPath !== undefined && intent.unity.hubPath !== accepted.unity.hubPath) result.unity = { hubPath: intent.unity.hubPath };
    return result;
}
