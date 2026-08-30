import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

function packageName(page: Page, name: string) {
    return page.getByRole("table", { name: "Packages" }).getByText(name, { exact: true });
}

test("Package refresh is visible and handles zero or one repository with a final reload", async ({ page }) => {
    await openHarness(page, "package-no-repositories");
    const refresh = page.getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeVisible();
    await refresh.click();
    await expect(page.getByText("0 refreshed, 0 failed.")).toBeVisible();

    await openHarness(page, "ready");
    await expect(packageName(page, "Refreshed package")).toHaveCount(0);
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("1 refreshed, 0 failed.")).toBeVisible();
    await expect(packageName(page, "Refreshed package")).toBeVisible();
});

test("Package refresh processes every repository and reports partial failure without fake success", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("2 refreshed, 0 failed.")).toBeVisible();
    await expect(packageName(page, "Refreshed package")).toBeVisible();

    await openHarness(page, "package-partial-failure");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("1 refreshed, 1 failed.");
    await expect(packageName(page, "Refreshed package")).toBeVisible();
    await expect(page.getByText("2 refreshed, 0 failed.")).toHaveCount(0);
});

test("Package refresh retries a revision conflict once and then reloads", async ({ page }) => {
    await openHarness(page, "package-revision-conflict");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("1 refreshed, 0 failed.")).toBeVisible();
    await expect(packageName(page, "Refreshed package")).toBeVisible();
});

test("Package source filter uses daemon source kinds and only changes presentation", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Local tools")).toBeVisible();

    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Remote", exact: true }).click();
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Local tools")).toHaveCount(0);
    await expect(page.getByText("2 packages")).toBeVisible();

    await openHarness(page, "package-multiple");
    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Local repository", exact: true }).click();
    await expect(packageName(page, "Local tools")).toBeVisible();
    await expect(packageName(page, "Remote tools")).toHaveCount(0);
    await expect(page.getByText("1 package")).toBeVisible();
    await expect(page.getByText(/refreshed, .* failed/)).toHaveCount(0);
});

test("User Packages are a visible source choice and reinstall uses one plan review", async ({ page }) => {
    await openHarness(page, "package-user-source");
    const source = page.getByRole("button", { name: "Source for Avatar tools" });
    await expect(source).toBeVisible();
    await source.click();
    await page.getByRole("menuitem", { name: "Local avatar tools", exact: true }).click();
    await expect(page.locator(".package-row-source-menu")).toContainText("Local avatar tools");
    await page.getByRole("button", { name: "More actions for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Reinstall", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();

    await page.getByRole("checkbox", { name: "Select Avatar tools" }).check();
    await expect(page.getByRole("region", { name: "Selected package actions" })).toContainText("1 selected");
    await page.getByRole("button", { name: "Reinstall selected" }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
});

test("Package rows keep one primary action and move secondary actions into Material menus", async ({ page }) => {
    await openHarness(page, "package-multiple");
    const table = page.getByRole("table", { name: "Packages" });
    const installedRow = table.getByRole("row").filter({ hasText: "Avatar tools" });
    const availableRow = table.getByRole("row").filter({ hasText: "Remote tools" });

    await expect(installedRow.getByRole("button", { name: "Update", exact: true })).toBeVisible();
    await expect(installedRow.getByRole("button", { name: "Reinstall", exact: true })).toHaveCount(0);
    await expect(availableRow.getByRole("button", { name: "Install", exact: true })).toBeVisible();

    await installedRow.getByRole("button", { name: "More actions for Avatar tools" }).click();
    await expect(page.getByRole("menuitem", { name: "Reinstall", exact: true })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Choose Version…", exact: true })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove", exact: true })).toBeVisible();
});

test("Project workspace keeps high-frequency actions in context and complete project actions in overflow", async ({ page }) => {
    await openHarness(page, "ready");
    const actions = page.getByRole("navigation", { name: "Project actions" });
    await expect(actions.getByRole("button", { name: "Open Unity" })).toBeVisible();
    await expect(actions.getByRole("button", { name: "Backups" })).toBeVisible();
    await actions.getByRole("button", { name: /More actions for/ }).click();
    await expect(page.getByRole("menuitem", { name: "Open Project Directory" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Copy Project" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Use Automatic Unity Editor" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove from list" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Delete Project Directory…" })).toBeVisible();

    await page.getByRole("menuitem", { name: "Use Automatic Unity Editor" }).click();
    await expect(page.locator(".project-workspace-feedback")).toContainText("Unity editor selection returned to Automatic");
});

test("Project workspace keeps permanent deletion behind its destructive review", async ({ page }) => {
    await openHarness(page, "ready");
    await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }).click();
    await page.getByRole("menuitem", { name: "Delete Project Directory…" }).click();

    const review = page.locator("md-dialog").filter({ hasText: "Permanently delete project directory?" });
    await expect(review).toBeVisible();
    await expect(review).toContainText("does not use the Recycle Bin or Trash");
    await expect(review).toContainText("No automatic backup will be created");
    await expect(review.getByRole("textbox", { name: /Confirm project directory name/ })).toBeVisible();
});

test("User Package management lists, refreshes and removes only the enrollment", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fuser-packages&state=package-user-source");
    await expect(page.getByRole("heading", { name: "Local avatar tools" })).toBeVisible();
    await page.getByRole("article").getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("revision 2")).toBeVisible();
    await page.getByRole("button", { name: "Remove enrollment" }).click();
    await expect(page.getByText("No User Packages enrolled")).toBeVisible();
});

async function openHarness(page: Page, state: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&state=${state}`);
}
