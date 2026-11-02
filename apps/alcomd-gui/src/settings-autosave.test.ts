import type { SettingsGetResult, SettingsUpdate } from "./core-models.ts";
import { SettingsAutosave, StorageDirectoryConflictError, UnityArgumentsConflictError, UnityHubConflictError } from "./settings-autosave.ts";

const initial: SettingsGetResult = {
    configSchema: 6, revision: 7,
    directories: { projectDirectory: "C:\\Projects", backupDirectory: "C:\\Backups" },
    unityHub: { status: "not_found", source: "none", path: null, configuredPathStatus: "unset" },
    settings: {
        paths: { projectDirectory: null, backupDirectory: null },
        unity: { hubPath: null, defaultLaunchArguments: null },
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
                configSchema: 6, revision: expectedRevision + 1,
                directories: {
                    projectDirectory: update.paths?.projectDirectory ?? current.directories.projectDirectory,
                    backupDirectory: update.paths?.backupDirectory ?? current.directories.backupDirectory
                },
                unityHub: current.unityHub,
                settings: {
                    ...current.settings, ...update,
                    appearance: { ...current.settings.appearance, ...update.appearance },
                    packages: { ...current.settings.packages, ...update.packages },
                    paths: { ...current.settings.paths, ...update.paths },
                    unity: { ...current.settings.unity, ...update.unity }
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
    await fixture.writer.saveTheme({ mode: "dark", scheme: "neutral", sourceColor: "#6CB6FF" });
    await idle(fixture.writer);
    equal(fixture.requests.map((request) => request.expectedRevision), [7, 8], "theme and ordinary writes share one order");
    equal(Object.keys(fixture.requests[1]!.update.appearance!).sort(), ["mode", "scheme", "sourceColor"], "explicit theme writes only its fields");
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
    const theme = fixture.writer.saveTheme({ mode: "dark", sourceColor: null, scheme: "neutral" }).then(() => false, () => true);
    fixture.writer.change({ appearance: { motion: "full" } });
    await started.promise;
    fixture.writer.stop();
    release.resolve();
    assert(await theme, "stopped theme task must reject after draining an in-flight ordinary save");
    equal(fixture.requests.length, 1, "stop must prevent the new theme RPC across the await boundary");
    equal(fixture.applied.length, 0, "stopped theme task must not apply old UI state");
}

{
    const fixture = model();
    fixture.writer.change({ packages: { showPrerelease: true } });
    await fixture.writer.saveUnityHub("C:\\Tools\\Unity Hub.exe");
    await fixture.writer.saveTheme({ mode: "dark", sourceColor: null, scheme: "neutral" });
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
    await fixture.writer.saveTheme({ mode: "dark", sourceColor: null, scheme: "neutral" });
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

console.log("Settings autosave coordination passed (24 scenarios).");
