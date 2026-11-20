import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { PassThrough, Readable } from "node:stream";
import test from "node:test";
import { AlcomdSdkError, FrameReader, FramedBridge } from "../src/node-runtime.mjs";

function frame(json) {
    const bytes = Buffer.from(json);
    const header = Buffer.alloc(4);
    header.writeUInt32LE(bytes.length);
    return Buffer.concat([header, bytes]);
}

function chunkedReader(...chunks) {
    return new FrameReader(Readable.from(chunks));
}

function connection() {
    const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout: new PassThrough() });
    const reader = new FrameReader(child.stdin);
    return {
        client: new FramedBridge(child),
        request: () => reader.read(),
        respond: (data) => child.stdout.write(frame(JSON.stringify(data)))
    };
}

function kind(expectedKind, outcomeUnknown = false) {
    return (error) => error instanceof AlcomdSdkError && error.kind === expectedKind
        && error.outcomeUnknown === outcomeUnknown;
}

test("reads coalesced and fragmented UTF-8 frames at a safe integer boundary", async () => {
    const first = frame('{"data":"多字节","sequence":9007199254740991}');
    const second = frame('{"result":{}}');
    const reader = chunkedReader(first.subarray(0, 2), first.subarray(2, 14), Buffer.concat([first.subarray(14), second]));
    assert.deepEqual(await reader.read(), { data: "多字节", sequence: Number.MAX_SAFE_INTEGER });
    assert.deepEqual(await reader.read(), { result: {} });
    await assert.rejects(reader.read(), kind("transport"));
});

test("rejects invalid lengths, truncated frames, unsafe integers and invalid UTF-8", async () => {
    const oversized = Buffer.alloc(4);
    oversized.writeUInt32LE(4 * 1024 * 1024 + 1);
    for (const input of [
        Buffer.alloc(4),
        oversized,
        Buffer.from([3, 0]),
        Buffer.from([3, 0, 0, 0, 123]),
        frame('{"revision":9007199254740993}'),
        frame('{"revision":-9223372036854775808}'),
        Buffer.from([1, 0, 0, 0, 0xc0])
    ]) {
        await assert.rejects(chunkedReader(input).read(), kind("protocol"));
    }
});

test("retains structured remote errors and reuses a completed command connection", async () => {
    const { client, request, respond } = connection();
    try {
        const first = client.exchange("settings.update", { idempotencyKey: "keep-this" }, {}, true);
        assert.deepEqual(await request(), { id: "1", method: "settings.update", params: { idempotencyKey: "keep-this" } });
        respond({ id: "1", error: { kind: "remote", code: "revision_conflict", message: "Conflict.",
            diagnosticId: "diagnostic-a", data: { expectedRevision: 5 } } });
        await assert.rejects(first, (error) => kind("remote")(error) && error.code === "revision_conflict"
            && error.diagnosticId === "diagnostic-a" && error.data.expectedRevision === 5);
        const next = client.exchange("system.status", {});
        assert.equal((await request()).id, "2");
        respond({ id: "2", result: { state: "ready" } });
        assert.deepEqual(await next, { state: "ready" });
    } finally {
        client.close();
    }
});

test("does not dispatch invalid or already cancelled writes", async () => {
    const { client, request, respond } = connection();
    try {
        await assert.rejects(client.exchange("settings.update", { revision: 2 ** 53 }, {}, true), kind("invalid_request"));
        await assert.rejects(client.exchange("settings.update", {}, { signal: AbortSignal.abort() }, true), kind("cancelled"));
        const status = client.exchange("system.status", {});
        const sent = await request();
        assert.equal(sent.method, "system.status");
        respond({ id: sent.id, result: {} });
        await status;
    } finally {
        client.close();
    }
});

test("interrupted writes close the connection and report an unknown outcome", async () => {
    const { client, request } = connection();
    const cancellation = new AbortController();
    const write = client.exchange("packages.apply", { idempotencyKey: "keep-this" }, { signal: cancellation.signal }, true);
    assert.equal((await request()).params.idempotencyKey, "keep-this");
    await assert.rejects(client.exchange("system.status", {}), kind("invalid_request"));
    cancellation.abort();
    await assert.rejects(write, kind("cancelled", true));
    await assert.rejects(client.exchange("system.status", {}), kind("transport"));
});

test("a deadline does not retry or re-key a dispatched write", async () => {
    const { client, request } = connection();
    const write = client.exchange("packages.apply", { idempotencyKey: "same-key" }, { timeoutMs: 30 }, true);
    assert.equal((await request()).params.idempotencyKey, "same-key");
    await assert.rejects(write, kind("timeout", true));
    await assert.rejects(client.exchange("system.status", {}), kind("transport"));
});

test("a mismatched response or competing result and error fields invalidate the connection", async () => {
    for (const response of [
        { id: "other", result: {} },
        { id: "1", error: null, result: {} }
    ]) {
        const { client, request, respond } = connection();
        const pending = client.exchange("system.status", {});
        await request();
        respond(response);
        await assert.rejects(pending, kind("protocol"));
        await assert.rejects(client.exchange("system.status", {}), kind("transport"));
    }
});
