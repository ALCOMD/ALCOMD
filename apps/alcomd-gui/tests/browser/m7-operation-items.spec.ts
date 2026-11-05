import { expect, test, type Page } from "@playwright/test";
import { H5_INSTALL_ID } from "./h5-utilities-client";

test.use({ viewport: { width: 1280, height: 800 } });

async function open(page: Page) {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/operations&h5-utilities=1&h5-tasks-live=1&motion=reduced");
    const table = page.getByRole("table", { name: "Tasks", exact: true });
    // The two-row recent page and the older active install are separate reads.
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(page.locator('.page-header [data-icon-name="refresh"]')).toHaveCount(0);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await page.clock.runFor(500);
    return table;
}

async function listReads(page: Page, activeOnly = false) {
    return page.evaluate((onlyActive) => window.h5Utilities.requests.filter((item) => item.method === "operations.list"
        && (!onlyActive || item.params.activeOnly === true)).length, activeOnly);
}

async function addTask(page: Page, index: number, prefix = "new.task") {
    await page.evaluate(({ index, prefix }) => window.h5Utilities.recordOperation({
        operationId: `00000000-0000-4000-8000-${String(9000 + index).padStart(12, "0")}`,
        kind: `${prefix}.${index}`, state: "running", revision: 1,
        createdAtMs: 1_700_000_100_000 + index, updatedAtMs: 1_700_000_100_000 + index
    }), { index, prefix });
}

test("tasks merge notified multi-page arrivals and older task completion without dropping loaded history", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("operations.list", "store_unavailable"));
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("button", { name: "Retry loading more", exact: true })).toBeVisible();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(4);
    const original = await table.elementHandle();
    await page.evaluate((id) => {
        window.h5Utilities.complete(id, "succeeded");
        for (let index = 1; index <= 5; index++) window.h5Utilities.recordOperation({
            operationId: `00000000-0000-4000-8000-00000000900${index}`, kind: `new.task.${index}`, state: "running", revision: 1,
            createdAtMs: 1_700_000_010_000 + index, updatedAtMs: 1_700_000_010_000 + index
        });
    }, H5_INSTALL_ID);
    await page.clock.runFor(500);
    await expect(table.locator("tbody tr")).toHaveCount(9);
    await expect(table).toContainText("new task 1");
    await expect(table).toContainText("new task 5");
    await expect(table.getByRole("row").filter({ hasText: "Install extension" }).filter({ hasText: "succeeded" })).toHaveCount(1);
    expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
    const idle = await listReads(page);
    await page.clock.fastForward(60_000);
    expect(await listReads(page)).toBe(idle);
});

test("task read failures retain data until a new change and isolate late list responses after leaving", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("operations.list", "permission_denied"));
    await addTask(page, 7);
    await page.clock.runFor(500);
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(table.locator("tbody tr")).toHaveCount(3);
    const failed = await listReads(page);
    await page.clock.fastForward(9_000);
    expect(await listReads(page)).toBe(failed);
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await addTask(page, 8);
    await page.clock.runFor(500);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(table.locator("tbody tr")).toHaveCount(5);
    await expect(table).toContainText("new task 7");
    await expect(table).toContainText("new task 8");
    await page.evaluate(() => window.h5Utilities.hold("operations.list", true));
    await addTask(page, 9);
    await page.clock.runFor(500);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.list"))).toBe(1);
    const reads = await listReads(page);
    const activeReads = await listReads(page, true);
    await addTask(page, 10);
    await page.clock.fastForward(9_000);
    expect(await listReads(page)).toBe(reads);
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Projects$/ }) }).click();
    await expect(page.getByRole("heading", { name: "Projects", level: 1, exact: true })).toBeVisible();
    await page.evaluate(() => window.h5Utilities.release("operations.list", "permission_denied"));
    await page.clock.runFor(1_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    // The shell may reconcile the new count; the retired task view cannot start
    // active pagination or keep rereading on a clock after its held response.
    expect(await listReads(page, true)).toBe(activeReads);
    const settled = await listReads(page);
    await page.clock.fastForward(6_000);
    expect(await listReads(page)).toBe(settled);
});

test("task notifications stay dirty while hidden and restore all active and recent pages on visibility", async ({ page }) => {
    const table = await open(page);
    const reads = await listReads(page);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
        document.dispatchEvent(new Event("visibilitychange"));
        for (let index = 1; index <= 5; index++) window.h5Utilities.recordOperation({
            operationId: `00000000-0000-4000-8000-${String(9090 + index).padStart(12, "0")}`, kind: `visible.new.task.${index}`,
            state: "running", revision: 1, createdAtMs: 1_700_000_100_000 + index, updatedAtMs: 1_700_000_100_000 + index });
    });
    await page.clock.fastForward(6_000);
    expect(await listReads(page)).toBe(reads);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
        document.dispatchEvent(new Event("visibilitychange"));
    });
    await page.clock.runFor(500);
    await expect(table).toContainText("visible new task 1");
    await expect(table).toContainText("visible new task 5");
    await expect(table.locator("tbody tr")).toHaveCount(8);
    expect(await listReads(page)).toBeGreaterThan(reads + 1);
    await expect(table).toContainText("Install extension");
    const idle = await listReads(page);
    await page.clock.fastForward(60_000);
    expect(await listReads(page)).toBe(idle);
});
