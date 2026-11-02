import type { RegistryCursor, RepositorySnapshot, SettingsGetResult, SettingsUpdate } from "../../src/core-models";
import type { GuiRpcClient } from "../../src/rpc";

export const H5_SETTINGS_SECRET = "h5-settings-secret-never-rendered";
export const H5_SETTINGS_DIAGNOSTIC = "h5-settings-diagnostic";
export const H5_HUB_PATH = "\\\\?\\C:\\H5 fixture\\Unity Hub.exe";
export const H5_HIDDEN_ID = "00000000-0000-4000-8000-000000000801";
export const H5_VISIBLE_ID = "00000000-0000-4000-8000-000000000802";
export const H5_REPOSITORY_CURSOR: RegistryCursor = { registeredAtMs: 1_700_000_000_000, id: H5_HIDDEN_ID };

interface SettingsRequest { method: string; params: Record<string, unknown> }
interface SettingsFailure { code: string; afterCommit: boolean }
interface H5SettingsFixture {
    requests: SettingsRequest[];
    commits: Array<{ expectedRevision: number; update: SettingsUpdate; revision: number }>;
    failNext(method: string, code: string, afterCommit?: boolean): void;
}

declare global { interface Window { h5Settings: H5SettingsFixture } }

/** Public DTO fixture only: no native Core, filesystem, or production settings access. */
export function withH5SettingsClient(base: GuiRpcClient, query: URLSearchParams): GuiRpcClient {
    let current: SettingsGetResult | undefined;
    let initialization: Promise<void> | undefined;
    const failures = new Map<string, SettingsFailure[]>();
    const fixture: H5SettingsFixture = {
        requests: [], commits: [],
        failNext(method, code, afterCommit = false) {
            failures.set(method, [...failures.get(method) ?? [], { code, afterCommit }]);
        }
    };
    window.h5Settings = fixture;

    async function initialize(): Promise<void> {
        initialization ??= base.settingsGet().then((snapshot) => {
            current = structuredClone(snapshot);
            if (query.has("h5-hidden-limit")) current.settings.packages.hiddenRepositoryIds = [H5_HIDDEN_ID,
                ...Array.from({ length: 255 }, (_, index) => `00000000-0000-4000-8000-${String(1000 + index).padStart(12, "0")}`)].sort();
        });
        await initialization;
    }
    function error(code: string) {
        return { code, message: H5_SETTINGS_SECRET, diagnosticId: H5_SETTINGS_DIAGNOSTIC,
            data: { untrustedDetail: H5_SETTINGS_SECRET } };
    }
    async function response<T>(method: string, params: Record<string, unknown>, produce: () => T | Promise<T>): Promise<T> {
        fixture.requests.push({ method, params: structuredClone(params) });
        const failure = failures.get(method)?.shift();
        if (failure !== undefined && !failure.afterCommit) throw error(failure.code);
        const value = await produce();
        if (failure !== undefined) throw error(failure.code);
        return structuredClone(value);
    }
    function repository(repositoryId: string, name: string): RepositorySnapshot {
        const url = `https://h5.example.invalid/${repositoryId}.json`;
        return { repositoryId, name, source: { kind: "remote", url }, declaredId: repositoryId, declaredUrl: url,
            issues: [], revision: 1, refreshedAtMs: 1_700_000_000_000 };
    }
    const methods: Partial<GuiRpcClient> = {
        settingsGet() {
            return response("settings.get", {}, async () => { await initialize(); return current!; });
        },
        settingsUpdate(expectedRevision, update) {
            return response("settings.update", { expectedRevision, update }, async () => {
                await initialize();
                if (current!.revision !== expectedRevision) throw error("revision_conflict");
                const previous = current!;
                const hubPath = update.unity?.hubPath === undefined ? previous.settings.unity.hubPath : update.unity.hubPath;
                current = {
                    ...previous, revision: previous.revision + 1,
                    directories: {
                        projectDirectory: update.paths?.projectDirectory === undefined ? previous.directories.projectDirectory : update.paths.projectDirectory ?? "C:\\Fixture",
                        backupDirectory: update.paths?.backupDirectory === undefined ? previous.directories.backupDirectory : update.paths.backupDirectory ?? "C:\\Fixture\\Backups"
                    },
                    unityHub: update.unity?.hubPath === undefined ? previous.unityHub : hubPath === null
                        ? { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" }
                        : { status: "available", source: "configured", path: hubPath, configuredPathStatus: "available" },
                    settings: {
                        paths: { ...previous.settings.paths, ...update.paths },
                        appearance: { ...previous.settings.appearance, ...update.appearance },
                        packages: { ...previous.settings.packages, ...update.packages },
                        unity: { ...previous.settings.unity, ...update.unity },
                        locale: update.locale ?? previous.settings.locale
                    }
                };
                fixture.commits.push({ expectedRevision, update: structuredClone(update), revision: current.revision });
                return current;
            });
        },
        selectUnityHub(currentPath) { return response("selectUnityHub", { currentPath }, () => H5_HUB_PATH); },
        unityInstallationsRefresh() { return response("unity.installations.refresh", {}, () => base.unityInstallationsRefresh()); },
        repositoriesList(cursor) {
            return response("repositories.list", cursor === undefined ? {} : { cursor }, () => cursor === undefined
                ? { repositories: [repository(H5_HIDDEN_ID, "H5 hidden source")], nextCursor: H5_REPOSITORY_CURSOR }
                : { repositories: [repository(H5_VISIBLE_ID, "H5 visible source")] });
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
