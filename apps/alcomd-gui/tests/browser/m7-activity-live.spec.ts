import { expect, test, type Page } from "@playwright/test";
import { H5_INSTALL_ID } from "./h5-utilities-client";
const url = "/browser-harness.html?route=/activity&h5-utilities=1&h5-activity-live=1&motion=reduced";
const table = (page: Page) => page.getByRole("table", { name: "Activity", exact: true });
async function append(page: Page, n: number) {
    await page.evaluate((n) => window.h5Utilities.recordActivity({ activityId: `00000000-0000-4000-8000-${String(9000+n).padStart(12,"0")}`, revision: 1, action: "project.add", status: "succeeded", startedAtMs: 1_700_000_010_000+n, subject: { kind: "project", name: `New project ${n}` }, parameters: {} }), n);
}
test("new activity comes from its own committed event without polling", async ({ page }) => {
    await page.clock.install(); await page.goto(url);
    await expect(table(page).locator("tbody tr")).toHaveCount(2);
    await append(page, 1); await page.clock.runFor(600);
    await expect(table(page)).toContainText("New project 1");
    const reads = await page.evaluate(() => window.h5Utilities.requests.filter((r) => r.method === "activity.list").length);
    await page.clock.fastForward(120000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((r) => r.method === "activity.list").length)).toBe(reads);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
});
test("loaded older active rows update by ID without changing their started-time order", async ({ page }) => {
    await page.goto(url); await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(table(page).locator("tbody tr")).toHaveCount(3);
    const before = await table(page).locator("tbody tr").allTextContents();
    await page.evaluate((id) => window.h5Utilities.complete(id, "failed", "extension_signature_invalid"), H5_INSTALL_ID);
    await expect(table(page).locator('tbody tr[data-row-key="' + H5_INSTALL_ID + '"]')).toContainText("failed");
    const after = await table(page).locator("tbody tr").allTextContents();
    expect(after.slice(0,2)).toEqual(before.slice(0,2));
    await expect(table(page).locator("tbody tr")).toHaveCount(3);
    expect(await page.evaluate(() => window.h5Utilities.requests.some((r) => r.method === "activity.get"))).toBe(true);
});
test("activity paging failure keeps loaded history and retries the same cursor", async ({ page }) => {
    await page.goto(url); await expect(table(page).locator("tbody tr")).toHaveCount(2);
    await page.evaluate(() => window.h5Utilities.failNext("activity.list", "store_unavailable"));
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Could not load more items");
    await expect(table(page).locator("tbody tr")).toHaveCount(2);
    await page.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(table(page).locator("tbody tr")).toHaveCount(3);
    const cursors = await page.evaluate(() => window.h5Utilities.requests.filter((r) => r.method === "activity.list" && r.params.cursor !== undefined).map((r) => r.params.cursor));
    expect(cursors[0]).toEqual(cursors[1]);
});

test("progress changes neither create activities nor query their history", async ({ page }) => {
    await page.clock.install();
    await page.goto(url);
    await expect(table(page).locator("tbody tr")).toHaveCount(2);
    await page.clock.runFor(600);
    const before = await page.evaluate(() => window.h5Utilities.requests.filter((r) => r.method.startsWith("activity.")).length);
    await page.evaluate((operationId) => {
        for (let n = 0; n < 100; n++) window.h5Utilities.recordOperation({ operationId, revision: n + 2, kind: "extensions.install", state: "running", createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_001_000 + n, progress: { phase: "source_verified" } });
    }, H5_INSTALL_ID);
    await page.clock.runFor(1000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((r) => r.method.startsWith("activity.")).length)).toBe(before);
    await expect(table(page).locator("tbody tr")).toHaveCount(2);
});
