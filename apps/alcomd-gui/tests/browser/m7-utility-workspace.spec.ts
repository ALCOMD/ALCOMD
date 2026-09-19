import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
    await page.goto("/tests/browser/utility-workspace.html");
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(1);
    await page.evaluate(() => window.workspaceReads[0]!.resolve("Initial snapshot"));
    await expect(page.getByLabel("Snapshot")).toHaveText("Initial snapshot");
});

test("mutation invalidations coalesce after a modal draft has been explicitly cancelled", async ({ page }) => {
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await page.getByRole("textbox", { name: "Draft" }).fill("Keep this draft");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toBeHidden();
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.getByRole("button", { name: "Mutation completed" }).click();
    await expect(page.getByRole("button", { name: "Refreshing…" })).toBeDisabled();
    expect(await page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.evaluate(() => window.workspaceReads[1]!.resolve("Before mutation"));
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.evaluate(() => window.workspaceReads[2]!.resolve("After mutation"));
    await expect(page.getByLabel("Snapshot")).toHaveText("After mutation");
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await expect(page.getByRole("textbox", { name: "Draft" })).toHaveValue("");
});

test("route changes discard late responses and queued invalidations from the old loader", async ({ page }) => {
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.getByRole("button", { name: "Mutation completed" }).click();
    await page.getByRole("button", { name: "Change route" }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await expect(page.getByLabel("Snapshot")).toHaveCount(0);
    await page.evaluate(() => window.workspaceReads[2]!.resolve("Second route"));
    await page.evaluate(() => window.workspaceReads[1]!.reject({ code: "old_failure" }));
    await expect(page.getByLabel("Snapshot")).toHaveText("Second route");
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.workspaceReads.length)).toBe(3);
});
