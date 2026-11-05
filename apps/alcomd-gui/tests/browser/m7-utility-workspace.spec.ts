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
    await page.getByRole("button", { name: "Update source", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.getByRole("button", { name: "Mutation completed" }).click();
    await expect(page.getByLabel("Snapshot")).toHaveText("Initial snapshot");
    expect(await page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.evaluate(() => window.workspaceReads[1]!.resolve("Before mutation"));
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.evaluate(() => window.workspaceReads[2]!.resolve("After mutation"));
    await expect(page.getByLabel("Snapshot")).toHaveText("After mutation");
    await expect(page.getByRole("button", { name: "Update source", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    await expect(page.getByRole("textbox", { name: "Draft" })).toHaveValue("");
});

test("route changes discard late responses and queued invalidations from the old loader", async ({ page }) => {
    await page.getByRole("button", { name: "Update source", exact: true }).click();
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

test("refresh failures stay with loaded content and keep an explicit retry", async ({ page }) => {
    const snapshot = page.getByLabel("Snapshot");
    await page.getByRole("button", { name: "Update source", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.evaluate(() => window.workspaceReads[1]!.reject({ code: "permission_denied" }));
    const failure = page.locator(".utility-workspace-body").getByRole("alert");
    await expect(failure).toContainText("Refresh failed. The last loaded result is still shown.");
    await expect(snapshot).toHaveText("Initial snapshot");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await failure.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(failure).toContainText("permission_denied");
    await failure.getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.evaluate(() => window.workspaceReads[2]!.resolve("Recovered snapshot"));
    await expect(snapshot).toHaveText("Recovered snapshot");
    await expect(failure).toHaveCount(0);
});
test("dialog errors preserve their draft and remain attached while a nested dialog opens", async ({ page }) => {
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    const draft = page.getByRole("textbox", { name: "Draft", exact: true });
    await draft.fill("Keep the draft");
    await page.getByRole("button", { name: "Report operation failure", exact: true }).click();
    const parent = page.locator("md-dialog[open]").filter({ has: draft });
    await expect(parent.getByRole("alert")).toBeVisible();
    await page.getByRole("button", { name: "Open nested dialog", exact: true }).click();
    const nested = page.locator("md-dialog[open]").filter({ has: page.getByRole("button", { name: "Close nested dialog", exact: true }) });
    await expect(nested.getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await nested.getByRole("button", { name: "Close nested dialog", exact: true }).click();
    await expect(draft).toHaveValue("Keep the draft");
    await parent.getByRole("button", { name: "Recover operation", exact: true }).click();
    await expect(parent.getByRole("alert")).toHaveCount(0);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
});
