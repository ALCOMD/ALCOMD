import { expect, test } from "@playwright/test";
import { createGuiWriteAttempt } from "../../src/gui-write-attempt";
import type { GuiRpcClient } from "../../src/rpc";

test("a view retries only its unacknowledged write with the same arguments and key", async () => {
    type Args = Parameters<GuiRpcClient["repositoryMove"]>;
    const calls: Args[] = [];
    let failure: string | undefined = "daemon_unavailable";
    const client = {
        async repositoryMove(...args: Args) {
            calls.push(args);
            if (failure) throw { code: failure };
            return { changed: true, replayed: false };
        },
    };
    const write = createGuiWriteAttempt(client);
    const move = () => write("repositoryMove", "first", "second", "order");
    await expect(move()).rejects.toMatchObject({ code: failure });
    failure = undefined;
    await move();
    expect(calls[1]).toEqual(calls[0]);
    await move();
    expect(calls[2]![3]).not.toEqual(calls[1]![3]);

    failure = "internal_error";
    await expect(move()).rejects.toMatchObject({ code: failure });
    await expect(write("repositoryMove", "first", null, "order")).rejects.toMatchObject({ code: failure });
    expect(calls[4]![3]).not.toEqual(calls[3]![3]);
    const otherView = createGuiWriteAttempt(client);
    failure = "revision_conflict";
    await expect(otherView("repositoryMove", "first", null, "order")).rejects.toMatchObject({ code: failure });
    expect(calls[5]![3]).not.toEqual(calls[4]![3]);
    failure = undefined;
    await otherView("repositoryMove", "first", null, "order");
    expect(calls[6]![3]).not.toEqual(calls[5]![3]);
    expect(calls.every((args) => typeof args[3] === "string")).toBe(true);
});
