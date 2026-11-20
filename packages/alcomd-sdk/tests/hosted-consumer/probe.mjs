import assert from "node:assert/strict";
import { RPC_VERSION, SDK_METHODS } from "@alcomd/sdk";
import { AlcomdSdkError, connect } from "@alcomd/sdk/node";

const mode = process.argv[2];
assert.equal(RPC_VERSION, 1);
assert.equal(SDK_METHODS["system.status"], "read");
assert.equal(SDK_METHODS["events.subscribe"], "stream");
if (mode === "unavailable") {
    await assert.rejects(async () => {
        const client = await connect();
        client.close();
    }, (error) => error instanceof AlcomdSdkError && error.kind === "unavailable" && !error.outcomeUnknown);
} else {
    assert(["hold", "probe"].includes(mode));
    const client = await connect();
    let events;
    try {
        const status = await client.request("system.status", {});
        const page = await client.request("projects.list", { limit: 1 });
        events = await client.subscribeEvents();
        console.log(JSON.stringify({
            hello: client.hello,
            status,
            instanceId: events.ready.instanceId,
            projectCount: page.projects.length
        }));
        if (mode === "hold") {
            await new Promise((release) => {
                process.stdin.once("end", release);
                process.stdin.resume();
            });
        }
    } finally {
        events?.close();
        client.close();
    }
}
