import { expect, test, type Page } from "@playwright/test";
import { H5_INSTALL_ID } from "./h5-utilities-client";

test.use({ viewport: { width: 1280, height: 800 } });

async function open(page: Page) {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/operations&h5-utilities=1&h5-tasks-live=1&motion=reduced");
    const table = page.getByRole("table", { name: "Tasks", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(2);
    await expect(page.locator('.page-header [data-icon-name="refresh"]')).toHaveCount(0);
    return table;
}

test("tasks automatically merge multi-page arrivals and update older unfinished tasks without dropping loaded history", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("operations.list", "store_unavailable"));
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("button", { name: "Retry loading more", exact: true })).toBeVisible();
    await expect(table.locator("tbody tr")).toHaveCount(2);
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
    await page.clock.fastForward(3_000);
    await expect(table.locator("tbody tr")).toHaveCount(9);
    await expect(table).toContainText("new task 1");
    await expect(table).toContainText("new task 5");
    await expect(table.getByRole("row").filter({ hasText: "Install extension" }).filter({ hasText: "succeeded" })).toHaveCount(1);
    expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
});

test("task read failures preserve rows and recover automatically without duplicate reads or late errors after leaving", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("operations.list", "permission_denied"));
    await page.clock.fastForward(3_000);
    await expect(page.getByRole("alert")).toContainText("Tasks could not be updated. The last loaded entries are still shown.");
    await expect(table.locator("tbody tr")).toHaveCount(2);
    await page.clock.fastForward(3_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await page.evaluate(() => window.h5Utilities.hold("operations.list", true));
    await page.clock.fastForward(3_000);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.list"))).toBe(1);
    const reads = await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length);
    await page.clock.fastForward(9_000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length)).toBe(reads);
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Projects$/ }) }).click();
    await expect(page.getByRole("heading", { name: "Projects", level: 1, exact: true })).toBeVisible();
    // Entering another route also reads the shell's task count once.
    const afterNavigation = await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length);
    await page.evaluate(() => window.h5Utilities.release("operations.list", "permission_denied"));
    await page.clock.fastForward(6_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length)).toBe(afterNavigation);
});

test("task polling pauses while hidden and resumes on visibility", async ({ page }) => {
    const table = await open(page);
    const reads = await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
        document.dispatchEvent(new Event("visibilitychange"));
        window.h5Utilities.recordOperation({ operationId: "00000000-0000-4000-8000-000000009099", kind: "visible.new.task", state: "running", revision: 1,
            createdAtMs: 1_700_000_100_000, updatedAtMs: 1_700_000_100_000 });
    });
    await page.clock.fastForward(6_000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "operations.list").length)).toBe(reads);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
        document.dispatchEvent(new Event("visibilitychange"));
    });
    await expect(table).toContainText("visible new task");
    await expect(table.locator("tbody tr")).toHaveCount(3);
});
