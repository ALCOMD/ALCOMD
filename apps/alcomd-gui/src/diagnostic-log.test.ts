import type { DiagnosticLogItem, DiagnosticLogReadParams, DiagnosticLogReadResult } from "@alcomd/sdk";
import { DiagnosticLogFeed, diagnosticLogKey, type DiagnosticLogSource } from "./diagnostic-log.ts";

function equal(actual: unknown, expected: unknown, name: string) {
    if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${name}: ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`);
}
const tick = async () => { for (let index = 0; index < 5; index += 1) await Promise.resolve(); };
const CORE_A = "10000000-0000-4000-8000-000000000001";
const CORE_B = "10000000-0000-4000-8000-000000000002";
const log = (sequence: number, level: DiagnosticLogItem["level"] = "error", notify = true, instanceId: string = CORE_A): DiagnosticLogItem =>
    ({ instanceId, sequence, occurredAtMs: sequence, level, code: `diagnostic.${level}`, diagnosticId: `20000000-0000-4000-8000-${String(sequence).padStart(12, "0")}`, notify });
const result = (sequence: number, items: DiagnosticLogItem[] = [], reset = false, gap = false, instanceId: string = CORE_A): DiagnosticLogReadResult =>
    ({ items, nextCursor: { instanceId, sequence }, reset, gap });

function fixture() {
    const requests: DiagnosticLogReadParams[] = [];
    const pending: Array<{ resolve(value: DiagnosticLogReadResult): void; reject(error: unknown): void }> = [];
    let cancels = 0;
    let resumeRetry: (() => void) | undefined;
    const source: DiagnosticLogSource = {
        diagnosticLogRead: (params) => { requests.push(structuredClone(params)); return new Promise((resolve, reject) => pending.push({ resolve, reject })); },
        diagnosticLogCancel: async () => { cancels += 1; pending.splice(0).forEach((read) => read.reject({ code: "daemon_unavailable" })); }
    };
    const feed = new DiagnosticLogFeed(source, (signal) => new Promise<void>((resolve) => {
        const finish = () => { signal.removeEventListener("abort", finish); resumeRetry = undefined; resolve(); };
        resumeRetry = finish;
        signal.addEventListener("abort", finish, { once: true });
    }));
    return {
        feed, requests, pending,
        respond: async (value: DiagnosticLogReadResult) => { pending.shift()!.resolve(value); await tick(); },
        fail: async () => { pending.shift()!.reject({ code: "daemon_unavailable", message: "secret" }); await tick(); },
        retry: async () => { resumeRetry!(); await tick(); },
        cancelCount: () => cancels
    };
}

{
    const f = fixture();
    f.feed.start();
    f.feed.start();
    equal(f.pending.length, 1, "only one read is in flight");
    await f.respond(result(2, [log(1), log(2, "warning")]));
    equal(f.feed.getSnapshot().items.map(diagnosticLogKey), [`${CORE_A}:1`, `${CORE_A}:2`], "initial history remains visible");
    equal(f.feed.getSnapshot().notifications, [], "initial history is never replayed as notifications");
    equal(f.requests[1], { cursor: { instanceId: CORE_A, sequence: 2 }, limit: 200, waitMs: 20000 }, "subsequent read waits from the consumed cursor");
    await f.respond(result(8, [log(3), log(4, "warning", false), log(5, "warning"), log(6, "info"), log(7, "debug"), log(8, "trace")]));
    equal(f.feed.getSnapshot().notifications.map(diagnosticLogKey), [`${CORE_A}:3`, `${CORE_A}:5`], "only explicit error/warning notifications are shown");
    f.feed.dismiss(`${CORE_A}:3`);
    await f.respond(result(8, [log(3), log(5), log(5)]));
    equal(f.feed.getSnapshot().notifications.map(diagnosticLogKey), [`${CORE_A}:5`], "duplicates and dismissed records cannot reappear");
    equal(f.feed.getSnapshot().items.length, 8, "dismissal does not erase a log record");
    await f.feed.stop();
    equal(f.cancelCount(), 1, "stop cancels the dedicated read");
}
{
    const f = fixture();
    f.feed.start();
    await f.respond(result(0));
    await f.respond(result(2, [log(1), log(2)]));
    await f.fail();
    equal(f.feed.getSnapshot().error, { code: "daemon_unavailable", message: "" }, "read failure keeps only safe metadata");
    equal(f.feed.getSnapshot().items.length, 2, "read failure retains loaded records");
    equal(f.feed.getSnapshot().notifications.length, 2, "read failure does not create a recursive notification");
    await f.retry();
    equal(f.requests.at(-1)?.cursor, { instanceId: CORE_A, sequence: 2 }, "reconnect resumes the previous cursor");
    await f.respond(result(3, [log(2), log(3)]));
    equal(f.feed.getSnapshot().notifications.map(diagnosticLogKey), [`${CORE_A}:1`, `${CORE_A}:2`, `${CORE_A}:3`], "reconnect only publishes new records");
    equal(f.feed.getSnapshot().error, undefined, "successful resume clears the read failure");
    await f.feed.stop();
}
{
    const f = fixture();
    f.feed.start();
    await f.respond(result(4, [log(4)]));
    await f.respond(result(5, [log(5)]));
    await f.respond(result(10, [log(9), log(10)], false, true));
    equal(f.feed.getSnapshot().gap, true, "truncated history is not presented as continuous");
    equal(f.feed.getSnapshot().notifications.map(diagnosticLogKey), [`${CORE_A}:5`], "a gap keeps acknowledged context without replaying an unknown backlog");
    await f.respond(result(2, [log(1, "error", true, CORE_B), log(2, "warning", true, CORE_B)], true, false, CORE_B));
    equal(f.feed.getSnapshot().items.map(diagnosticLogKey), [`${CORE_B}:1`, `${CORE_B}:2`], "restart replaces the previous instance view");
    equal(f.feed.getSnapshot().notifications, [], "restart establishes a fresh baseline without replay");
    equal(f.feed.getSnapshot().gap, false, "restart has an independent continuity state");
    await f.respond(result(3, [log(3, "error", true, CORE_B)], false, false, CORE_B));
    equal(f.feed.getSnapshot().notifications.map(diagnosticLogKey), [`${CORE_B}:3`], "new instance publishes only after its baseline");
    await f.feed.stop();
}
{
    const f = fixture();
    f.feed.start();
    await f.respond(result(0));
    const stale = f.pending.shift()!;
    await f.feed.stop();
    stale.resolve(result(1, [log(1)]));
    await tick();
    equal(f.feed.getSnapshot().items, [], "a late response after stop cannot change the cache or publish");
    f.feed.start();
    equal(f.requests.at(-1)?.cursor, { instanceId: CORE_A, sequence: 0 }, "restarting the same feed preserves its cursor");
    await f.respond(result(1, [log(1)]));
    equal(f.feed.getSnapshot().notifications.length, 1, "resuming the same instance does not discard new logs");
    await f.feed.stop();
}
{
    const f = fixture();
    f.feed.start();
    await f.respond(result(0));
    for (let page = 0; page < 6; page += 1) {
        await f.respond(result((page + 1) * 200, Array.from({ length: 200 }, (_, index) => log(page * 200 + index + 1))));
    }
    equal(f.feed.getSnapshot().items.length, 1024, "the GUI cache is bounded");
    equal(f.feed.getSnapshot().notifications.length, 1024, "unacknowledged popup storage is bounded");
    equal(f.feed.getSnapshot().items[0]?.sequence, 177, "bounded views retain the newest records");
    await f.feed.stop();
}
console.log("Diagnostic log feed checks passed.");
