import { expect, test, type Page } from "@playwright/test";
import { H5_INSTALL_ID, H5_UNINSTALL_ID } from "./h5-utilities-client";

test.use({ viewport: { width: 1280, height: 800 } });

async function open(page: Page) {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/activity&h5-utilities=1&h5-activity-live=1&motion=reduced");
    const table = page.getByRole("table", { name: "Activity", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(2);
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await page.clock.runFor(500);
    return table;
}

async function activityReads(page: Page) {
    return page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length);
}

async function addActivity(page: Page, sequence: number, summaryCode: string) {
    await page.evaluate(({ sequence, summaryCode }) => window.h5Utilities.recordActivity({
        type: "event", eventSequence: sequence, occurredAtMs: 1_700_000_010_000 + sequence,
        summaryCode, resourceKind: "project", resourceId: `00000000-0000-4000-8000-${String(9000 + sequence).padStart(12, "0")}`
    }), { sequence, summaryCode });
}

test("activity merges notified multi-page bursts and task changes while retaining history, filters and scroll position", async ({ page }) => {
    const table = await open(page);
    await expect(page.locator('.page-header [data-icon-name="refresh"]')).toHaveCount(0);
    const search = page.getByRole("searchbox", { name: "Search activity", exact: true });
    await search.fill("operation");
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await page.getByRole("combobox", { name: "Record type", exact: true }).click();
    await page.clock.runFor(300);
    await page.getByRole("option", { name: "operation", exact: true }).click();
    await page.clock.runFor(300);
    const original = await table.elementHandle();
    await page.evaluate((id) => {
        window.h5Utilities.complete(id, "failed", "extension_signature_invalid");
        for (let sequence = 1; sequence <= 20; sequence++) window.h5Utilities.recordActivity({
            type: "event", eventSequence: sequence, occurredAtMs: 1_700_000_002_000,
            summaryCode: `operation.extensions.live-${sequence}`, resourceKind: "project", resourceId: `00000000-0000-4000-8000-${String(9000 + sequence).padStart(12, "0")}`
        });
    }, H5_INSTALL_ID);
    await page.clock.runFor(500);
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(table.getByRole("row").filter({ hasText: H5_INSTALL_ID })).toContainText("failed");
    await expect(table).toContainText(H5_UNINSTALL_ID);
    await expect(search).toHaveValue("operation");
    await expect(page.getByRole("checkbox", { name: "Show details", exact: true })).toBeChecked();
    await expect(page.locator('.logs-filter-toolbar md-filled-select').filter({ has: page.getByRole("combobox", { name: "Record type", exact: true }) })).toHaveJSProperty("value", "operation");
    expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
    await page.getByRole("combobox", { name: "Record type", exact: true }).click();
    await page.clock.runFor(300);
    await page.getByRole("option", { name: "All Record type", exact: true }).click();
    await page.clock.runFor(300);
    await expect(table.locator("tbody tr")).toHaveCount(23);
    await expect(table).toContainText("live-1");
    await expect(table).toContainText("live-20");
    const anchor = table.getByText(H5_UNINSTALL_ID, { exact: true });
    await anchor.scrollIntoViewIfNeeded();
    const before = (await anchor.boundingBox())!.y;
    await page.evaluate(() => window.h5Utilities.recordActivity({ type: "event", eventSequence: 21, occurredAtMs: 1_700_000_005_000,
        summaryCode: "operation.extensions.live-21", resourceKind: "project", resourceId: "00000000-0000-4000-8000-000000009021" }));
    await page.clock.runFor(500);
    await expect(table.locator("tbody tr")).toHaveCount(24);
    await expect(anchor).toBeInViewport();
    expect(Math.abs((await anchor.boundingBox())!.y - before)).toBeLessThanOrEqual(1);
    const idle = await activityReads(page);
    await page.clock.fastForward(60_000);
    expect(await activityReads(page)).toBe(idle);
});

test("activity retains failed reads until a valid change retries and isolates pending reads after leaving", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("activity.list", "permission_denied"));
    await page.evaluate((id) => window.h5Utilities.complete(id, "failed", "extension_signature_invalid"), H5_INSTALL_ID);
    await page.clock.runFor(500);
    await expect(page.getByRole("alert")).toContainText("New activity could not be loaded. The last loaded entries are still shown.");
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    const failedReads = await activityReads(page);
    await page.clock.fastForward(9_000);
    expect(await activityReads(page)).toBe(failedReads);
    await expect(page.getByRole("alert")).toContainText("New activity could not be loaded.");
    await addActivity(page, 1, "Activity read recovered");
    await page.clock.runFor(500);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(table.locator("tbody tr")).toHaveCount(4);
    await expect(table).toContainText("Activity read recovered");
    await expect(table.getByRole("row").filter({ has: page.locator('[title="operation.extensions.install.failed"]') })).toContainText("failed");
    await page.evaluate(() => window.h5Utilities.hold("activity.list", true));
    await addActivity(page, 2, "Held activity update");
    await page.clock.runFor(500);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("activity.list"))).toBe(1);
    const reads = await activityReads(page);
    await addActivity(page, 3, "Coalesced during held read");
    await page.clock.fastForward(9_000);
    expect(await activityReads(page)).toBe(reads);
    await page.getByRole("navigation", { name: "Logs", exact: true }).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await expect(page.getByRole("table", { name: "Technical Logs", exact: true })).toBeVisible();
    await page.evaluate(() => window.h5Utilities.release("activity.list", "permission_denied"));
    await page.clock.fastForward(6_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await activityReads(page)).toBe(reads);
});

test("activity registers hidden changes without reads and restores every missing page on visibility", async ({ page }) => {
    const table = await open(page);
    const reads = await activityReads(page);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
        document.dispatchEvent(new Event("visibilitychange"));
        for (let index = 1; index <= 5; index++) window.h5Utilities.recordActivity({ type: "event", eventSequence: index,
            occurredAtMs: 1_700_000_004_000 + index, summaryCode: `New visible event ${index}`,
            resourceKind: "project", resourceId: `00000000-0000-4000-8000-${String(9000 + index).padStart(12, "0")}` });
    });
    await page.clock.fastForward(6_000);
    expect(await activityReads(page)).toBe(reads);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
        document.dispatchEvent(new Event("visibilitychange"));
    });
    await page.clock.runFor(500);
    await expect(table).toContainText("New visible event 1");
    await expect(table).toContainText("New visible event 5");
    await expect(table.locator("tbody tr")).toHaveCount(8);
    expect(await activityReads(page)).toBeGreaterThan(reads + 1);
    await expect(table).toContainText("Uninstall extension");
    const idle = await activityReads(page);
    await page.clock.fastForward(60_000);
    expect(await activityReads(page)).toBe(idle);
});

test("returning after cache eviction restores the full loaded history, filter and stable reading anchor", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/activity&h5-utilities=1&h5-activity-live=1&view-cache-pressure=1&motion=reduced");
    const table = page.getByRole("table", { name: "Activity", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(2);
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    for (let index = 1; index <= 30; index++) await addActivity(page, index, `anchor event ${index}`);
    await page.clock.runFor(1_000);
    await expect(table.locator("tbody tr")).toHaveCount(33);
    await page.getByRole("searchbox", { name: "Search activity", exact: true }).fill("anchor event");
    await expect(table.locator("tbody tr")).toHaveCount(30);
    const anchor = table.getByText("anchor event 8", { exact: true });
    await anchor.scrollIntoViewIfNeeded();
    const before = (await anchor.boundingBox())!.y;
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Projects$/ }) }).click();
    await expect(page.getByRole("heading", { name: "Projects", level: 1, exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.viewCachePressure.hasActivity())).toBe(true);
    expect(await page.evaluate(() => window.viewCachePressure.evict())).toBe(true);
    for (let index = 31; index <= 35; index++) await addActivity(page, index, `anchor event ${index}`);
    const reads = await activityReads(page);
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Logs$/ }) }).click();
    await page.clock.runFor(1_000);
    await expect(page.getByRole("searchbox", { name: "Search activity", exact: true })).toHaveValue("anchor event");
    await expect(table.locator("tbody tr")).toHaveCount(35);
    await expect(table).toContainText("anchor event 35");
    await expect(table).toContainText("anchor event 1");
    expect(await activityReads(page)).toBeGreaterThan(reads + 1);
    await expect(anchor).toBeInViewport();
    expect(Math.abs((await anchor.boundingBox())!.y - before)).toBeLessThanOrEqual(1);
});
