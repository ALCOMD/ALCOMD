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

test("refresh failures use the shared outlet without moving loaded content and keep an explicit retry", async ({ page }) => {
    const snapshot = page.getByLabel("Snapshot");
    const before = await snapshot.boundingBox();
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(2);
    await page.evaluate(() => window.workspaceReads[1]!.reject({ code: "permission_denied" }));
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toHaveCount(1);
    await expect(outlet.getByRole("alert")).toContainText("Refresh failed. The last loaded result is still shown.");
    await expect(snapshot).toHaveText("Initial snapshot");
    expect(await snapshot.boundingBox()).toEqual(before);
    await expect(page.locator(".utility-workspace-body").getByRole("alert")).toHaveCount(0);
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(outlet.getByRole("alert")).toBeVisible();
    await outlet.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(outlet.getByText("permission_denied", { exact: true })).toBeVisible();
    await outlet.getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.workspaceReads.length)).toBe(3);
    await page.evaluate(() => window.workspaceReads[2]!.resolve("Recovered snapshot"));
    await expect(snapshot).toHaveText("Recovered snapshot");
    await expect(outlet.locator(".reminder")).toHaveCount(0);
    await expect(outlet).toBeHidden();
});

test("one reminder outlet follows nested native dialogs and remains interactive without moving their content", async ({ page }) => {
    await page.getByRole("button", { name: "Edit", exact: true }).click();
    const draft = page.getByRole("textbox", { name: "Draft", exact: true });
    await draft.click();
    const before = await draft.boundingBox();
    await page.getByRole("button", { name: "Report operation failure", exact: true }).click();
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toHaveCount(1);
    await expect(outlet.getByRole("alert")).toBeVisible();
    expect(await draft.boundingBox()).toEqual(before);
    await page.getByRole("button", { name: "Open nested dialog", exact: true }).click();
    const nested = page.locator("md-dialog[open]").filter({ has: page.getByRole("button", { name: "Close nested dialog", exact: true }) });
    await expect(nested.locator(".reminder-floating")).toHaveCount(1);
    await expect(outlet).toHaveCount(1);
    await outlet.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(outlet.getByText("permission_denied", { exact: true })).toBeVisible();
    const bounds = (await outlet.boundingBox())!;
    expect(bounds.x + bounds.width).toBe(page.viewportSize()!.width - 24);
    expect(bounds.y + bounds.height).toBe(page.viewportSize()!.height - 24);
    await nested.getByRole("button", { name: "Close nested dialog", exact: true }).click();
    await expect(page.locator("md-dialog[open]")).toHaveCount(1);
    await expect(page.locator("md-dialog[open] .reminder-floating")).toHaveCount(1);
    await outlet.getByRole("button", { name: "Recover operation", exact: true }).click();
    await expect(outlet.getByRole("alert")).toHaveCount(0);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("body > .reminder-floating")).toHaveCount(1);
    await expect(outlet).toBeHidden();
});
