import { expect, test } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

test("repository success feedback never changes table or toolbar geometry", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const table = page.getByRole("table", { name: "Repositories", exact: true });
    const row = table.getByRole("row").filter({ hasText: "Example packages" });
    await expect(row).toBeVisible();
    await page.screenshot({ animations: "disabled" });
    const before = await Promise.all([table.boundingBox(), page.locator(".utility-workspace-toolbar").boundingBox(), row.boundingBox()]);
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository refreshed." })).toBeVisible();
    await expect(row.getByRole("status")).toHaveCount(0);
    const after = await Promise.all([table.boundingBox(), page.locator(".utility-workspace-toolbar").boundingBox(), row.boundingBox()]);
    for (let index = 0; index < before.length; index++) {
        for (const key of ["x", "y", "width", "height"] as const) {
            expect(Math.abs(after[index]![key] - before[index]![key])).toBeLessThanOrEqual(1);
        }
    }
});

test("repository failure stays in its dialog and closing clears the previous operation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&state=package-partial-failure");
    const table = page.getByRole("table", { name: "Repositories", exact: true });
    const row = table.getByRole("row").filter({ hasText: "Local packages" });
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Repository refresh failed", exact: true });
    const host = page.locator("md-dialog[open]");
    await expect(dialog).toBeVisible();
    await expect(row.getByRole("alert")).toHaveCount(0);
    await expectRequestError(host.getByRole("alert"), "repository_unavailable");
    await page.clock.install();
    await page.clock.fastForward(10_000);
    await expect(host.getByRole("alert")).toBeVisible();
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toBeHidden();
    await row.getByRole("button", { name: "Remove", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Remove this repository?", exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByRole("alert")).toHaveCount(0);
});

test("success reminders replace each other, expire, and do not survive a new page visit", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/repositories");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: "Example packages" });
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository refreshed." })).toBeVisible();
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository refreshed." })).toHaveCount(1);
    await page.clock.fastForward(5100);
    await expect(page.getByText("Repository refreshed.", { exact: true })).toHaveCount(0);
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByText("Repository refreshed.", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await expect(page.getByText("Repository refreshed.", { exact: true })).toHaveCount(0);
});

test("a late refresh completion cannot add feedback to another Resources section", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&hold-resource-refresh=1");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: "Example packages" });
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(row.getByRole("button", { name: "Refreshing repository", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("test-resource-refresh-ready")));
    await expect(page.getByText("Repository refreshed.", { exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.resourceRequests.filter((entry) => entry.method === "repositories.refresh"))).toHaveLength(1);
});

test("long repository paths wrap in review while all actions remain reachable", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await page.getByRole("button", { name: "Add repository", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    await host.getByLabel("Repository URL", { exact: true }).fill(`https://fixture.example/${"long-path-segment".repeat(90)}/repository.json`);
    await host.getByRole("button", { name: "Review repository", exact: true }).click();
    await expect(host.locator(".dialog-summary")).toBeVisible();
    const cells = host.locator(".dialog-summary dd");
    expect(await cells.evaluateAll((items) => items.every((item) => item.scrollWidth <= item.clientWidth + 1))).toBe(true);
    for (const name of ["Cancel", "Back", "Add repository"]) await expect(host.getByRole("button", { name, exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.resourceRequests.filter((entry) => entry.method === "repositories.register"))).toEqual([]);
});
