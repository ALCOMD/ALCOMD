import { expect, test, type Page } from "@playwright/test";
import type {} from "./core-events-client";
import type {} from "./h5-utilities-client";
import type {} from "./h4-management-client";

const taskId = "00000000-0000-4000-8000-000000000701";
const extensionId = "com.example.alpha";
test.use({ viewport: { width: 1280, height: 800 } });

async function openTask(page: Page) {
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/operations/${taskId}`)}&h5-utilities=1&h5-tasks-live=1&motion=reduced`);
    await expect(page.getByRole("heading", { name: "Operation detail", level: 1, exact: true })).toBeVisible();
    await expect(page.locator(".operation-follow strong")).toHaveText("running");
    await pauseClock(page);
    await page.clock.runFor(1_000);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
}

async function pauseClock(page: Page) {
    // install() lets page startup run normally. Freeze after loading so ACK
    // deadlines and the five-second query budget use controlled time only.
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
}

async function readCounts(page: Page) {
    return page.evaluate((id) => {
        const requests = window.h5Utilities.requests;
        return {
            get: requests.filter((item) => item.method === "operations.get" && item.params.operationId === id).length,
            count: requests.filter((item) => item.method === "operations.list").length,
            activity: requests.filter((item) => item.method === "activity.list").length,
            // This outer client records delegated methods as well as H5 reads;
            // the H5-only request array cannot prove other pages stayed idle.
            others: window.coreEvents.rpcRequests.filter((item) => /^(projectsList|projectGet|repositoriesList|repositoryGet|repositoryPackages|userPackagesList|userPackageGet|templatesList|templateGet|backupsList|backupGet|extensionsList|extensionGet|extensionPermissionsList|unityInstallationsList|unityInstallationGet|unityProjectLaunchConfigGet|unityWriterState|unityLaunchOptions)$/.test(item.method)).length
        };
    }, taskId);
}

async function progress(page: Page, revision: number, phase = "source_verified") {
    await page.evaluate(({ id, revision, phase }) => window.h5Utilities.recordOperation({
        operationId: id, kind: "extensions.install", state: "running", revision,
        createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000 + revision,
        progress: { phase }
    }), { id: taskId, revision, phase });
}

async function subscriptions(page: Page) {
    return page.evaluate(() => window.coreEvents.requests.filter((item) => item.method === "subscribe").length);
}

test("1000 progress changes over five seconds stay within the query budget and terminal updates bypass coalescing", async ({ page }) => {
    await openTask(page);
    const before = await readCounts(page);
    for (let step = 0; step < 50; step++) {
        await page.evaluate(({ id, step }) => {
            for (let index = 0; index < 20; index++) {
                const revision = step * 20 + index + 2;
                window.h5Utilities.recordOperation({ operationId: id, kind: "extensions.install", state: "running", revision,
                    createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000 + revision,
                    progress: { phase: index % 2 === 0 ? "source_verified" : "prepared" } });
            }
        }, { id: taskId, step });
        await page.clock.runFor(100);
    }
    const progressed = await readCounts(page);
    expect(progressed.get - before.get).toBeGreaterThan(0);
    expect(progressed.get - before.get).toBeLessThanOrEqual(22);
    expect(progressed.count).toBe(before.count);
    expect(progressed.activity).toBe(before.activity);
    expect(progressed.others).toBe(before.others);

    await page.evaluate((id) => window.h5Utilities.complete(id, "succeeded"), taskId);
    await page.clock.runFor(1);
    // No ordinary 100ms merge or 250ms progress interval is advanced here.
    await expect(page.locator(".operation-follow strong")).toHaveText("succeeded");
    const finished = await readCounts(page);
    expect(finished.get - before.get).toBeLessThanOrEqual(22);
    await page.clock.runFor(1_000);
    const idle = await readCounts(page);
    await page.clock.fastForward(60_000);
    expect(await readCounts(page)).toEqual(idle);
});

test("ACK does not wait for a held resource read and a stalled consumer retains only one pending batch", async ({ page }) => {
    await openTask(page);
    await page.evaluate(() => window.h5Utilities.hold("operations.get", true));
    await progress(page, 2, "prepared");
    await page.clock.runFor(300);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.get"))).toBe(1);
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    await progress(page, 3, "staging_complete");
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    expect(await page.evaluate(() => window.h5Utilities.pending("operations.get"))).toBe(1);
    await page.evaluate(() => { window.h5Utilities.hold("operations.get", false); window.h5Utilities.release("operations.get"); });
    await page.clock.runFor(600);
    await expect(page.locator(".operation-follow")).toContainText("staging complete");

    const opened = await subscriptions(page);
    await page.evaluate(() => window.coreEvents.holdAcknowledgement(true));
    await progress(page, 4, "prepared");
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(1);
    await page.evaluate(() => window.coreEvents.duplicate());
    await page.evaluate((id) => window.h5Utilities.complete(id, "succeeded"), taskId);
    await page.clock.runFor(500);
    expect(await page.evaluate(() => window.coreEvents.pending)).toBe(1);
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(1);
    expect(await subscriptions(page)).toBe(opened);
    await page.evaluate(() => window.coreEvents.holdAcknowledgement(false));
    await page.clock.runFor(1);
    await expect(page.locator(".operation-follow strong")).toHaveText("succeeded");
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    expect(await subscriptions(page)).toBe(opened);
});

test("ACK timeout cancels the old consumer and reconnects once with a fresh baseline", async ({ page }) => {
    await openTask(page);
    const opened = await subscriptions(page);
    await page.evaluate(() => window.coreEvents.holdAcknowledgement(true));
    await progress(page, 2, "prepared");
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(1);
    await progress(page, 3, "staging_complete");
    await page.clock.runFor(10_000);
    expect(await page.evaluate(() => window.coreEvents.pending)).toBe(0);
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(0);
    await page.evaluate(() => window.coreEvents.holdAcknowledgement(false));
    await page.clock.runFor(250);
    await expect.poll(() => subscriptions(page)).toBe(opened + 1);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    const request = await page.evaluate(() => window.coreEvents.requests.filter((item) => item.method === "subscribe").at(-1)!.params as { cursor?: unknown });
    expect(request.cursor).toBeUndefined();
    await page.clock.runFor(1_000);
    await expect(page.locator(".operation-follow")).toContainText("staging complete");
    expect(await page.evaluate(() => window.coreEvents.pending)).toBe(0);
});

test("transient disconnect recovers authoritative state and permanent closed never retries on a timer", async ({ page }) => {
    await openTask(page);
    const opened = await subscriptions(page);
    await page.evaluate(() => window.coreEvents.disconnect());
    await progress(page, 2, "prepared");
    await page.clock.runFor(250);
    await expect.poll(() => subscriptions(page)).toBe(opened + 1);
    await page.clock.runFor(500);
    await expect(page.locator(".operation-follow")).toContainText("prepared");
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.clock.runFor(500);
    const stable = await readCounts(page);
    await page.clock.fastForward(30_000);
    expect(await readCounts(page)).toEqual(stable);

    const recovered = await subscriptions(page);
    await page.evaluate(() => window.coreEvents.close(false, "permission_denied"));
    await expect(page.getByRole("alert")).toContainText("Updates are temporarily unavailable. The displayed data may be out of date.");
    await page.clock.fastForward(60_000);
    expect(await subscriptions(page)).toBe(recovered);
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(0);
    await expect(page.locator(".operation-follow strong")).toHaveText("running");
});

test("a new Core epoch reloads the snapshot even when its final state predates the new subscription baseline", async ({ page }) => {
    await openTask(page);
    await progress(page, 2, "prepared");
    await page.clock.runFor(500);
    await expect(page.locator(".operation-follow")).toContainText("prepared");
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    const instance = await page.evaluate(() => window.coreEvents.instanceId);
    const before = await readCounts(page);
    const acknowledgements = await page.evaluate(() => window.coreEvents.requests.filter((item) => item.method === "ack").length);
    await page.evaluate((id) => {
        window.coreEvents.restart();
        // The mutation precedes the new ready baseline, so only a snapshot can recover it.
        window.h5Utilities.complete(id, "succeeded");
    }, taskId);
    await page.clock.runFor(250);
    await page.clock.runFor(500);
    await expect(page.locator(".operation-follow strong")).toHaveText("succeeded");
    expect(await page.evaluate(() => window.coreEvents.instanceId)).not.toBe(instance);
    expect((await readCounts(page)).get).toBeGreaterThan(before.get);
    expect(await page.evaluate(() => window.coreEvents.requests.filter((item) => item.method === "ack").length)).toBe(acknowledgements);
    await expect(page.getByRole("heading", { name: "Operation detail", level: 1, exact: true })).toBeVisible();
    await expect(page.locator(".reminder-floating")).toBeHidden();
    expect(await page.evaluate(() => window.coreEvents.connections)).toBe(1);
});

test("cross-client extension desired changes use scope invalidation without a fake durable event", async ({ page }) => {
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/extensions/${extensionId}`)}&state=ready&h4-management=1&motion=reduced`);
    await expect(page.getByRole("heading", { name: "Extension detail", level: 1, exact: true })).toBeVisible();
    await expect(page.getByText("Desired state", { exact: true }).locator("..")).toContainText("enabled");
    await pauseClock(page);
    await page.clock.runFor(1_000);
    await page.evaluate((id) => window.h4Management.setDesired(id, "installed_disabled"), extensionId);
    await page.clock.runFor(500);
    await expect(page.getByText("Desired state", { exact: true }).locator("..")).toContainText("installed disabled");
    await expect(page.getByText("Runtime", { exact: true }).locator("..")).toContainText("stopped");
    expect(await page.evaluate(() => window.h4Management.requests.filter((item) => ["extensions.enable", "extensions.disable"].includes(item.method)).length)).toBe(0);
    await expect.poll(() => page.evaluate(() => window.coreEvents.pending)).toBe(0);
    await page.evaluate(() => window.coreEvents.disconnect());
    await page.clock.runFor(250);
    const cursor = await page.evaluate(() => (window.coreEvents.requests.filter((item) => item.method === "subscribe").at(-1)!.params as { cursor?: { sequence: number; resourcesRevision: number } }).cursor);
    expect(cursor).toMatchObject({ sequence: 0, resourcesRevision: 1 });
});

test("cross-client grant changes update an already open permission list through extension scope invalidation", async ({ page }) => {
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/extensions/${extensionId}`)}&state=ready&h4-management=1&motion=reduced`);
    await expect(page.getByRole("heading", { name: "Extension detail", level: 1, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
    // MD's native dialog lives in Shadow DOM; the permission content is slotted
    // light DOM, so scope content through the open MD host as the H4 suite does.
    const dialog = page.locator("md-dialog[open]");
    const allowed = dialog.getByRole("region", { name: "Allowed permissions", exact: true });
    await expect(allowed).toContainText("Run in background");
    await pauseClock(page);
    await page.clock.runFor(1_000);
    const reads = await page.evaluate(() => window.h4Management.requests.filter((item) => item.method === "extensions.permissions.list").length);
    await page.evaluate((id) => window.h4Management.setGrants(id, []), extensionId);
    await page.clock.runFor(500);
    await expect(allowed).toContainText("No permissions granted");
    await expect(allowed.locator("li")).toHaveCount(0);
    expect(await page.evaluate(() => window.h4Management.requests.filter((item) => item.method === "extensions.permissions.list").length)).toBeGreaterThan(reads);
    expect(await page.evaluate(() => window.h4Management.requests.filter((item) => ["extensions.setGrant", "extensions.revokeGrant"].includes(item.method)).length)).toBe(0);
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
});

test("cross-client project launch arguments update the current Unity page through its committed aggregate event", async ({ page }) => {
    const projectId = "00000000-0000-4000-8000-000000000101";
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${projectId}/unity`)}&motion=reduced`);
    await expect(page.getByRole("heading", { name: "Project Unity", exact: true })).toBeVisible();
    const argumentsRow = page.getByText("Arguments", { exact: true }).locator("..");
    await expect(argumentsRow).toContainText("1 arguments");
    await pauseClock(page);
    const revision = await page.evaluate(async (id) => (await window.fixtureCore.unityProjectLaunchConfigGet(id)).config.revision, projectId);
    await page.evaluate(async ({ id, revision }) => { await window.fixtureCore.unityProjectLaunchConfigSet(id, ["-force-d3d11", "-noUpm"], revision); }, { id: projectId, revision });
    await page.clock.runFor(500);
    await expect(argumentsRow).toContainText("2 arguments");
    await page.evaluate(async ({ id, revision }) => { await window.fixtureCore.unityProjectLaunchConfigClear(id, revision + 1); }, { id: projectId, revision });
    await page.clock.runFor(500);
    await expect(argumentsRow).toContainText("Use global defaults");
    const stable = await page.evaluate(() => window.coreEvents.rpcRequests.length);
    await page.clock.fastForward(60_000);
    expect(await page.evaluate(() => window.coreEvents.rpcRequests.length)).toBe(stable);
});

test("an explicit page retry clears a failed resynchronization job without another connection or timed retry", async ({ page }) => {
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/extensions/${extensionId}`)}&state=ready&h4-management=1&motion=reduced`);
    await expect(page.getByRole("heading", { name: "Extension detail", level: 1, exact: true })).toBeVisible();
    await pauseClock(page);
    await page.evaluate(() => { window.h4Management.failNext("extensions.get", "permission_denied"); window.coreEvents.restart(); });
    await page.clock.runFor(750);
    await expect(page.getByRole("alert")).toContainText("The last loaded result is still shown.");
    await expect(page.getByRole("status").filter({ hasText: "Updates are being restored." })).toBeVisible();
    const reads = await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.get").length);
    await page.clock.fastForward(10_000);
    expect(await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.get").length)).toBe(reads);
    const opened = await subscriptions(page);
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await page.clock.runFor(500);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.getByRole("status").filter({ hasText: "Updates are being restored." })).toHaveCount(0);
    expect(await subscriptions(page)).toBe(opened);
    await expect(page.getByRole("heading", { name: "Extension detail", level: 1, exact: true })).toBeVisible();
});
