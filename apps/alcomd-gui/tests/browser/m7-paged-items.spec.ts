import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/tests/browser/paged-items.html");
    await expect(page.getByRole("list", { name: "Loaded items" })).toHaveText("First item");
});

test("paging is explicit, locks duplicate requests, retains items on error and retries the same cursor", async ({ page }) => {
    expect(await page.evaluate(() => window.pendingPages.length)).toBe(0);
    await page.locator("md-filled-tonal-button").filter({ hasText: "Load more" }).evaluate((button) => {
        button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        button.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await expect(page.getByRole("status")).toHaveText("Loading more items…");
    expect(await page.evaluate(() => window.pendingPages.map((pending) => pending.cursor))).toEqual([1]);
    await page.evaluate(() => window.pendingPages[0]!.reject(new Error("Private transport error")));
    await expect(page.getByRole("alert")).toContainText("loaded items are still available");
    await expect(page.getByRole("list")).toHaveText("First item");
    await expect(page.getByText("Private transport error")).toHaveCount(0);
    await page.getByRole("button", { name: "Retry loading more" }).click();
    expect(await page.evaluate(() => window.pendingPages.map((pending) => pending.cursor))).toEqual([1, 1]);
    await page.evaluate(() => window.pendingPages[1]!.resolve({ items: ["Second item"], nextCursor: 2 }));
    await expect(page.getByRole("list")).toHaveText("First itemSecond item");
    expect(await page.evaluate(() => window.pendingPages.length)).toBe(2);
    await page.getByRole("button", { name: "Load more" }).click();
    expect(await page.evaluate(() => window.pendingPages[2]!.cursor)).toBe(2);
    await page.evaluate(() => window.pendingPages[2]!.resolve({ items: ["Last item"] }));
    await expect(page.getByRole("list")).toHaveText("First itemSecond itemLast item");
    await expect(page.getByRole("button", { name: "Load more" })).toHaveCount(0);
});

test("first-page and cursor replacements discard appended items and late responses", async ({ page }) => {
    await page.getByRole("button", { name: "Load more" }).click();
    await page.getByRole("button", { name: "Refresh first page" }).click();
    await expect(page.getByRole("list")).toHaveText("Refreshed item");
    await page.evaluate(() => window.pendingPages[0]!.resolve({ items: ["Stale item"] }));
    await page.getByRole("button", { name: "Load more" }).click();
    await page.evaluate(() => window.pendingPages[1]!.resolve({ items: ["Current item"], nextCursor: 2 }));
    await expect(page.getByRole("list")).toHaveText("Refreshed itemCurrent item");
    await page.getByRole("button", { name: "Load more" }).click();
    await page.getByRole("button", { name: "Replace cursor" }).click();
    await expect(page.getByRole("list")).toHaveText("Refreshed item");
    await page.getByRole("button", { name: "Load more" }).click();
    expect(await page.evaluate(() => window.pendingPages[3]!.cursor)).toBe(10);
    await page.evaluate(() => window.pendingPages[2]!.resolve({ items: ["Old cursor item"] }));
    await page.evaluate(() => window.pendingPages[3]!.resolve({ items: ["New cursor item"] }));
    await expect(page.getByRole("list")).toHaveText("Refreshed itemNew cursor item");
});

test("unmount discards an in-flight response without affecting a new list", async ({ page }) => {
    await page.getByRole("button", { name: "Load more" }).click();
    await page.getByRole("button", { name: "Toggle list" }).click();
    await expect(page.getByRole("list")).toHaveCount(0);
    await page.getByRole("button", { name: "Toggle list" }).click();
    await page.getByRole("button", { name: "Load more" }).click();
    await page.evaluate(() => window.pendingPages[0]!.reject(new Error("Old request failure")));
    await page.evaluate(() => window.pendingPages[1]!.resolve({ items: ["New list item"] }));
    await expect(page.getByRole("list")).toHaveText("First itemNew list item");
    await expect(page.getByRole("alert")).toHaveCount(0);
});
