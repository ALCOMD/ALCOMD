import type { SettingsGetResult, SettingsUpdate } from "./core-models.ts";
import { SettingsAutosave, StorageDirectoryConflictError, UnityArgumentsConflictError, UnityHubConflictError } from "./settings-autosave.ts";

const initial: SettingsGetResult = {
    configSchema: 10, revision: 7,
    directories: { projectDirectory: "C:\\Projects", backupDirectory: "C:\\Backups" },
    unityHub: { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" },
    settings: {
        projects: { autoDiscover: true },
        backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false },
        paths: { projectDirectory: null, backupDirectory: null },
        unity: { hubPath: null, defaultLaunchArguments: null },
        window: { rememberState: false, fullscreen: false },
        locale: "en-US",
        appearance: { mode: "system", scheme: "vibrant", sourceColor: null, motion: "system" },
        packages: { showPrerelease: false, hideLocalUserPackages: false, hiddenRepositoryIds: [] }
    }
};

function assert(value: unknown, message: string): asserts value {
    if (!value) throw new Error(message);
}
function equal(actual: unknown, expected: unknown, message: string) {
    assert(JSON.stringify(actual) === JSON.stringify(expected), `${message}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
}
function deferred() {
    let resolve!: () => void;
    const promise = new Promise<void>((done) => { resolve = done; });
    return { promise, resolve };
}
function model() {
    let current = structuredClone(initial);
    const requests: Array<{ expectedRevision: number; update: SettingsUpdate }> = [];
    const applied: SettingsGetResult[] = [];
    const client = {
        settingsGet: async () => structuredClone(current),
        settingsUpdate: async (expectedRevision: number, update: SettingsUpdate) => {
            requests.push({ expectedRevision, update: structuredClone(update) });
            assert(expectedRevision === current.revision, "CAS expected current revision");
            current = {
                configSchema: 10, revision: expectedRevision + 1,
                directories: {
                    projectDirectory: update.paths?.projectDirectory ?? current.directories.projectDirectory,
                    backupDirectory: update.paths?.backupDirectory ?? current.directories.backupDirectory
                },
                unityHub: current.unityHub,
                settings: {
                    ...current.settings, ...update,
                    projects: { ...current.settings.projects, ...update.projects },
                    backups: { ...current.settings.backups, ...update.backups },
                    appearance: { ...current.settings.appearance, ...update.appearance },
                    packages: { ...current.settings.packages, ...update.packages },
                    paths: { ...current.settings.paths, ...update.paths },
                    unity: { ...current.settings.unity, ...update.unity },
                    window: { ...current.settings.window, ...update.window }
                }
            };
            return structuredClone(current);
        }
    };
    const writer = new SettingsAutosave(client, (snapshot) => applied.push(snapshot));
    writer.seed(initial);
    return { writer, client, requests, applied, current: () => current };
}
async function idle(writer: SettingsAutosave) {
    for (let step = 0; step < 200; step += 1) {
        if (!writer.getSnapshot().saving) return;
        await Promise.resolve();
    }
    throw new Error("Settings queue did not settle");
}

{
    const fixture = model();
    const started = deferred();
    const release = deferred();
    const original = fixture.client.settingsUpdate;
    let first = true;
    fixture.client.settingsUpdate = async (...arguments_) => {
        if (first) { first = false; started.resolve(); await release.promise; }
        return original(...arguments_);
    };
    fixture.writer.change({ appearance: { motion: "full" } });
    await started.promise;
    fixture.writer.change({ appearance: { motion: "system" }, packages: { hideLocalUserPackages: true } });
    fixture.writer.change({ packages: { showPrerelease: true } });
    fixture.writer.change({ packages: { hideLocalUserPackages: false } });
    release.resolve();
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8], "serial CAS");
    equal(fixture.requests[1]!.update, { appearance: { motion: "system" }, packages: { showPrerelease: true } }, "latest intents and toggle-back survive in-flight save");
    equal(fixture.current().settings.appearance.motion, "system", "toggle-back must not be trimmed against old authority");
    equal(fixture.current().settings.packages.hideLocalUserPackages, false, "latest package toggle wins without losing its sibling intent");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    let failed = false;
    fixture.client.settingsUpdate = async (...arguments_) => {
        const result = await original(...arguments_);
        if (!failed) { failed = true; throw new Error("Response lost after commit"); }
        return result;
    };
    fixture.writer.change({ appearance: { motion: "full" } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "unknown outcome must be explicit");
    equal(fixture.applied, [], "unacknowledged commit must not claim accepted UI state");
    await fixture.writer.retry();
    await idle(fixture.writer);
    equal(fixture.requests.length, 1, "reread reconciles committed intent without a duplicate write");
    assert(!fixture.writer.getSnapshot().failed, "retry reconciled authority");
    equal(fixture.writer.getSnapshot().accepted!.revision, 8, "accepted actual committed revision");
}

{
    const fixture = model();
    await fixture.client.settingsUpdate(7, { appearance: { mode: "dark", scheme: "neutral" } });
    fixture.writer.change({ appearance: { motion: "full" } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "CAS conflict is explicit");
    fixture.writer.change({ packages: { hideLocalUserPackages: true } });
    await fixture.writer.retry();
    await idle(fixture.writer);
    equal(fixture.current().settings.appearance.mode, "dark", "retry preserves external mode");
    equal(fixture.current().settings.appearance.scheme, "neutral", "retry preserves external scheme");
    equal(fixture.requests.at(-1)!.update, { appearance: { motion: "full" }, packages: { hideLocalUserPackages: true } }, "retry patches only retained intent");
}

{
    const fixture = model();
    fixture.writer.change({ appearance: { motion: "full" } });
    await idle(fixture.writer);
    fixture.writer.change({ appearance: { mode: "dark", scheme: "neutral", sourceColor: "#6CB6FF" } });
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8], "theme and ordinary writes share one order");
    equal(Object.keys(fixture.requests[1]!.update.appearance!).sort(), ["mode", "scheme", "sourceColor"], "theme autosave writes only its changed fields");
    equal(fixture.current().settings.appearance.motion, "full", "theme preserves ordinary saved values");
    fixture.writer.seed(initial);
    equal(fixture.writer.getSnapshot().accepted!.revision, 9, "late snapshots cannot regress authority");
}

{
    const fixture = model();
    const started = deferred();
    const release = deferred();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...arguments_) => { started.resolve(); await release.promise; return original(...arguments_); };
    fixture.writer.change({ appearance: { motion: "reduced" } });
    await started.promise;
    fixture.writer.change({ appearance: { motion: "full" } });
    fixture.writer.stop();
    release.resolve();
    for (let step = 0; step < 30; step += 1) await Promise.resolve();
    equal(fixture.requests.length, 1, "stopped connection must not replay queued intentions");
    equal(fixture.applied.length, 0, "stopped connection must not update new UI authority");
}

{
    const fixture = model();
    const started = deferred();
    const release = deferred();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...arguments_) => { started.resolve(); await release.promise; return original(...arguments_); };
    fixture.writer.change({ appearance: { mode: "dark", scheme: "neutral" } });
    await started.promise;
    fixture.writer.change({ appearance: { motion: "full", sourceColor: "#112233" } });
    fixture.writer.stop();
    release.resolve();
    for (let step = 0; step < 30; step += 1) await Promise.resolve();
    equal(fixture.requests.length, 1, "stop prevents queued theme and ordinary RPC after an in-flight save");
    equal(fixture.applied.length, 0, "stopped theme writes must not apply old UI state");
}

{
    const fixture = model();
    fixture.writer.change({ packages: { showPrerelease: true } });
    await fixture.writer.saveUnityHub("C:\\Tools\\Unity Hub.exe");
    fixture.writer.change({ appearance: { mode: "dark", sourceColor: null, scheme: "neutral" } });
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8, 9], "Hub, ordinary and theme changes serialize together");
    equal(fixture.requests[1]!.update, { unity: { hubPath: "C:\\Tools\\Unity Hub.exe" } }, "Hub save patches only its field");
    equal(fixture.current().settings.packages.showPrerelease, true, "Hub save preserves ordinary settings");
}

{
    const fixture = model();
    const path = "C:\\Tools\\Unity Hub.exe";
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await original(...args); throw new Error("Lost response"); };
    await fixture.writer.saveUnityHub(path).catch(() => undefined);
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, null, "lost reply cannot show unconfirmed path");
    await fixture.writer.saveUnityHub(path, { previousPath: null });
    equal(fixture.requests.length, 1, "Hub retry reads committed path before considering a duplicate update");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (revision, update) => {
        await original(revision, { ...update, unity: { hubPath: "C:\\Canonical\\Unity Hub.exe" } });
        throw new Error("Lost normalized reply");
    };
    await fixture.writer.saveUnityHub("C:\\Tools\\Unity Hub.exe").catch(() => undefined);
    const conflict = await fixture.writer.saveUnityHub("C:\\Tools\\Unity Hub.exe", { previousPath: null }).then(() => false, (error) => error instanceof UnityHubConflictError);
    assert(conflict, "unverifiable changed location requires reselect rather than assuming ownership");
    equal(fixture.requests.length, 1, "Hub retry cannot overwrite an ambiguous changed path");
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, "C:\\Canonical\\Unity Hub.exe", "reconciliation shows actual Core authority");
}

{
    const fixture = model();
    const fresh: SettingsGetResult = { ...structuredClone(initial), unityHub: { status: "available", source: "discovered", path: "C:\\Detected\\Unity Hub.exe", configuredPathStatus: "unset" } };
    fixture.writer.seed(fresh);
    equal(fixture.writer.getSnapshot().accepted!.unityHub, fresh.unityHub, "filesystem evidence refreshes at the same settings revision");
    fixture.writer.seed({ ...initial, revision: 6 });
    equal(fixture.writer.getSnapshot().accepted!.unityHub, fresh.unityHub, "older snapshot cannot regress refreshed Hub evidence");
    equal(fixture.requests.length, 0, "evidence refresh never writes settings");
}

{
    const fixture = model();
    let reads = 0;
    const original = fixture.client.settingsGet;
    fixture.client.settingsGet = async () => {
        reads += 1;
        return { ...await original(), unityHub: { status: "available", source: "discovered", path: "C:\\Detected\\Unity Hub.exe", configuredPathStatus: "unset" } };
    };
    await fixture.writer.saveUnityHub(null);
    equal(reads, 1, "automatic mode always rereads Core evidence");
    equal(fixture.requests.length, 0, "redetection in automatic mode does not write preferences");
    equal(fixture.writer.getSnapshot().accepted!.revision, 7, "redetection preserves the accepted settings revision");
    equal(fixture.writer.getSnapshot().accepted!.unityHub.path, "C:\\Detected\\Unity Hub.exe", "same-revision evidence reaches the view");
}

{
    const fixture = model();
    await fixture.writer.saveUnityHub("C:\\Manual\\Unity Hub.exe");
    await fixture.writer.saveUnityHub(null);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8], "manual selection and automatic reset share serial CAS");
    equal(fixture.requests[1]!.update, { unity: { hubPath: null } }, "automatic mode explicitly clears only Hub preference");
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, null, "automatic mode is accepted only after Core confirms it");
}

{
    const fixture = model();
    const path = "C:\\Manual\\Unity Hub.exe";
    await fixture.writer.saveUnityHub(path);
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await original(...args); throw new Error("Lost clear reply"); };
    await fixture.writer.saveUnityHub(null).catch(() => undefined);
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, path, "unknown clear outcome cannot show automatic mode as accepted");
    await fixture.writer.saveUnityHub(null, { previousPath: path });
    equal(fixture.requests.length, 2, "lost null-clear reply is reconciled without repeating the write");
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, null, "Retry accepts the actual cleared Core preference");
}

{
    const fixture = model();
    const original = fixture.client.settingsGet;
    let failed = false;
    fixture.client.settingsGet = async () => { if (!failed) { failed = true; throw new Error("Read failed"); } return original(); };
    const rejected = await fixture.writer.saveUnityHub(null).then(() => false, () => true);
    assert(rejected, "redetection read errors remain explicit");
    await fixture.writer.saveUnityHub(null, { previousPath: null });
    equal(fixture.requests.length, 0, "redetection Retry retains null intent without an unnecessary write");
}

{
    const fixture = model();
    await fixture.client.settingsUpdate(7, { unity: { hubPath: "C:\\Other client\\Unity Hub.exe" } });
    const conflict = await fixture.writer.saveUnityHub(null).then(() => false, (error) => error instanceof UnityHubConflictError);
    assert(conflict, "redetection does not silently clear a newly observed external manual preference");
    equal(fixture.requests.length, 1, "stale automatic-mode read cannot issue an unseen override");
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.hubPath, "C:\\Other client\\Unity Hub.exe", "conflict exposes current Core authority");
}

{
    const fixture = model();
    fixture.writer.change({ paths: { projectDirectory: "D:\\Projects" } });
    fixture.writer.change({ paths: { backupDirectory: "E:\\Backups" } });
    await idle(fixture.writer);
    equal(fixture.current().settings.paths, { projectDirectory: "D:\\Projects", backupDirectory: "E:\\Backups" }, "directory siblings survive the shared queue");
    equal(fixture.current().settings.appearance, initial.settings.appearance, "directory update preserves appearance");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    let failed = false;
    fixture.client.settingsUpdate = async (...arguments_) => {
        const result = await original(...arguments_);
        if (!failed) { failed = true; throw new Error("Directory reply lost after commit"); }
        return result;
    };
    const rejected = await fixture.writer.saveDirectory("projectDirectory", "D:\\Projects", null).then(() => false, () => true);
    assert(rejected, "unknown directory save stays explicit");
    await fixture.writer.retry();
    equal(fixture.requests.length, 1, "directory Retry reconciles an already committed save");
    equal(fixture.writer.getSnapshot().accepted!.directories.projectDirectory, "D:\\Projects", "effective directory follows acknowledged Core result");
}

{
    const fixture = model();
    await fixture.client.settingsUpdate(7, { paths: { backupDirectory: "E:\\Other backup" } });
    const conflict = await fixture.writer.saveDirectory("backupDirectory", "F:\\Backup", null).then(() => false, (error) => error instanceof StorageDirectoryConflictError);
    assert(conflict, "directory edit cannot overwrite an unseen external change");
    equal(fixture.requests.length, 1, "directory conflict does not write");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (revision, update) => {
        await original(revision, { paths: { ...update.paths, projectDirectory: "\\\\?\\D:\\Projects" } });
        throw new Error("Normalized directory committed before reply loss");
    };
    await fixture.writer.saveDirectory("projectDirectory", "D:\\Projects", null).catch(() => undefined);
    await fixture.writer.retry();
    equal(fixture.requests.length, 1, "unknown normalized path is never submitted twice");
    equal(fixture.writer.getSnapshot().error?.code, "revision_conflict", "different path text is exposed as changed authority");
    await fixture.writer.retry();
    assert(!fixture.writer.getSnapshot().failed, "acknowledged changed authority can clear the conflict without another write");
    equal(fixture.requests.length, 1, "clearing the directory conflict does not rewrite the current location");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    let rejected = false;
    fixture.client.settingsUpdate = async (...arguments_) => {
        if (!rejected) { rejected = true; throw { code: "invalid_request" }; }
        return original(...arguments_);
    };
    await fixture.writer.saveDirectory("backupDirectory", "relative", null).catch(() => undefined);
    assert(fixture.writer.getSnapshot().failed, "invalid directory preserves a global settings failure");
    await fixture.writer.saveDirectory("backupDirectory", "E:\\Corrected", null).catch(() => undefined);
    equal(fixture.requests.length, 0, "editing a failed intent waits for explicit Retry");
    await fixture.writer.retry();
    equal(fixture.current().settings.paths.backupDirectory, "E:\\Corrected", "Retry saves the corrected directory intent");
    equal(fixture.requests.length, 1, "only corrected directory is written");
}

{
    const fixture = model();
    const arguments_ = ["-logFile", "C:\\Logs with spaces\\Editor.log", "", "  spaced  ", '"literal" \\value'];
    fixture.writer.change({ packages: { showPrerelease: true } });
    await fixture.writer.saveUnityArguments(arguments_, null);
    fixture.writer.change({ appearance: { mode: "dark", sourceColor: null, scheme: "neutral" } });
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8, 9], "global argv serializes with ordinary and theme writes");
    equal(fixture.requests[1]!.update, { unity: { defaultLaunchArguments: arguments_ } }, "argv values are forwarded intact in a narrow patch");
    equal(fixture.current().settings.unity.hubPath, null, "argv save preserves Hub preference");
}

{
    const fixture = model();
    await fixture.writer.saveUnityArguments([], null);
    await fixture.writer.saveUnityArguments(null, []);
    equal(fixture.requests.map((request) => request.update), [{ unity: { defaultLaunchArguments: [] } }, { unity: { defaultLaunchArguments: null } }], "empty override and built-in defaults are distinct");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await original(...args); throw new Error("Lost response"); };
    await fixture.writer.saveUnityArguments(["-batchmode"], null).catch(() => undefined);
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.defaultLaunchArguments, null, "unconfirmed argv is not shown as saved");
    await fixture.writer.saveUnityArguments(["-batchmode"], null);
    equal(fixture.requests.length, 1, "argv retry reconciles committed values without duplicating the write");
}

{
    const fixture = model();
    await fixture.client.settingsUpdate(7, { unity: { defaultLaunchArguments: ["-batchmode"] } });
    const conflict = await fixture.writer.saveUnityArguments(["-nographics"], null).then(() => false, (error) => error instanceof UnityArgumentsConflictError);
    assert(conflict, "competing global argv edits require a new editor snapshot");
    equal(fixture.requests.length, 1, "argv save does not overwrite a concurrent default edit");
    equal(fixture.writer.getSnapshot().accepted!.settings.unity.defaultLaunchArguments, ["-batchmode"], "conflict reloads Core authority");
}

{
    const fixture = model();
    equal(fixture.writer.getSnapshot().accepted!.settings.window.rememberState, false, "window state remembering starts disabled");
    fixture.writer.change({ window: { rememberState: true } });
    await idle(fixture.writer);
    fixture.writer.change({ window: { rememberState: false } });
    await idle(fixture.writer);
    equal(fixture.requests, [
        { expectedRevision: 7, update: { window: { rememberState: true } } },
        { expectedRevision: 8, update: { window: { rememberState: false } } }
    ], "window preference toggles use narrow serialized patches");
    equal(fixture.current().settings.unity, initial.settings.unity, "window preference does not alter Unity settings");
}

{
    const fixture = model();
    fixture.writer.change({ window: { rememberState: true } });
    fixture.writer.change({ window: { rememberState: false } });
    await idle(fixture.writer);
    equal(fixture.requests, [], "a rapid reverted window toggle has no effective update");
    equal(fixture.writer.getSnapshot().accepted!.settings.window.rememberState, false, "reverted intent never becomes accepted");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await original(...args); throw new Error("Lost response"); };
    fixture.writer.change({ window: { rememberState: true } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "unknown window save remains failed until checked");
    equal(fixture.writer.getSnapshot().accepted!.settings.window.rememberState, false, "unknown window save cannot enable the native behavior");
    equal(fixture.writer.getSnapshot().settings!.window.rememberState, true, "window edit intent is retained for explicit retry");
    equal(fixture.applied.length, 0, "unknown window setting is never published as applied");
    await fixture.writer.retry();
    equal(fixture.requests.length, 1, "window retry checks the committed value without a duplicate write");
    equal(fixture.writer.getSnapshot().accepted!.settings.window.rememberState, true, "only reconciled Core authority enables remembering");
    equal(fixture.applied.length, 1, "accepted window setting is applied after reconciliation");
}

{
    const fixture = model();
    const waiting = deferred();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await waiting.promise; return original(...args); };
    fixture.writer.change({ appearance: { motion: "full" } });
    await Promise.resolve();
    fixture.writer.change({ backups: { includeVpmPackages: false } });
    fixture.writer.change({ backups: { includeLibrary: true, compressionMode: "maximum" } });
    fixture.writer.change({ projects: { autoDiscover: false } });
    equal(fixture.writer.getSnapshot().settings!.backups, { compressionMode: "maximum", includeVpmPackages: false, includeLibrary: true }, "backup drafts retain sibling options during another write");
    waiting.resolve();
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8], "project and backup edits share the existing serialized CAS queue");
    equal(fixture.current().settings.backups, { compressionMode: "maximum", includeVpmPackages: false, includeLibrary: true }, "all edited backup defaults survive the in-flight update");
    equal(fixture.current().settings.projects.autoDiscover, false, "false auto-discovery intent is saved");
    equal(fixture.current().settings.paths, initial.settings.paths, "default edits do not overwrite directory settings");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...args) => { await original(...args); throw new Error("Lost response"); };
    fixture.writer.change({ projects: { autoDiscover: false }, backups: { includeLibrary: true } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "unknown project and backup save keeps an explicit retry");
    equal(fixture.writer.getSnapshot().accepted!.settings.projects.autoDiscover, true, "unknown outcome does not claim the discovery preference was accepted");
    await fixture.writer.retry();
    equal(fixture.requests.length, 1, "lost project and backup reply reconciles without repeating a committed write");
    equal(fixture.writer.getSnapshot().accepted!.settings.backups, { compressionMode: "fast", includeVpmPackages: true, includeLibrary: true }, "reconciliation preserves untouched backup defaults");
}

function customizedModel() {
    const fixture = model();
    const current = fixture.current();
    current.settings = {
        projects: { autoDiscover: false },
        backups: { compressionMode: "maximum", includeVpmPackages: false, includeLibrary: true },
        paths: { projectDirectory: "D:\\Projects", backupDirectory: "E:\\Backups" },
        unity: { hubPath: "C:\\Tools\\Unity Hub.exe", defaultLaunchArguments: ["-logFile", " C:\\Logs\\Editor.log ", ""] },
        window: { rememberState: true, fullscreen: true },
        locale: "zh-CN",
        appearance: { mode: "dark", sourceColor: "#112233", scheme: "expressive", motion: "full" },
        packages: { showPrerelease: true, hideLocalUserPackages: true, hiddenRepositoryIds: ["00000000-0000-4000-8000-000000000101"] }
    };
    current.directories = { projectDirectory: "D:\\Projects", backupDirectory: "E:\\Backups" };
    fixture.writer.seed(structuredClone(current));
    return fixture;
}

{
    const settings = customizedModel().current().settings;
    const cases: Array<{ name: string; update: SettingsUpdate; previous: SettingsUpdate; expected: SettingsGetResult["settings"] }> = [
        {
            name: "Projects",
            update: { projects: { autoDiscover: true }, paths: { projectDirectory: null } },
            previous: { projects: settings.projects, paths: { projectDirectory: settings.paths.projectDirectory } },
            expected: { ...settings, projects: { autoDiscover: true }, paths: { ...settings.paths, projectDirectory: null } }
        },
        {
            name: "Backups",
            update: { backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false }, paths: { backupDirectory: null } },
            previous: { backups: settings.backups, paths: { backupDirectory: settings.paths.backupDirectory } },
            expected: { ...settings, backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false }, paths: { ...settings.paths, backupDirectory: null } }
        },
        {
            name: "Default Unity launch arguments",
            update: { unity: { defaultLaunchArguments: null } },
            previous: { unity: { defaultLaunchArguments: settings.unity.defaultLaunchArguments } },
            expected: { ...settings, unity: { ...settings.unity, defaultLaunchArguments: null } }
        },
        {
            name: "Appearance",
            update: { locale: "system", appearance: { motion: "system" } },
            previous: { locale: settings.locale, appearance: { motion: settings.appearance.motion } },
            expected: { ...settings, locale: "system", appearance: { ...settings.appearance, motion: "system" } }
        },
        {
            name: "Window",
            update: { window: { rememberState: false, fullscreen: false } },
            previous: { window: settings.window },
            expected: { ...settings, window: { rememberState: false, fullscreen: false } }
        },
        {
            name: "Theme",
            update: { appearance: { mode: "system", sourceColor: null, scheme: "vibrant" } },
            previous: { appearance: { mode: settings.appearance.mode, sourceColor: settings.appearance.sourceColor, scheme: settings.appearance.scheme } },
            expected: { ...settings, appearance: { ...settings.appearance, mode: "system", sourceColor: null, scheme: "vibrant" } }
        }
    ];
    for (const scenario of cases) {
        const fixture = customizedModel();
        const restored = await fixture.writer.restoreDefaults(scenario.update, scenario.previous);
        equal(fixture.requests, [{ expectedRevision: 7, update: scenario.update }], `${scenario.name} restore writes only its card fields`);
        equal(restored.settings, scenario.expected, `${scenario.name} restore preserves every unrelated setting`);
        equal(restored.revision, 8, `${scenario.name} restore uses the current revision once`);
        equal(fixture.writer.getSnapshot().accepted, restored, `${scenario.name} restore accepts the actual Core result`);
    }
}

{
    const fixture = customizedModel();
    const started = deferred();
    const release = deferred();
    const original = fixture.client.settingsUpdate;
    let active = 0;
    let maximumActive = 0;
    let first = true;
    fixture.client.settingsUpdate = async (...arguments_) => {
        active += 1;
        maximumActive = Math.max(maximumActive, active);
        try {
            if (first) { first = false; started.resolve(); await release.promise; }
            return await original(...arguments_);
        } finally { active -= 1; }
    };
    fixture.writer.change({ backups: { compressionMode: "level3" } });
    await started.promise;
    const restored = fixture.writer.restoreDefaults(
        { locale: "system", appearance: { motion: "system" } },
        { locale: "zh-CN", appearance: { motion: "full" } }
    );
    fixture.writer.change({ projects: { autoDiscover: true } });
    equal(fixture.requests, [], "queued restore cannot start a write while another request is held");
    release.resolve();
    await restored;
    await idle(fixture.writer);
    equal(maximumActive, 1, "card restore shares the single settings write queue");
    equal(fixture.requests, [
        { expectedRevision: 7, update: { backups: { compressionMode: "level3" } } },
        { expectedRevision: 8, update: { projects: { autoDiscover: true } } },
        { expectedRevision: 9, update: { locale: "system", appearance: { motion: "system" } } }
    ], "card restore drains ordinary intents and uses their accepted CAS revision");
    equal(fixture.current().settings.backups.compressionMode, "level3", "card restore preserves the preceding backup edit");
    equal(fixture.current().settings.projects.autoDiscover, true, "card restore preserves an ordinary intent queued while waiting");
}

{
    const fixture = customizedModel();
    const update: SettingsUpdate = { backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false }, paths: { backupDirectory: null } };
    const previous: SettingsUpdate = { backups: structuredClone(fixture.current().settings.backups), paths: { backupDirectory: "E:\\Backups" } };
    const originalRead = fixture.client.settingsGet;
    const originalWrite = fixture.client.settingsUpdate;
    let reads = 0;
    fixture.client.settingsGet = async () => { reads += 1; return originalRead(); };
    fixture.client.settingsUpdate = async (...arguments_) => { await originalWrite(...arguments_); throw new Error("Restore reply lost after commit"); };
    const failed = await fixture.writer.restoreDefaults(update, previous).then(() => false, () => true);
    assert(failed, "lost restore response remains an explicit failed confirmation");
    equal(fixture.writer.getSnapshot().accepted!.revision, 7, "lost restore response cannot claim the new revision was accepted");
    equal(fixture.writer.getSnapshot().accepted!.settings.backups.compressionMode, "maximum", "unknown restore keeps the last acknowledged preference");
    const accepted = await fixture.writer.restoreDefaults(update, previous);
    equal(reads, 2, "explicit restore retry rereads Core before deciding whether to write");
    equal(fixture.requests.length, 1, "already committed restore is reconciled without another write");
    equal(accepted.revision, 8, "restore retry returns the actual committed revision");
    equal(accepted.settings.backups, { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false }, "restore retry accepts all committed card defaults");
}

for (const target of ["backup-compression", "unity-arguments"] as const) {
    const fixture = customizedModel();
    const previous: SettingsUpdate = target === "backup-compression"
        ? { backups: structuredClone(fixture.current().settings.backups) }
        : { unity: { defaultLaunchArguments: structuredClone(fixture.current().settings.unity.defaultLaunchArguments) } };
    const update: SettingsUpdate = target === "backup-compression"
        ? { backups: { compressionMode: "fast", includeVpmPackages: true, includeLibrary: false } }
        : { unity: { defaultLaunchArguments: null } };
    await fixture.client.settingsUpdate(7, target === "backup-compression"
        ? { backups: { compressionMode: "level5" } }
        : { unity: { defaultLaunchArguments: ["-batchmode", "-nographics"] } });
    const external = structuredClone(fixture.current());
    const conflict = await fixture.writer.restoreDefaults(update, previous).then(() => false, (error: unknown) => typeof error === "object" && error !== null && "code" in error && error.code === "revision_conflict");
    assert(conflict, `${target} changed during confirmation requires a new review`);
    equal(fixture.requests.length, 1, `${target} conflict never sends the restore patch`);
    equal(fixture.current(), external, `${target} conflict preserves external authority`);
    equal(fixture.writer.getSnapshot().accepted, external, `${target} conflict shows the freshly read authority`);
}

{
    const fixture = customizedModel();
    const previous: SettingsUpdate = { appearance: { mode: "dark", sourceColor: "#112233", scheme: "expressive" } };
    await fixture.client.settingsUpdate(7, { appearance: { motion: "reduced" }, backups: { compressionMode: "level7" } });
    const restored = await fixture.writer.restoreDefaults({ appearance: { mode: "system", sourceColor: null, scheme: "vibrant" } }, previous);
    equal(fixture.requests[1], { expectedRevision: 8, update: { appearance: { mode: "system", sourceColor: null, scheme: "vibrant" } } }, "unrelated concurrent edits permit a narrow theme restore at the latest revision");
    equal(restored.settings.appearance, { mode: "system", sourceColor: null, scheme: "vibrant", motion: "reduced" }, "theme restore preserves an externally changed sibling field");
    equal(restored.settings.backups.compressionMode, "level7", "theme restore preserves another card's external change");
    equal(restored.settings.unity.hubPath, "C:\\Tools\\Unity Hub.exe", "card restore never changes Unity Hub");
}

{
    const fixture = model();
    equal(fixture.writer.getSnapshot().accepted!.settings.window.fullscreen, false, "fullscreen starts disabled");
    fixture.writer.change({ window: { fullscreen: false } });
    await idle(fixture.writer);
    equal(fixture.requests, [], "the default fullscreen state needs no write");
    const started = deferred();
    const release = deferred();
    const original = fixture.client.settingsUpdate;
    let first = true;
    fixture.client.settingsUpdate = async (...arguments_) => {
        if (first) { first = false; started.resolve(); await release.promise; }
        return original(...arguments_);
    };
    fixture.writer.change({ window: { fullscreen: true } });
    await started.promise;
    fixture.writer.change({ window: { fullscreen: false, rememberState: true } });
    release.resolve();
    await idle(fixture.writer);
    equal(fixture.requests, [
        { expectedRevision: 7, update: { window: { fullscreen: true } } },
        { expectedRevision: 8, update: { window: { fullscreen: false, rememberState: true } } }
    ], "fullscreen toggle-back and remembering share the serialized window intent");
    equal(fixture.current().settings.window, { rememberState: true, fullscreen: false }, "latest fullscreen and sibling preference both survive");
}

{
    const fixture = model();
    const original = fixture.client.settingsUpdate;
    fixture.client.settingsUpdate = async (...arguments_) => {
        await original(...arguments_);
        throw new Error("Fullscreen response lost after commit");
    };
    fixture.writer.change({ window: { fullscreen: true } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "an unknown fullscreen result remains explicit");
    equal(fixture.writer.getSnapshot().accepted!.settings.window.fullscreen, false, "unacknowledged fullscreen intent is not accepted authority");
    equal(fixture.writer.getSnapshot().settings!.window.fullscreen, true, "fullscreen intent remains available for Retry");
    await fixture.writer.retry();
    equal(fixture.requests.length, 1, "fullscreen Retry checks Core before avoiding a duplicate committed write");
    equal(fixture.writer.getSnapshot().accepted!.settings.window, { rememberState: false, fullscreen: true }, "reconciled fullscreen keeps remembering unchanged");
}

{
    const fixture = model();
    await fixture.client.settingsUpdate(7, { window: { rememberState: true } });
    fixture.writer.change({ window: { fullscreen: true } });
    await idle(fixture.writer);
    assert(fixture.writer.getSnapshot().failed, "a competing window preference must surface its CAS conflict");
    await fixture.writer.retry();
    equal(fixture.requests.at(-1), { expectedRevision: 8, update: { window: { fullscreen: true } } }, "fullscreen Retry patches only the requested field at the current revision");
    equal(fixture.current().settings.window, { rememberState: true, fullscreen: true }, "fullscreen cannot overwrite an externally enabled size memory preference");
}

console.log("Settings autosave coordination passed (43 scenarios).");
