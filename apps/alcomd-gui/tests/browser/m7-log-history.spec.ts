import { expect, test } from "@playwright/test";
test.use({ viewport: { width: 1280, height: 800 } });
const url = "/browser-harness.html?route=/diagnostics&state=empty&diagnostic-log=1&log-history=1&motion=reduced";
test("saved previous sessions and live entries share identity without duplicate rows", async ({ page }) => {
    await page.goto(url);
    const table = page.getByRole("table", { name: "Technical Logs", exact: true });
    await expect(table).toContainText("Previous session entry 3");
    await page.getByRole("button", { name: "Load more · Core", exact: true }).click();
    await expect(table).toContainText("Previous session entry 1");
    await expect(table).toContainText("Core logging initialized.");
    await expect(table.locator("tbody tr")).toHaveCount(4);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
});
test("source filtering scans history and preserves a cursor even when the first page has no match", async ({ page }) => {
    await page.goto(url);
    const table = page.getByRole("table", { name: "Technical Logs", exact: true });
    await expect(table).toContainText("Previous session entry 3");
    await page.getByRole("searchbox", { name: "Search technical logs", exact: true }).fill("Core logging initialized");
    await expect(table).not.toContainText("Previous session entry 3");
    await page.getByRole("button", { name: "Load more · Core", exact: true }).click();
    await expect(table).toContainText("Core logging initialized.");
    await expect(table.locator("tbody tr")).toHaveCount(1);
});
test("history failure remains explicit and retry retains accepted real live data", async ({ page }) => {
    await page.goto(url + "&history-initial-failure=1");
    await expect(page.getByRole("alert")).toContainText("Core logs could not be read.");
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-diagnostic-log", { detail: { message: "Live during history failure" } })));
    const table = page.getByRole("table", { name: "Technical Logs", exact: true });
    await expect(table).toContainText("Live during history failure");
    await page.locator('[data-log-source="core"]').getByRole("button", { name: "Retry", exact: true }).click();
    await expect(table).toContainText("Previous session entry 3");
    await expect(table.getByText("Live during history failure", { exact: true })).toHaveCount(1);
});
