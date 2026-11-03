import type { RpcError } from "@alcomd/sdk";
import { WindowStateMemory, windowGeometry, type WindowGeometry, type WindowStateHost } from "./window-state.ts";

function equal(actual: unknown, expected: unknown, name: string): void {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${name}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
}
class ViewStorage implements Storage {
    data = new Map<string, string>();
    blocked = false;
    get length() { return this.data.size; }
    clear() { this.data.clear(); }
    key(index: number) { return [...this.data.keys()][index] ?? null; }
    getItem(key: string) { if (this.blocked) throw new Error("blocked"); return this.data.get(key) ?? null; }
    setItem(key: string, value: string) { if (this.blocked) throw new Error("blocked"); this.data.set(key, value); }
    removeItem(key: string) { if (this.blocked) throw new Error("blocked"); this.data.delete(key); }
}
function fixture(saved?: unknown) {
    const storage = new ViewStorage();
    const key = "test-window";
    if (saved !== undefined) storage.setItem(key, JSON.stringify(saved));
    const calls: unknown[] = [];
    const errors: (RpcError | undefined)[] = [];
    let changed: (value: unknown) => void = () => {};
    let stopCount = 0;
    const host: WindowStateHost = {
        configure: async (enabled, geometry, fullscreen) => { calls.push([enabled, geometry, fullscreen]); return enabled ? geometry ?? [1400, 800, false] : undefined; },
        listen: async (listener) => { changed = listener; return () => { stopCount += 1; }; }
    };
    const memory = new WindowStateMemory(host, () => storage, key, (error) => errors.push(error));
    return { storage, key, calls, errors, host, memory, emit: (value: unknown) => changed(value), stopCount: () => stopCount };
}

{
    const f = fixture([1200, 700, true]);
    await f.memory.apply(false, false);
    f.emit([1500, 900, false]);
    await f.memory.apply(false, false);
    equal(f.calls, [[false, undefined, false]], "disabled only configures once");
    equal(f.storage.getItem(f.key), null, "disabled clears cache and never records");
}
{
    const f = fixture([1200, 700, true]);
    await f.memory.apply(true, false);
    f.emit([1250, 720, false]);
    await f.memory.apply(true, false);
    equal(f.calls, [[true, [1200, 700, true], false]], "restore once across settings rereads");
    equal(JSON.parse(f.storage.getItem(f.key)!), [1250, 720, false], "record accepted native geometry");
    await f.memory.apply(false, false);
    await f.memory.apply(true, false);
    equal(f.calls, [[true, [1200, 700, true], false], [false, undefined, false], [true, undefined, false]], "reenable keeps current window");
}
{
    for (const invalid of [null, {}, [0, 0, false], [319, 800, true], [1400, 479, true], [40000, 800, false], [1400, 800, "true"], [1400, 800, false, 1]]) {
        const f = fixture(invalid);
        await f.memory.apply(true, false);
        equal(f.calls, [[true, undefined, false]], "invalid cache uses defaults");
    }
    equal(windowGeometry([Number.NaN, 800, false]), undefined, "nonfinite width");
    equal(windowGeometry([1400, Infinity, false]), undefined, "nonfinite height");
}
{
    const f = fixture();
    f.storage.data.set(f.key, "malformed");
    await f.memory.apply(true, false);
    equal(f.calls, [[true, undefined, false]], "malformed JSON uses defaults");
    f.emit([1, 0, false]);
    equal(JSON.parse(f.storage.getItem(f.key)!), [1400, 800, false], "invalid native event does not replace normal size");
}
{
    const f = fixture([1200, 700, true]);
    await f.memory.startupReadFailed();
    equal(JSON.parse(f.storage.getItem(f.key)!), [1200, 700, true], "unavailable Core does not discard unknown preference cache");
    await f.memory.apply(true, false);
    equal(f.calls, [[false, undefined, false], [true, undefined, false]], "reconnect does not restore late");
}
{
    const f = fixture(); f.storage.blocked = true;
    await f.memory.apply(false, false);
    equal(f.errors.filter(Boolean), [], "unused unavailable cache has no warning");
    await f.memory.apply(true, false);
    f.emit([1200, 700, false]);
    equal(f.errors.filter(Boolean).length, 1, "cache write failure is bounded feedback");
    equal(f.errors.filter(Boolean)[0]?.code, "window_state_cache_unavailable", "accurate cache failure");
}
{
    const f = fixture([1200, 700, true]);
    f.host.configure = async () => { throw new Error("private native detail"); };
    await f.memory.apply(true, false);
    equal(JSON.parse(f.storage.getItem(f.key)!), [1200, 700, true], "restore failure preserves saved geometry");
    equal(f.errors.filter(Boolean), [{ code: "window_state_unavailable", message: "" }], "native error is safe and explicit");
}
{
    const f = fixture();
    await f.memory.apply(true, false); f.memory.dispose(); await Promise.resolve();
    f.emit([1200, 700, true]);
    equal(f.stopCount(), 1, "dispose removes listener");
    equal(JSON.parse(f.storage.getItem(f.key)!), [1400, 800, false], "disposed bridge cannot write");
}
{
    const f = fixture([1200, 700, true]);
    const enabled = f.memory.apply(true, false);
    const disabled = f.memory.apply(false, false);
    await Promise.all([enabled, disabled]);
    equal(f.calls, [[false, undefined, false]], "superseded queued toggle cannot restore");
    equal(f.storage.getItem(f.key), null, "disabled cache remains cleared");
}
{
    const f = fixture();
    let finish: ((value: WindowGeometry) => void) | undefined;
    f.host.configure = async (enabled) => enabled ? new Promise<WindowGeometry>((resolve) => { finish = resolve; }) : undefined;
    const first = f.memory.apply(true, false);
    while (finish === undefined) await Promise.resolve();
    const second = f.memory.apply(false, false);
    finish([1200, 700, true]); await Promise.all([first, second]);
    equal(f.storage.getItem(f.key), null, "late native response cannot repopulate disabled cache");
}
{
    const f = fixture(); const original = f.host.listen; let attempts = 0;
    f.host.listen = async (...args) => { if (++attempts === 1) throw new Error("listen failed"); return original(...args); };
    await f.memory.apply(true, false); await f.memory.apply(false, false); await f.memory.apply(true, false);
    f.emit([1200, 700, false]);
    equal(attempts, 2, "reenable can recover failed listener");
    equal(JSON.parse(f.storage.getItem(f.key)!), [1200, 700, false], "recovered listener records state");
}
{
    const f = fixture([1200, 700, true]);
    await f.memory.apply(false, true);
    equal(f.calls, [[false, undefined, true]], "fullscreen applies even with geometry memory disabled");
    equal(f.storage.getItem(f.key), null, "fullscreen preference is never cached in disposable geometry");
    await f.memory.apply(false, false);
    equal(f.calls.at(-1), [false, undefined, false], "leaving fullscreen applies while memory is disabled");
}
{
    const f = fixture([1200, 700, true]);
    await f.memory.apply(true, true);
    await f.memory.apply(true, false);
    equal(f.calls, [[true, [1200, 700, true], true], [true, undefined, false]], "fullscreen toggle never restores cached geometry a second time");
}
{
    const f = fixture();
    const original = f.host.configure;
    f.host.configure = async () => { throw new Error("private native detail"); };
    await f.memory.apply(false, true);
    equal(f.errors.filter(Boolean).at(-1)?.code, "window_state_unavailable", "fullscreen failure is explicit");
    f.host.configure = original;
    await f.memory.retry();
    equal(f.calls, [[false, undefined, true]], "native retry applies accepted fullscreen without another settings write");
}
console.log("Window state coordination passed (14 scenarios).");
