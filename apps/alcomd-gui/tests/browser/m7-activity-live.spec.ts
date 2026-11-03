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
    return table;
}

test("activity automatically merges multi-page bursts and task changes while retaining history, filters and scroll position", async ({ page }) => {
    const table = await open(page);
    await expect(page.locator(".page-header-refresh")).toHaveCount(0);
    const search = page.getByRole("searchbox", { name: "Search activity", exact: true });
    await search.fill("operation");
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await page.getByRole("combobox", { name: "Record type", exact: true }).click();
    await page.getByRole("option", { name: "operation", exact: true }).click();
    const original = await table.elementHandle();
    await page.evaluate((id) => {
        window.h5Utilities.complete(id, "failed", "extension_signature_invalid");
        for (let sequence = 1; sequence <= 20; sequence++) window.h5Utilities.recordActivity({
            type: "event", eventSequence: sequence, occurredAtMs: 1_700_000_002_000,
            summaryCode: "operation.extensions.live", resourceKind: "Extension", resourceId: `live-${sequence}`
        });
    }, H5_INSTALL_ID);
    await page.clock.fastForward(3_000);
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(table.getByRole("row").filter({ hasText: H5_INSTALL_ID })).toContainText("failed");
    await expect(table).toContainText(H5_UNINSTALL_ID);
    await expect(search).toHaveValue("operation");
    await expect(page.getByRole("checkbox", { name: "Show details", exact: true })).toBeChecked();
    await expect(page.locator('.logs-filter-toolbar md-filled-select').filter({ has: page.getByRole("combobox", { name: "Record type", exact: true }) })).toHaveJSProperty("value", "operation");
    expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
    await page.getByRole("combobox", { name: "Record type", exact: true }).click();
    await page.getByRole("option", { name: "All Record type", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(23);
    await expect(table).toContainText("live-1");
    await expect(table).toContainText("live-20");
    const anchor = table.getByText(H5_UNINSTALL_ID, { exact: true });
    await anchor.scrollIntoViewIfNeeded();
    const before = (await anchor.boundingBox())!.y;
    await page.evaluate(() => window.h5Utilities.recordActivity({ type: "event", eventSequence: 21, occurredAtMs: 1_700_000_005_000,
        summaryCode: "operation.extensions.live", resourceKind: "Extension", resourceId: "live-21" }));
    await page.clock.fastForward(3_000);
    await expect(table.locator("tbody tr")).toHaveCount(24);
    await expect(anchor).toBeInViewport();
    expect(Math.abs((await anchor.boundingBox())!.y - before)).toBeLessThanOrEqual(1);
});

test("activity keeps loaded history on a failed read, recovers automatically, and isolates pending reads after leaving", async ({ page }) => {
    const table = await open(page);
    await page.evaluate(() => window.h5Utilities.failNext("activity.list", "permission_denied"));
    await page.clock.fastForward(3_000);
    await expect(page.getByRole("alert")).toContainText("New activity could not be loaded. The last loaded entries are still shown.");
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    await page.clock.fastForward(3_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await page.evaluate(() => window.h5Utilities.hold("activity.list", true));
    await page.clock.fastForward(3_000);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("activity.list"))).toBe(1);
    const reads = await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length);
    await page.clock.fastForward(9_000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length)).toBe(reads);
    await page.getByRole("navigation", { name: "Logs", exact: true }).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await expect(page.getByRole("table", { name: "Technical Logs", exact: true })).toBeVisible();
    await page.evaluate(() => window.h5Utilities.release("activity.list", "permission_denied"));
    await page.clock.fastForward(6_000);
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length)).toBe(reads);
});

test("activity pauses while hidden and catches up when the page becomes visible", async ({ page }) => {
    const table = await open(page);
    const reads = await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "hidden" });
        document.dispatchEvent(new Event("visibilitychange"));
        window.h5Utilities.recordActivity({ type: "event", eventSequence: 1, occurredAtMs: 1_700_000_004_000, summaryCode: "New visible event" });
    });
    await page.clock.fastForward(6_000);
    expect(await page.evaluate(() => window.h5Utilities.requests.filter((item) => item.method === "activity.list").length)).toBe(reads);
    await page.evaluate(() => {
        Object.defineProperty(document, "visibilityState", { configurable: true, value: "visible" });
        document.dispatchEvent(new Event("visibilitychange"));
    });
    await expect(table).toContainText("New visible event");
    await expect(table.locator("tbody tr")).toHaveCount(4);
});
