import type { DiagnosticLogHealth, DiagnosticLogItem, DiagnosticLogReadParams, DiagnosticLogReadResult, DiagnosticLogHistoryResult } from "@alcomd/sdk";
import { DiagnosticLogFeed, diagnosticLogKey, type DiagnosticLogSource } from "./diagnostic-log.ts";
function equal(actual: unknown, expected: unknown, name: string) { if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${name}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`); }
const tick = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
const health: DiagnosticLogHealth = { sourceId: "core", sessionId: "session", revision: 0, state: "ready", queueDropped: 0, oversizeRejected: 0, writeFailed: 0, rotationFailed: 0, flushFailed: 0, hasGap: false };
const log = (offset: number, sourceId = "core"): DiagnosticLogItem => ({ sourceId, sessionId: "session", segmentId: "segment", offset, record: { timestamp: new Date(offset * 1000).toISOString(), level: "ERROR", target: "alcomd::test", fields: { message: "Real failure text", path: "C:\\项目A\\Packages\\vpm-manifest.json", error: "outer failure\ncaused by: original parser failure" } } });
const result = (items: DiagnosticLogItem[] = [], nextCursor = "B", baseline = false, currentHealth = health): DiagnosticLogReadResult => ({ items, nextCursor, baseline, health: currentHealth, issues: [], hasMore: false, scannedBytes: items.length * 100 });
function fixture() {
    const requests: DiagnosticLogReadParams[] = [];
    const reads: Array<{ resolve(page: DiagnosticLogReadResult): void; reject(error: unknown): void }> = [];
    const histories: Array<{ resolve(page: DiagnosticLogHistoryResult): void; reject(error: unknown): void }> = [];
    let cancels = 0;
    const source: DiagnosticLogSource = {
        read: (params) => { requests.push(params); return new Promise((resolve, reject) => reads.push({ resolve, reject })); },
        history: () => new Promise((resolve, reject) => histories.push({ resolve, reject })),
        cancel: async () => { cancels++; reads.splice(0).forEach((item) => item.reject({ code: "daemon_unavailable" })); }
    };
    const feed = new DiagnosticLogFeed(source);
    return { feed, requests, reads, histories, get cancels() { return cancels; },
        respond: async (page: DiagnosticLogReadResult) => { reads.shift()!.resolve(page); await tick(); },
        history: async (items: DiagnosticLogItem[]) => { histories.shift()!.resolve({ items, health, issues: [], hasMore: false, scannedBytes: 100 }); await tick(); }
    };
}
{
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true));
    equal(f.requests.map((x) => x.cursor), [undefined, "B"], "baseline precedes the only pending follow");
    equal(f.requests[1]?.follow, true, "follow waits on real changes");
    await tick(); equal(f.requests.length, 2, "idle does not manufacture reads");
    const entry = log(1); await f.history([entry]); await f.respond(result([entry], "C"));
    equal(f.feed.getSnapshot().items.length, 1, "history/live duplicates share physical identity");
    equal(f.feed.getSnapshot().items[0]?.record, entry.record, "path, causes and body remain the source JSON");
    await f.feed.stop(); equal(f.cancels, 1, "unmount cancels the pending native read");
}
{
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true)); await f.history([log(1)]);
    await f.respond(result([], "B-health", false, { ...health, revision: 1, state: "degraded", queueDropped: 7, hasGap: true }));
    equal(f.feed.getSnapshot().health?.queueDropped, 7, "health-only response is accepted without a record");
    equal(f.feed.getSnapshot().items.length, 1, "health does not invent failure log rows"); await f.feed.stop();
}
{
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true)); await f.history([log(1)]);
    f.reads.shift()!.reject({ code: "daemon_unavailable", message: "secret remote message" }); await tick();
    const count = f.requests.length; await tick(); equal(f.requests.length, count, "ordinary failure starts no periodic retry");
    equal(f.feed.getSnapshot().error?.message, "", "untrusted error text is not displayed");
    equal(f.feed.getSnapshot().items.length, 1, "failure retains source entries"); await f.feed.stop();
}
{
    const f = fixture(); f.feed.start(); const late = f.reads[0]!; await f.feed.stop(); late.resolve(result([log(1)], "late")); await tick();
    equal(f.feed.getSnapshot().items.length, 0, "late response after cancellation cannot insert records");
}
{
    equal(diagnosticLogKey(log(1, "core")) === diagnosticLogKey(log(1, "gui")), false, "source is part of physical identity");
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true)); await f.history([]);
    await f.respond(result(Array.from({ length: 1200 }, (_, n) => log(n)), "C"));
    equal(f.feed.getSnapshot().items.length, 1000, "each source owns half the shared row budget");
    equal(f.feed.getSnapshot().limited, true, "cache eviction is disclosed"); await f.feed.stop();
}
{
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true));
    await f.respond(result([], "C", false, { ...health, revision: 2, state: "degraded", writeFailed: 1, hasGap: true }));
    await f.history([log(1)]);
    equal(f.feed.getSnapshot().health?.revision, 2, "late history health cannot replace a newer source revision");
    equal(f.feed.getSnapshot().health?.writeFailed, 1, "newer failure counts survive an older history response");
    await f.feed.stop();
}
{
    const f = fixture(); f.feed.start(); await f.respond(result([], "B", true));
    f.reads.shift()!.reject({ code: "daemon_unavailable" }); await tick();
    await f.feed.retry(); await f.respond(result([], "C", true));
    equal(f.histories.length, 2, "retry starts a new history read while an old generation is pending");
    const fresh = f.histories.pop()!;
    fresh.resolve({ items: [log(2)], health, issues: [], hasMore: false, scannedBytes: 100 }); await tick();
    await f.history([log(1)]);
    equal(f.feed.getSnapshot().items.map((item) => item.offset), [2], "late history from the cancelled generation cannot insert entries");
    equal(f.feed.getSnapshot().loading, false, "the new generation finishes loading");
    await f.feed.stop();
}
console.log("Technical log feed checks passed (7 scenarios).");
