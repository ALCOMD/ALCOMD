import type { OfficialSettings, SettingsGetResult, SettingsUpdate } from "./core-models";
import type { GuiRpcClient } from "./rpc";
import type { RpcError } from "@alcomd/sdk";
import { normalizeRequestError } from "./request-errors";

interface SaveState {
    accepted?: SettingsGetResult;
    settings?: OfficialSettings;
    saving: boolean;
    failed: boolean;
    error?: RpcError;
}

export class UnityHubConflictError extends Error {}
export class StorageDirectoryConflictError extends Error {}
export class UnityArgumentsConflictError extends Error {}

/** UI intent and RPC ordering only; the daemon remains the settings authority. */
export class SettingsAutosave {
    private accepted?: SettingsGetResult;
    private pending: SettingsUpdate = {};
    private directoryBaselines: Partial<OfficialSettings["paths"]> = {};
    private inFlight: SettingsUpdate = {};
    private failed = false;
    private error?: RpcError;
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

    change(update: SettingsUpdate): void {
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
                this.error = undefined;
                await this.drain();
            } catch (caught: unknown) {
                this.failed = true;
                this.error = normalizeRequestError(caught);
                this.publish();
            }
        });
    }

    restoreDefaults(update: SettingsUpdate, previous: SettingsUpdate): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            await this.drain();
            this.requireActive();
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            const current = await this.client.settingsGet();
            this.requireActive();
            this.apply(current);
            for (const key of Object.keys(update) as Array<keyof SettingsUpdate>) {
                const desired = update[key];
                const baseline = previous[key];
                const actual = current.settings[key];
                if (typeof desired === "object" && desired !== null && !Array.isArray(desired)) {
                    for (const [field, value] of Object.entries(desired)) {
                        const old = (baseline as Record<string, unknown> | undefined)?.[field];
                        const fresh = (actual as unknown as Record<string, unknown>)[field];
                        if (!sameValue(fresh, value) && !sameValue(fresh, old)) throw { code: "revision_conflict", message: "" };
                    }
                } else if (!sameValue(actual, desired) && !sameValue(actual, baseline)) {
                    throw { code: "revision_conflict", message: "" };
                }
            }
            const patch = difference(update, current.settings);
            if (Object.keys(patch).length === 0) return current;
            const next = await this.client.settingsUpdate(current.revision, patch);
            this.requireActive();
            this.apply(next);
            return next;
        });
    }

    saveUnityHub(path: string | null, retry?: { previousPath: string | null }): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            await this.drain();
            this.requireActive();
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
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

    saveDirectory(key: keyof OfficialSettings["paths"], path: string | null, previousPath: string | null): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            await this.drain();
            this.requireActive();
            if (!this.failed) {
                this.apply(await this.client.settingsGet());
                this.requireActive();
                const current = this.accepted!;
                if (current.settings.paths[key] === path) return current;
                if (current.settings.paths[key] !== previousPath) throw new StorageDirectoryConflictError("Save location changed.");
            }
            this.directoryBaselines[key] = previousPath;
            this.pending = merge(this.pending, { paths: { [key]: path } });
            this.publish();
            await this.drain();
            this.requireActive();
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            return this.accepted!;
        });
    }

    saveUnityArguments(arguments_: string[] | null, previousArguments: string[] | null): Promise<SettingsGetResult> {
        return this.enqueue(async () => {
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            await this.drain();
            this.requireActive();
            if (this.failed) throw this.error ?? { code: "internal_error", message: "" };
            this.apply(await this.client.settingsGet());
            this.requireActive();
            const current = this.accepted!;
            const actual = current.settings.unity.defaultLaunchArguments;
            if (JSON.stringify(actual) === JSON.stringify(arguments_)) return current;
            if (JSON.stringify(actual) !== JSON.stringify(previousArguments)) {
                throw new UnityArgumentsConflictError("Unity launch defaults changed.");
            }
            const next = await this.client.settingsUpdate(current.revision, { unity: { defaultLaunchArguments: arguments_ } });
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
        this.directoryBaselines = {};
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
                projects: { ...this.accepted.settings.projects, ...desired.projects },
                backups: { ...this.accepted.settings.backups, ...desired.backups },
                appearance: { ...this.accepted.settings.appearance, ...desired.appearance },
                packages: { ...this.accepted.settings.packages, ...desired.packages },
                window: { ...this.accepted.settings.window, ...desired.window },
                paths: { ...this.accepted.settings.paths, ...desired.paths },
                unity: { ...this.accepted.settings.unity, ...desired.unity }
            },
            saving: this.jobs > 0,
            failed: this.failed,
            error: this.error
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
                let directoryConflict = false;
                for (const key of Object.keys(this.directoryBaselines) as Array<keyof OfficialSettings["paths"]>) {
                    const desired = intent.paths?.[key];
                    const current = this.accepted!.settings.paths[key];
                    if (desired !== undefined && current !== desired && current !== this.directoryBaselines[key]) {
                        delete intent.paths![key];
                        delete this.directoryBaselines[key];
                        directoryConflict = true;
                    }
                }
                if (intent.paths !== undefined && Object.keys(intent.paths).length === 0) delete intent.paths;
                if (directoryConflict) throw { code: "revision_conflict", message: "" };
                const update = difference(intent, this.accepted!.settings);
                if (Object.keys(update).length > 0) this.apply(await this.client.settingsUpdate(this.accepted!.revision, update));
                if (intent.paths !== undefined) for (const key of Object.keys(intent.paths) as Array<keyof OfficialSettings["paths"]>) delete this.directoryBaselines[key];
                this.inFlight = {};
                this.publish();
            } catch (caught: unknown) {
                this.pending = merge(intent, this.pending);
                this.inFlight = {};
                this.failed = true;
                this.error = normalizeRequestError(caught);
                this.publish();
            }
        }
    }
}

function merge(first: SettingsUpdate, second: SettingsUpdate): SettingsUpdate {
    return {
        ...first, ...second,
        ...(first.appearance === undefined && second.appearance === undefined ? {} : { appearance: { ...first.appearance, ...second.appearance } }),
        ...(first.unity === undefined && second.unity === undefined ? {} : { unity: { ...first.unity, ...second.unity } }),
        ...(first.window === undefined && second.window === undefined ? {} : { window: { ...first.window, ...second.window } }),
        ...(first.paths === undefined && second.paths === undefined ? {} : { paths: { ...first.paths, ...second.paths } }),
        ...(first.projects === undefined && second.projects === undefined ? {} : { projects: { ...first.projects, ...second.projects } }),
        ...(first.backups === undefined && second.backups === undefined ? {} : { backups: { ...first.backups, ...second.backups } }),
        ...(first.packages === undefined && second.packages === undefined ? {} : { packages: { ...first.packages, ...second.packages } })
    };
}

function sameValue(first: unknown, second: unknown): boolean { return JSON.stringify(first) === JSON.stringify(second); }

function difference(intent: SettingsUpdate, accepted: OfficialSettings): SettingsUpdate {
    const result: SettingsUpdate = {};
    if (intent.locale !== undefined && intent.locale !== accepted.locale) result.locale = intent.locale;
    if (intent.window !== undefined) {
        const window = Object.fromEntries(Object.entries(intent.window).filter(([key, value]) => value !== accepted.window[key as keyof typeof intent.window]));
        if (Object.keys(window).length > 0) result.window = window;
    }
    if (intent.appearance !== undefined) {
        const appearance = Object.fromEntries(Object.entries(intent.appearance).filter(([key, value]) => value !== accepted.appearance[key as keyof typeof intent.appearance]));
        if (Object.keys(appearance).length > 0) result.appearance = appearance;
    }
    if (intent.packages !== undefined) {
        const packages = Object.fromEntries(Object.entries(intent.packages).filter(([key, value]) => JSON.stringify(value) !== JSON.stringify(accepted.packages[key as keyof typeof intent.packages])));
        if (Object.keys(packages).length > 0) result.packages = packages;
    }
    if (intent.projects?.autoDiscover !== undefined && intent.projects.autoDiscover !== accepted.projects.autoDiscover) {
        result.projects = { autoDiscover: intent.projects.autoDiscover };
    }
    if (intent.backups !== undefined) {
        const backups = Object.fromEntries(Object.entries(intent.backups).filter(([key, value]) => value !== accepted.backups[key as keyof typeof intent.backups]));
        if (Object.keys(backups).length > 0) result.backups = backups;
    }
    if (intent.unity?.hubPath !== undefined && intent.unity.hubPath !== accepted.unity.hubPath) result.unity = { hubPath: intent.unity.hubPath };
    if (intent.unity?.defaultLaunchArguments !== undefined && !sameValue(intent.unity.defaultLaunchArguments, accepted.unity.defaultLaunchArguments)) {
        result.unity = { ...result.unity, defaultLaunchArguments: intent.unity.defaultLaunchArguments };
    }
    if (intent.paths !== undefined) {
        const paths = Object.fromEntries(Object.entries(intent.paths).filter(([key, value]) => value !== accepted.paths[key as keyof typeof intent.paths]));
        if (Object.keys(paths).length > 0) result.paths = paths;
    }
    return result;
}
