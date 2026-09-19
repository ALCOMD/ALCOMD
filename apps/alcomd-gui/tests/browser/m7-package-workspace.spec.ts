import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

test("Package toolbar uses the shared search preset and keeps maintenance in overflow", async ({ page }) => {
    await openHarness(page, "ready");
    const toolbar = page.locator(".package-workspace-toolbar");
    await expect(toolbar.getByRole("heading", { name: "Manage packages" })).toBeVisible();
    await expect(toolbar.locator("md-icon-button").filter({ has: page.locator('[data-icon-name="refresh"]') })).toBeVisible();
    await expect(toolbar.locator("md-filled-text-field.alcomd-search-field")).toBeVisible();
    await expect(toolbar.getByRole("combobox")).toHaveCount(0);
    await expect(toolbar.getByRole("button", { name: "Resolve", exact: true })).toHaveCount(0);
    await toolbar.getByRole("button", { name: "More package actions" }).click();
    await page.getByRole("menuitem", { name: "Resolve Dependencies…" }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
});

test("Package filters stay anchored, save repository visibility and return keyboard focus", async ({ page }) => {
    await openHarness(page, "package-multiple");
    const trigger = page.getByRole("button", { name: "Filter packages" });
    await trigger.click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await expect(panel).toBeVisible();
    expect(await panel.evaluate((element) => element.matches(":popover-open") && element.parentElement === document.body)).toBe(true);
    const remote = panel.getByRole("checkbox").first();
    await expect(remote).toBeChecked();
    await remote.click();
    await expect(remote).not.toBeChecked();
    await expect(packageName(page, "Remote tools")).toHaveCount(0);
    await expect(packageName(page, "Local tools")).toBeVisible();
    await remote.click();
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toHaveCount(0);
});

test("Failed filter persistence keeps the last confirmed visibility and exposes recovery", async ({ page }) => {
    for (const state of ["package-filter-conflict", "package-filter-denied"]) {
        await openHarness(page, state);
        await page.getByRole("button", { name: "Filter packages" }).click();
        const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
        const repository = panel.getByRole("checkbox").first();
        await repository.click();
        await expect(panel.getByRole("alert")).toContainText(state.endsWith("conflict") ? "Settings changed elsewhere" : "permission_denied");
        await expect(repository).toBeChecked();
        await expect(packageName(page, "Remote tools")).toBeVisible();
        await expect(panel.getByRole("button", { name: "Reload filters" })).toBeVisible();
    }
});

test("Showing prereleases requires confirmation and only updates package visibility", async ({ page }) => {
    await openHarness(page, "ready");
    await page.getByRole("button", { name: "Filter packages" }).click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await panel.getByRole("checkbox", { name: "Show prerelease versions" }).click();
    const confirmation = page.getByRole("dialog", { name: "Show prerelease versions?", exact: true });
    await expect(confirmation).toBeVisible();
    const confirmationHost = page.locator("md-dialog").filter({ hasText: "Show prerelease versions?" });
    await confirmationHost.getByRole("button", { name: "Cancel" }).click();
    await expect(panel.getByRole("checkbox", { name: "Show prerelease versions" })).not.toBeChecked();
    await panel.getByRole("checkbox", { name: "Show prerelease versions" }).click();
    await confirmationHost.getByRole("button", { name: "Show prerelease versions", exact: true }).click();
    await expect(panel.getByRole("checkbox", { name: "Show prerelease versions" })).toBeChecked();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toHaveCount(0);
});

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

    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Remote", exact: true }).click();
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Local tools")).toHaveCount(0);
    await expect(page.getByText("2 packages")).toBeVisible();

    await openHarness(page, "package-multiple");
    await page.getByRole("button", { name: "Filter packages" }).click();
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
    await expect(installedRow.getByRole("button", { name: "Version for Avatar tools: 1.2.3" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove", exact: true })).toBeVisible();
});

test("Package removal review presents nullable wire versions as user-facing state", async ({ page }) => {
    await openHarness(page, "ready");
    const row = page.getByRole("table", { name: "Packages" }).getByRole("row").filter({ hasText: "Avatar tools" });
    await row.getByRole("button", { name: "More actions for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Remove", exact: true }).click();

    const host = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(host).toContainText("1.2.3 → Removed");
    await expect(host).not.toContainText("null");
    await expect(page.locator(".material-data-table-scroll")).toHaveCSS("isolation", "isolate");
    const dialogBox = await page.getByRole("dialog", { name: "Apply package changes?" }).boundingBox();
    const applyBox = await host.getByRole("button", { name: "Apply changes" }).boundingBox();
    expect(dialogBox).not.toBeNull();
    expect(applyBox).not.toBeNull();
    expect(applyBox!.x + applyBox!.width).toBeLessThanOrEqual(dialogBox!.x + dialogBox!.width);
});

test("Project workspace keeps high-frequency actions in context and complete project actions in overflow", async ({ page }) => {
    await openHarness(page, "ready");
    const actions = page.getByRole("navigation", { name: "Project actions" });
    await expect(actions.getByRole("button", { name: "Open Unity" })).toBeVisible();
    await expect(actions.getByRole("button", { name: "Backups" })).toBeVisible();
    await actions.getByRole("button", { name: /More actions for/ }).click();
    await expect(page.getByRole("menuitem", { name: "Open Project Directory" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Copy Project" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove from list" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Delete Project Directory…" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: /Automatic Unity Editor/ })).toHaveCount(0);
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
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await expect(row.getByRole("cell").first()).toHaveText("Local avatar toolscom.example.avatar");
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("revision 2")).toBeVisible();
    await page.getByRole("button", { name: "Remove enrollment" }).click();
    await expect(page.getByRole("dialog", { name: "Remove User Package?" })).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await expect(page.getByText("No User Packages enrolled")).toBeVisible();
});

async function openHarness(page: Page, state: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&state=${state}`);
}

test("Inline version selection pins the chosen source and preserves installed state until Apply", async ({ page }) => {
    await openHarness(page, "package-multiple&versions=1");
    const trigger = page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" });
    await trigger.click();
    await expect(page.locator("md-menu-item").filter({ hasText: "1.2.3 · Local packages · Installed" })).toHaveJSProperty("disabled", true);
    for (const version of ["legacy", "9.0.0", "2.0.0-beta.1"]) await expect(page.locator("md-menu-item").filter({ hasText: version }).first()).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await expect(trigger).toBeFocused();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);

    await trigger.click();
    await page.getByRole("menuitem", { name: "1.1.0 · Local packages", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review).toContainText("1.2.3 → 1.1.0");
    const requests = await page.evaluate(() => window.packageRequests);
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({ method: "downgrade", params: { projectId: PROJECT_ID, expectedRevision: 2, packageId: "com.example.avatar", version: "1.1.0", source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000112" } } });
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(trigger).toBeVisible();
    await trigger.click();
    await page.getByRole("menuitem", { name: "1.10.0 · Example packages", exact: true }).click();
    await expect(review).toContainText("1.2.3 → 1.10.0");
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["downgrade", "upgrade"]);
    expect(await page.evaluate(() => window.packageRequests[1]?.params)).toMatchObject({ versionRange: "=1.10.0" });
});

test("Inline prerelease selection uses daemon classification and surfaces plan failure", async ({ page }) => {
    await openHarness(page, "ready&versions=1&planError=1");
    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("checkbox", { name: "Show prerelease versions" }).click();
    await page.locator("md-dialog").filter({ hasText: "Show prerelease versions?" }).getByRole("button", { name: "Show prerelease versions", exact: true }).click();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" }).click();
    await page.getByRole("menuitem", { name: "2.0.0-beta.1 · Example packages · Prerelease", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Package changes were not applied");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "upgrade", params: { versionRange: "=2.0.0-beta.1", includePrerelease: true } }]);
    await expect(page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" })).toBeVisible();
});

test("Selected installed packages remain in one bulk request when hidden by search", async ({ page }) => {
    await openHarness(page, "package-user-source&twoInstalled=1");
    await page.getByRole("button", { name: "Source for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Local avatar tools", exact: true }).click();
    await page.getByRole("button", { name: "Source for Remote tools" }).click();
    await page.getByRole("menuitem", { name: "Example packages", exact: true }).click();
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await page.getByRole("checkbox", { name: "Select Remote tools", exact: true }).check();
    await page.getByRole("searchbox", { name: "Search packages" }).fill("Remote tools");
    await expect(packageName(page, "Avatar tools")).toHaveCount(0);
    await page.getByRole("button", { name: "Reinstall selected", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review).toContainText("com.example.avatar");
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "Remove selected", exact: true }).click();
    await expect(review).toContainText("1.2.3 → Removed");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([
        { method: "bulk", params: { intents: [{ kind: "reinstall", packageId: "com.example.avatar", source: { kind: "user_package", userPackageId: "00000000-0000-4000-8000-000000000113" } }, { kind: "reinstall", packageId: "com.example.remote", source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000102" } }] } },
        { method: "bulk", params: { intents: [{ kind: "remove", packageId: "com.example.avatar" }, { kind: "remove", packageId: "com.example.remote" }] } }
    ]);
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "Clear selection", exact: true }).click();
    await expect(page.getByRole("region", { name: "Selected package actions" })).toHaveCount(0);
});

test("User Package inline downgrade uses authoritative classification and pins its selected source", async ({ page }) => {
    await openHarness(page, "package-user-source");
    await expect(page.getByRole("button", { name: "Source for Avatar tools" })).toBeEnabled();
    await page.getByRole("button", { name: "Source for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Local avatar tools", exact: true }).click();
    await expect(page.locator("md-outlined-button").filter({ has: page.getByRole("button", { name: "Source for Avatar tools" }) })).toHaveText("Local avatar tools");
    await expect(page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" })).toBeEnabled();
    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" }).click();
    await page.getByRole("menuitem", { name: "1.1.0 · Local avatar tools", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "downgrade", params: { projectId: PROJECT_ID, expectedRevision: 2, packageId: "com.example.avatar", version: "1.1.0", source: { kind: "user_package" } } }]);
});
