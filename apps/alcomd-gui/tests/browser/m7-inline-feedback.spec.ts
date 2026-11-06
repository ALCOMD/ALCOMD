import { expect, test } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";
test.use({ viewport: { width: 1280, height: 800 } });

test("favorite failure leaves the row intact and the original action retries it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=favorite-conflict");
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("button", { name: "Remove from favorites", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
});
test("package link failure adds no prompt to its row or the next page", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101&state=package-multiple&package-link-error=1");
    const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: "Avatar tools" });
    await row.getByRole("button", { name: "More actions for Avatar tools", exact: true }).click();
    await page.getByRole("menuitem", { name: "Documentation", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
});
test("user package refresh retries through the same control and reloads real revision", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("revision 2", { exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.resourceRequests.filter((r) => r.method === "packages.userPackages.refresh"))).toHaveLength(2);
});

test("a repository removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&state=package-partial-failure&resource-remove-error=1");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: "Local packages" });
    await row.getByRole("button", { name: "Manage repository", exact: true }).click();
    const management = page.locator("md-dialog[open]");
    await management.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await management.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await row.getByRole("button", { name: "Remove", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove repository", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await expect(dialog).toContainText("Packages already installed in projects stay installed.");
});

test("a user package removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1&resource-remove-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await row.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
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
