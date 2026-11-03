import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

test.use({ viewport: { width: 1280, height: 800 } });

async function sharedOutlet(page: Page) {
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toHaveCount(1);
    await expect(outlet).toBeVisible();
    const bounds = (await outlet.boundingBox())!;
    expect(bounds.x + bounds.width).toBe(1280 - 24);
    expect(bounds.y + bounds.height).toBe(800 - 24);
    return outlet;
}

test("project favorite failure retains details in the shared outlet and the next action replaces it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=favorite-conflict");
    const toolbar = page.locator(".projects-toolbar");
    const before = await toolbar.boundingBox();
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    const outlet = await sharedOutlet(page);
    await expectRequestError(outlet.getByRole("alert"), "revision_conflict");
    await expect(page.locator(".projects-content").getByRole("alert")).toHaveCount(0);
    expect(await toolbar.boundingBox()).toEqual(before);
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(outlet.getByRole("alert")).toBeVisible();
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(outlet.getByRole("alert")).toHaveCount(0);
    await expect(outlet).toContainText("Project added to favorites.");
    await page.clock.fastForward(5100);
    await expect(outlet).toBeHidden();
});

test("a package link failure is visible through the same outlet and clears on leaving the page", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101&state=package-multiple&package-link-error=1");
    const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: "Avatar tools" });
    await row.getByRole("button", { name: "More actions for Avatar tools", exact: true }).click();
    await page.getByRole("menuitem", { name: "Documentation", exact: true }).click();
    const outlet = await sharedOutlet(page);
    await expect(outlet).toContainText("Package link unavailable:");
    await expectRequestError(outlet.getByRole("alert"), "permission_denied");
    await expect(row.getByRole("alert")).toHaveCount(0);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await expect(outlet.locator(".reminder")).toHaveCount(0);
    await expect(outlet).toBeHidden();
});

test("user package refresh failure keeps retry in the shared outlet without opening a modal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    const before = await row.boundingBox();
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    const outlet = await sharedOutlet(page);
    await expectRequestError(outlet.getByRole("alert"), "store_unavailable");
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    expect(await row.boundingBox()).toEqual(before);
    await outlet.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByText("revision 2", { exact: true })).toBeVisible();
    await expect(outlet.getByRole("alert")).toHaveCount(0);
    await expect(outlet).toContainText("User Package refreshed.");
    expect(await page.evaluate(() => window.resourceRequests.filter((entry) => entry.method === "packages.userPackages.refresh"))).toHaveLength(2);
});

test("a repository removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&state=package-partial-failure&resource-remove-error=1");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: "Local packages" });
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("Repository refresh failed");
    await row.getByRole("button", { name: "Remove", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove repository", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await expect(dialog).toContainText("Packages already installed in projects stay installed.");
});

test("a user package removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1&resource-remove-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("User Package refresh failed");
    await row.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await expect(dialog).toContainText("The source folder will stay on disk.");
});

for (const state of ["failed", "cancelled"]) {
    test(`an immediately ${state} project deletion keeps its real result in the destructive review`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101&state=${state}`);
        await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }).click();
        await page.getByRole("menuitem", { name: "Delete Project Directory…", exact: true }).click();
        const review = page.locator("md-dialog[open]");
        await review.getByRole("textbox", { name: "Confirm project directory name", exact: true }).fill("Sample");
        await review.getByRole("button", { name: "Permanently delete", exact: true }).click();
        await expect(review.getByRole("alert")).toContainText("Unable to delete project directory:");
        if (state === "failed") await expectRequestError(review.getByRole("alert"), "package_archive_invalid");
        else await expect(review.getByRole("alert")).toContainText("cancelled");
        await expect(page.locator(".reminder-floating .reminder--success")).toHaveCount(0);
        await expect(review).toContainText("does not use the Recycle Bin or Trash");
        await expect(review.getByRole("button", { name: "Back", exact: true })).toBeEnabled();
    });
}

test("a completed project deletion closes its review and reloads the project list", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101");
    await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }).click();
    await page.getByRole("menuitem", { name: "Delete Project Directory…", exact: true }).click();
    const review = page.locator("md-dialog[open]");
    await review.getByRole("textbox", { name: "Confirm project directory name", exact: true }).fill("Sample");
    await review.getByRole("button", { name: "Permanently delete", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "No registered projects", exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(page.locator(".reminder-floating .reminder--error")).toHaveCount(0);
});
