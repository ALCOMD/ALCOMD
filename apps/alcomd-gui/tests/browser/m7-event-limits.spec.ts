import { expect, test, type Page } from "@playwright/test";

async function open(page: Page) {
    await page.clock.install();
    await page.goto("/tests/browser/utility-workspace.html?eventLimits=1");
    await expect(page.getByRole("heading", { name: "Event coordinator limits evidence", exact: true })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.eventLimits.coordinator.getSnapshot().kind)).toBe("ready");
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
}

test("the shared RPC limiter reserves the fourth read for a terminal change while ordinary reads remain bounded to three", async ({ page }) => {
    await open(page);
    await page.evaluate(() => {
        for (let index = 1; index <= 5; index++) window.eventLimits.seed(index);
        for (let index = 1; index <= 4; index++) void window.eventLimits.read(index);
    });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(3);
    expect(await page.evaluate(() => window.eventLimits.reads)).toEqual(await page.evaluate(() => [1, 2, 3].map(window.eventLimits.id)));
    await page.evaluate(() => window.eventLimits.complete(5));
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    await page.evaluate(() => { void window.eventLimits.read(5); });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(4);
    expect(await page.evaluate(() => window.eventLimits.reads.at(-1))).toBe(await page.evaluate(() => window.eventLimits.id(5)));
    expect(await page.evaluate(() => window.eventLimits.activeNormal)).toBe(3);
    await page.evaluate(() => window.eventLimits.release(5));
    await expect.poll(() => page.evaluate(() => window.eventLimits.active)).toBe(3);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(4);
    await page.evaluate(() => window.eventLimits.release(1));
    await expect.poll(() => page.evaluate(() => window.eventLimits.reads.length)).toBe(5);
    expect(await page.evaluate(() => window.eventLimits.maxActive)).toBe(4);
    expect(await page.evaluate(() => window.eventLimits.maxNormal)).toBe(3);
    await page.evaluate(() => window.eventLimits.releaseAll());
    await expect.poll(() => page.evaluate(() => window.eventLimits.active)).toBe(0);
    const reads = await page.evaluate(() => window.eventLimits.reads.length);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(reads);
});

test("overflow of 512 dirty operation keys promotes to a category read and converges to every authoritative revision", async ({ page }) => {
    await open(page);
    await page.evaluate(() => {
        window.eventLimits.observe(513);
        window.eventLimits.progressAll(513);
    });
    await expect.poll(() => page.evaluate(() => window.coreEvents.requests.filter((item) => item.method === "ack").length)).toBe(6);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(0);
    await page.evaluate(() => window.eventLimits.registerOverflowRecovery());
    await page.clock.runFor(1_000);
    await expect.poll(() => page.evaluate(() => window.eventLimits.categoryResult.length)).toBe(513);
    expect(await page.evaluate(() => window.eventLimits.categoryResult.every((item) => item.revision === 2))).toBe(true);
    expect(await page.evaluate(() => window.eventLimits.categoryReads)).toBe(1);
    // Promoting must discard the individual ID keys rather than retaining an
    // unbounded duplicate invalidation set alongside the category marker.
    expect(await page.evaluate(() => window.eventLimits.discardedIdReads)).toBe(0);
    await expect.poll(() => page.evaluate(() => Array.from({ length: 513 }, (_, index) =>
        window.eventLimits.coordinator.operationSnapshot(window.eventLimits.id(index + 1)).value?.revision === 2).every(Boolean))).toBe(true);
    const reads = await page.evaluate(() => window.eventLimits.reads.length);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(reads);
    expect(await page.evaluate(() => window.eventLimits.categoryReads)).toBe(1);
});

test("the combined 5000-row view cache evicts the oldest snapshot and retains filters, history boundaries and reading anchors", async ({ page }) => {
    await open(page);
    const state = await page.evaluate(() => {
        const events = window.eventLimits.coordinator;
        const metadata = { filter: "failed", anchor: { operationId: "old-active", offset: 17 }, history: { createdAtMs: 42, operationId: "last-history" } };
        events.setMetadata("operations", "reading", metadata);
        events.writeView("projects", { name: "projects" }, 3_000);
        events.writeView("operations", { name: "operations" }, 2_000);
        const exact = events.readView("projects") !== undefined && events.readView("operations") !== undefined;
        events.readView("projects"); // Retain the more recently visited snapshot.
        events.writeView("projects", { name: "projects updated" }, 3_001);
        return { exact, projects: events.readView("projects"), operations: events.readView("operations"),
            metadata: events.metadata("operations", "reading", null) };
    });
    expect(state.exact).toBe(true);
    expect(state.projects).toEqual({ name: "projects updated" });
    expect(state.operations).toBeUndefined();
    expect(state.metadata).toEqual({ filter: "failed", anchor: { operationId: "old-active", offset: 17 }, history: { createdAtMs: 42, operationId: "last-history" } });
});

test("the view cache enforces the encoded eight MiB boundary including non-ASCII data without dropping reading metadata", async ({ page }) => {
    await open(page);
    const state = await page.evaluate(() => {
        const events = window.eventLimits.coordinator;
        const emptyBytes = new TextEncoder().encode(JSON.stringify({ text: "" })).length;
        events.setMetadata("projects", "anchor", { projectId: "retained", offset: 9 });
        events.writeView("projects", { text: "a".repeat(window.eventLimits.limits.cachedBytes - emptyBytes) }, 1);
        const exact = events.readView("projects") !== undefined;
        events.writeView("operations", { text: "界" }, 1);
        const evicted = events.readView("projects") === undefined;
        const survivor = events.readView<{ text: string }>("operations");
        events.writeView("projects", { text: "界".repeat(3 * 1024 * 1024) }, 1);
        return { exact, evicted, survivor, oversized: events.readView("projects") !== undefined,
            metadata: events.metadata("projects", "anchor", null) };
    });
    expect(state.exact).toBe(true);
    expect(state.evicted).toBe(true);
    expect(state.survivor).toEqual({ text: "界" });
    expect(state.oversized).toBe(false);
    expect(state.metadata).toEqual({ projectId: "retained", offset: 9 });
});

test("a queued ordinary read gains the reserved slot when its task becomes terminal", async ({ page }) => {
    await open(page);
    await page.evaluate(() => {
        for (let index = 1; index <= 4; index++) { window.eventLimits.seed(index); window.eventLimits.startRead(index); }
    });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(3);
    await page.evaluate(() => window.eventLimits.complete(4));
    await page.clock.runFor(1);
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(4);
    expect(await page.evaluate(() => window.eventLimits.activeNormal)).toBe(3);
    await page.evaluate(() => window.eventLimits.release(4));
    await expect.poll(() => page.evaluate(() => window.eventLimits.outcomes)).toContainEqual({ id: await page.evaluate(() => window.eventLimits.id(4)), state: "succeeded", revision: 2 });
    await page.evaluate(() => window.eventLimits.releaseAll());
});

test("held stale reads keep the five-second query budget and promote the final in-flight task immediately", async ({ page }) => {
    await open(page);
    await page.evaluate(() => { window.eventLimits.seed(1); window.eventLimits.startRead(1); });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(1);
    for (let step = 0; step < 50; step++) {
        await page.evaluate(() => { window.eventLimits.progress(1, 20); window.eventLimits.releaseIfPending(1); });
        await page.clock.runFor(100);
    }
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBeLessThanOrEqual(21);
    expect(await page.evaluate(() => window.eventLimits.outcomes)).toEqual([]);
    await page.evaluate(() => { window.eventLimits.complete(1); window.eventLimits.releaseAll(); });
    await page.clock.runFor(1);
    await expect.poll(() => page.evaluate(() => window.eventLimits.outcomes)).toEqual([{ id: await page.evaluate(() => window.eventLimits.id(1)), state: "succeeded", revision: 1_002 }]);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBeLessThanOrEqual(22);
    expect(await page.evaluate(() => window.eventLimits.maxActive)).toBe(1);
});

test("stopping between a stale response and its completion rejects the read and leaves no delayed or queued work", async ({ page }) => {
    await open(page);
    await page.evaluate(() => { window.eventLimits.seed(1); window.eventLimits.startRead(1); });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pending.length)).toBe(1);
    await page.evaluate(() => window.eventLimits.progress(1));
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    await page.evaluate(() => { window.eventLimits.stopAtResponse(1); window.eventLimits.release(1); });
    await expect.poll(() => page.evaluate(() => window.eventLimits.outcomes)).toEqual([{ id: await page.evaluate(() => window.eventLimits.id(1)), state: "rejected", code: "daemon_unavailable" }]);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(0);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(1);
    expect(await page.evaluate(() => window.eventLimits.active)).toBe(0);
    await page.evaluate(() => { window.eventLimits.releaseAll(); window.eventLimits.coordinator.start(); });
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    expect(await page.evaluate(async () => (await window.eventLimits.read(1)).revision)).toBe(2);
    expect(await page.evaluate(() => window.eventLimits.reads.length)).toBe(2);
});

test("a delayed old close cannot await or close the replacement subscription during a repeated mount", async ({ page }) => {
    await open(page);
    await page.evaluate(() => window.eventLimits.restartDuringClose());
    await expect.poll(() => page.evaluate(() => window.eventLimits.closeWaiting && window.eventLimits.openWaiting)).toBe(true);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(2);
    await page.evaluate(() => window.eventLimits.releaseClose());
    // The old stop must complete independently of the still-pending new open.
    await expect.poll(() => page.evaluate(() => window.eventLimits.stopFinished)).toBe(true);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.evaluate(() => window.eventLimits.releaseOpen());
    await page.clock.runFor(1);
    expect(await page.evaluate(() => window.eventLimits.closedGenerations)).toEqual([1]);
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(1);
});

test("promoting several delayed list reads wakes each resource once despite recursive scheduling", async ({ page }) => {
    await open(page);
    await page.evaluate(() => {
        window.eventLimits.seed(1); window.eventLimits.seed(2);
        window.eventLimits.holdLists();
        for (let index = 1; index <= 3; index++) window.eventLimits.startListRead(index);
    });
    await expect.poll(() => page.evaluate(() => window.eventLimits.pendingLists.length)).toBe(3);
    expect(await page.evaluate(() => window.eventLimits.activeNormal)).toBe(3);
    await page.evaluate(() => window.eventLimits.recover(1));
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    await page.evaluate(() => window.eventLimits.releasePendingLists());
    await expect.poll(() => page.evaluate(() => window.eventLimits.active)).toBe(0);
    await page.clock.runFor(100);
    expect(await page.evaluate(() => window.eventLimits.listReads.length)).toBe(3);
    // One committed terminal transition promotes all three delayed cursor reads.
    await page.evaluate(() => window.eventLimits.complete(1));
    await page.clock.runFor(1);
    await expect.poll(() => page.evaluate(() => window.eventLimits.listReads.length)).toBe(6);
    expect(await page.evaluate(() => window.eventLimits.pendingLists.length)).toBe(3);
    expect(await page.evaluate(() => window.eventLimits.listReads)).toEqual(await page.evaluate(() => [1, 2, 3, 1, 2, 3].map(window.eventLimits.id)));
    await page.evaluate(() => window.eventLimits.startRead(2));
    await expect.poll(() => page.evaluate(() => window.eventLimits.active)).toBe(4);
    expect(await page.evaluate(() => window.eventLimits.maxActive)).toBe(4);
    expect(await page.evaluate(() => window.eventLimits.maxNormal)).toBe(3);
    await page.evaluate(() => { window.eventLimits.releaseLists(); window.eventLimits.releaseAll(); });
    await expect.poll(() => page.evaluate(() => window.eventLimits.active)).toBe(0);
    await expect.poll(() => page.evaluate(() => window.eventLimits.listOutcomes.length)).toBe(3);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.eventLimits.listReads.length)).toBe(6);
});
