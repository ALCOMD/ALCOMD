import { expect, test, type Page } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { requestErrorMessage } from "../../src/request-errors";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

for (const [packageId, displayName] of [["com.vrchat.avatars", "VRChat SDK - Avatars"], ["com.vrchat.base", "VRChat SDK - Base"]] as const) {
    test(`Removing ${displayName} reviews its actual removal and dependent packages before Apply`, async ({ page }) => {
        await openHarness(page, "ready&removal-sdk&catalogIncomplete=1");
        const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: displayName });
        const remove = row.getByRole("button", { name: "Remove", exact: true });
        await remove.click();
        const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
        const visibleReview = page.getByRole("dialog", { name: "Apply package changes?", exact: true });
        await expect(visibleReview).toBeVisible();
        await expect(review.locator(".change-list")).toContainText(displayName);
        await expect(review.locator(".package-change-name")).toHaveAttribute("title", packageId);
        await expect(review.locator(".change-list")).toHaveCSS("list-style-type", "none");
        const conflicts = review.getByRole("region", { name: "Dependency conflicts", exact: true });
        await expect(conflicts.locator("li")).toHaveCount(1);
        await expect(conflicts).toContainText(`These packages still need ${displayName}:`);
        await expect(conflicts).toContainText("com.example.dependent");
        await expect(conflicts).not.toContainText(">=3.10.0");
        if (packageId === "com.vrchat.base") await expect(conflicts).toContainText("VRChat SDK - Avatars");
        await expect(review.locator('[slot="actions"]')).toContainText("Apply changes");
        await expect(page.getByRole("dialog", { name: "Packages are up to date", exact: true })).toHaveCount(0);
        expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(["remove"]);
        await review.getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(visibleReview).toHaveCount(0);
        await expect(remove).toBeFocused();
        expect(await page.evaluate(() => window.packageRequests.filter(({ method }) => method === "apply"))).toHaveLength(0);
        await remove.click();
        await expect(visibleReview).toBeVisible();
        await review.getByRole("button", { name: "Apply changes", exact: true }).click();
        await expect(visibleReview).toHaveCount(0);
        expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(["remove", "remove", "apply"]);
    });
}

test("Bulk removal includes installed transitive packages even without a complete catalog", async ({ page }) => {
    await openHarness(page, "ready&removal-sdk&catalogIncomplete=1");
    await page.getByRole("checkbox", { name: "Select VRChat SDK - Base", exact: true }).check();
    await page.getByRole("button", { name: "Remove selected", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    const visibleReview = page.getByRole("dialog", { name: "Apply package changes?", exact: true });
    await expect(visibleReview).toBeVisible();
    await expect(review.getByRole("region", { name: "Dependency conflicts" })).toContainText("These packages still need VRChat SDK - Base: VRChat SDK - Avatars and com.example.dependent.");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "bulk", params: { intents: [{ kind: "remove", packageId: "com.vrchat.base" }] } }]);
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(await page.evaluate(() => window.packageRequests.filter(({ method }) => method === "apply"))).toHaveLength(0);
});

test("Package confirmation lists legacy paths and Cancel does not apply", async ({ page }) => {
    await openHarness(page, "ready&legacy-removals");
    await page.getByRole("table", { name: "Packages", exact: true }).getByRole("button", { name: "Install", exact: true }).click();
    const dialog = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(dialog.getByText("The following legacy files and folders will be removed.")).toBeVisible();
    await expect(dialog.getByText("Assets/OldTools", { exact: true })).toBeVisible();
    await expect(dialog.getByText("Assets/OldTools.meta", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(["install"]);
});

test("Package toolbar uses the shared search preset and keeps maintenance in overflow", async ({ page }) => {
    await openHarness(page, "ready");
    const toolbar = page.locator(".package-workspace-toolbar");
    await expect(toolbar.getByRole("heading", { name: "Manage packages" })).toBeVisible();
    await expect(toolbar.locator("md-icon-button").filter({ has: page.locator('[data-icon-name="refresh"]') })).toBeVisible();
    await expect(toolbar.locator("md-filled-text-field.alcomd-search-field")).toBeVisible();
    const packages = page.getByRole("table", { name: "Packages", exact: true });
    await expect(packages).toHaveAttribute("data-sizing", "fixed");
    await expect(packages).toHaveCSS("table-layout", "fixed");
    await expect(toolbar.getByRole("button", { name: "Update All (1)", exact: true })).toBeVisible();
    expect(await toolbar.evaluate((element) => element.getBoundingClientRect().height)).toBeLessThanOrEqual(68);
    const listViewport = page.locator(".package-workspace-list-region > .desktop-list-viewport");
    expect(await listViewport.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await packages.locator("tbody .package-column-actions").evaluateAll((cells) => cells.every((cell) => {
        const bounds = cell.getBoundingClientRect();
        return Array.from(cell.children).every((child) => {
            const childBounds = child.getBoundingClientRect();
            return childBounds.left >= bounds.left && childBounds.right <= bounds.right;
        });
    }))).toBe(true);
    const updatePosition = await packages.getByRole("button", { name: "Update", exact: true }).boundingBox();
    const installPosition = await packages.getByRole("button", { name: "Install", exact: true }).boundingBox();
    expect(updatePosition).not.toBeNull();
    expect(installPosition).not.toBeNull();
    const latestPosition = await packages.locator("tbody .package-column-latest").first().boundingBox();
    expect(updatePosition!.x).toBeGreaterThanOrEqual(latestPosition!.x);
    expect(updatePosition!.x + updatePosition!.width).toBeLessThanOrEqual(latestPosition!.x + latestPosition!.width);
    const removePosition = await packages.getByRole("button", { name: "Remove", exact: true }).boundingBox();
    expect(installPosition?.x).toBe(removePosition?.x);
    await expect(packages.getByRole("button", { name: "More actions for Avatar tools", exact: true })).toBeVisible();
    await expect(packages.getByRole("button", { name: "More actions for Remote tools", exact: true })).toBeVisible();
    await expect(toolbar.getByRole("combobox")).toHaveCount(0);
    await expect(toolbar.getByRole("button", { name: "Resolve", exact: true })).toHaveCount(0);
    await page.screenshot({ path: test.info().outputPath("package-workspace.png"), fullPage: false, animations: "disabled" });
    await selectActionMenuItem(toolbar.getByRole("button", { name: "More package actions" }), page.getByRole("menuitem", { name: "Resolve Dependencies…" }));
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
});

test("Package filters stay anchored, save repository visibility and return keyboard focus", async ({ page }) => {
    await openHarness(page, "package-multiple");
    const trigger = page.getByRole("button", { name: "Filter packages" });
    await trigger.click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("border-radius", "28px");
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
        await expect(page.getByRole("alert")).toContainText(state.endsWith("conflict") ? "Settings changed elsewhere" : requestErrorMessage("permission_denied"));
        await expect(repository).toBeChecked();
        await expect(packageName(page, "Remote tools")).toBeVisible();
        await expect(page.getByRole("button", { name: "Reload filters" })).toBeVisible();
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

test("Package filter selects disable during saving and recover after prerelease confirmation", async ({ page }) => {
    await openHarness(page, "package-multiple&theme-save=hold");
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    const prerelease = panel.getByRole("checkbox", { name: "Show prerelease versions", exact: true });
    const status = panel.getByRole("combobox", { name: "Status", exact: true });
    const source = panel.getByRole("combobox", { name: "Source", exact: true });
    const statusHost = panel.locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: "Status", exact: true }) });
    const sourceHost = panel.locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: "Source", exact: true }) });
    await expect(statusHost).toHaveJSProperty("disabled", false);
    await expect(sourceHost).toHaveJSProperty("disabled", false);
    await prerelease.click();
    const confirmation = page.getByRole("dialog", { name: "Show prerelease versions?", exact: true });
    await expect(confirmation).toBeVisible();
    await page.locator("md-dialog").filter({ hasText: "Show prerelease versions?" }).getByRole("button", { name: "Show prerelease versions", exact: true }).click();
    await expect(confirmation).toBeHidden();
    await expect(panel).toBeVisible();
    await expect(panel.locator("fieldset[disabled]")).toHaveCount(2);
    await expect(prerelease).toBeDisabled();
    await expect(statusHost).toHaveJSProperty("disabled", true);
    await expect(sourceHost).toHaveJSProperty("disabled", true);
    await expect(status).toHaveAttribute("aria-expanded", "false");
    await expect(source).toHaveAttribute("aria-expanded", "false");
    expect(await page.evaluate(() => window.settingsRequests)).toMatchObject([{ update: { packages: { showPrerelease: true } } }]);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);

    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
    await expect(prerelease).toBeChecked();
    await expect(prerelease).toBeEnabled();
    await expect(panel.locator("fieldset[disabled]")).toHaveCount(0);
    await expect(statusHost).toHaveJSProperty("disabled", false);
    await expect(sourceHost).toHaveJSProperty("disabled", false);

    for (const [control, label] of [["Source", "Remote"], ["Status", "Installed"]]) {
        const select = panel.locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: control, exact: true }) });
        const lifecycle = await select.evaluateHandle((element) => ({
            opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
            closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
        }));
        await select.getByRole("combobox").click();
        await lifecycle.evaluate((events) => events.opened);
        await select.getByRole("option", { name: label, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
        await lifecycle.dispose();
        await expect(packageName(page, "Avatar tools")).toBeVisible();
        await expect(packageName(page, "Local tools")).toHaveCount(0);
        if (control === "Source") await expect(packageName(page, "Remote tools")).toBeVisible();
        else await expect(packageName(page, "Remote tools")).toHaveCount(0);
    }
    expect(await page.evaluate(() => window.settingsRequests)).toHaveLength(1);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([{ expectedRevision: 7, revision: 8 }]);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

function packageName(page: Page, name: string) {
    return page.getByRole("table", { name: "Packages" }).getByText(name, { exact: true });
}

test("Package refresh is visible and handles zero or one repository with a final reload", async ({ page }) => {
    await openHarness(page, "package-no-repositories");
    const refresh = page.getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeVisible();
    await refresh.click();
    await expect(refresh).toBeEnabled();
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll"))).toHaveLength(1);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refresh"))).toHaveLength(0);

    await openHarness(page, "ready");
    await expect(packageName(page, "Refreshed package")).toHaveCount(0);
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll").length)).toBe(1);
    await expect(packageName(page, "Refreshed package")).toBeVisible();
});

test("Package refresh attempts every repository and reloads available results without inserting a notice", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll").length)).toBe(1);
    await expect(packageName(page, "Refreshed package")).toBeVisible();

    await openHarness(page, "package-partial-failure");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll").length)).toBe(1);
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(packageName(page, "Refreshed package")).toBeVisible();
    await expect(page.locator(".package-refresh-action")).toHaveAttribute("title", `Refresh failed: ${requestErrorMessage("repository_unavailable")}`);
    await expect(page.getByText(/refreshed, .* failed/)).toHaveCount(0);
});

test("Package refresh delegates a large inventory to one Core request", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&package-refresh-parallel=1`);
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll").length)).toBe(1);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refresh"))).toEqual([]);
    await page.evaluate(() => window.dispatchEvent(new Event("test-package-refresh-ready")));
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
});

test("Package refresh leaves revision retry to Core and reloads its result", async ({ page }) => {
    await openHarness(page, "package-revision-conflict");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refreshAll").length)).toBe(1);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refresh"))).toEqual([]);
    await expect(packageName(page, "Refreshed package")).toBeVisible();
});

test("Package source filter uses daemon source kinds and only changes presentation", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Local tools")).toBeVisible();

    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("combobox", { name: "Source", exact: true }).click();
    await page.getByRole("option", { name: "Remote", exact: true }).click();
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Local tools")).toHaveCount(0);
    await expect(page.getByText("2 packages")).toBeVisible();
    await page.getByRole("combobox", { name: "Status", exact: true }).click();
    await page.getByRole("option", { name: "Available", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.candidateRequests.some((request) =>
        request.view.kind === "summary"
        && request.view.filter?.status === "available"
        && request.view.filter?.source === "remote"
    ))).toBe(true);
    await page.keyboard.press("Escape");
    await page.getByRole("searchbox", { name: "Search packages" }).fill("EXAMPLE PACKAGES");
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await expect(packageName(page, "Avatar tools")).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => window.candidateRequests.some((request) =>
        request.view.kind === "summary"
        && request.view.filter?.search === "EXAMPLE PACKAGES"
        && request.view.filter?.status === "available"
        && request.view.filter?.source === "remote"
        && request.expectedSnapshot !== undefined
    ))).toBe(true);

    await openHarness(page, "package-multiple");
    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("combobox", { name: "Source", exact: true }).click();
    await page.getByRole("option", { name: "Local repository", exact: true }).click();
    await expect(packageName(page, "Local tools")).toBeVisible();
    await expect(packageName(page, "Remote tools")).toHaveCount(0);
    await expect(page.getByText("1 package")).toBeVisible();
    await expect(page.getByText(/refreshed, .* failed/)).toHaveCount(0);
});

test("User Packages are a visible source choice and reinstall uses one plan review", async ({ page }) => {
    await openHarness(page, "package-user-source");
    const source = page.getByRole("combobox", { name: "Source for Avatar tools" });
    await expect(source).toBeVisible();
    await source.click();
    await page.getByRole("option", { name: "Local avatar tools", exact: true }).click();
    await expect(page.locator(".package-source-picker").filter({ has: source })).toHaveJSProperty("displayText", "Local avatar tools");
    await selectActionMenuItem(page.getByRole("button", { name: "More actions for Avatar tools" }), page.getByRole("menuitem", { name: "Reinstall", exact: true }));
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
    await expect(availableRow.getByRole("button", { name: "More actions for Remote tools", exact: true })).toBeVisible();

    await installedRow.getByRole("button", { name: "More actions for Avatar tools" }).click();
    const menu = installedRow.locator(".package-actions-menu");
    await expect(menu.locator(".menu")).toHaveCSS("border-radius", "12px");
    for (const item of await menu.locator("md-menu-item:not(.package-actions-menu-item--danger)").all()) {
        expect((await item.boundingBox())!.height).toBe(40);
        await expect(item.locator('[slot="headline"]')).toHaveCSS("font-size", "14px");
    }
    await expect(page.getByRole("menuitem", { name: "Reinstall", exact: true })).toBeVisible();
    await expect(installedRow.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove", exact: true })).toHaveCount(0);
    await openHarness(page, "package-multiple");
    const reloadedAvailableRow = page.getByRole("table", { name: "Packages" }).getByRole("row").filter({ hasText: "Remote tools" });
    await reloadedAvailableRow.getByRole("button", { name: "More actions for Remote tools", exact: true }).click();
    const availableMenu = reloadedAvailableRow.locator("md-menu");
    await expect(availableMenu.locator("md-menu-item").filter({ hasText: "Reinstall" })).toHaveJSProperty("disabled", true);
    await expect(page.getByRole("menuitem", { name: "Documentation", exact: true })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Changelog", exact: true })).toBeVisible();
    await expect(availableMenu.locator("md-menu-item").filter({ hasText: "Remove" })).toHaveCount(0);
    await expect(reloadedAvailableRow.getByRole("button", { name: "Remove", exact: true })).toHaveCount(0);
});

test("Package removal review presents nullable wire versions as user-facing state", async ({ page }) => {
    await openHarness(page, "ready");
    const row = page.getByRole("table", { name: "Packages" }).getByRole("row").filter({ hasText: "Avatar tools" });
    await row.getByRole("button", { name: "Remove", exact: true }).click();

    const host = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(host).toContainText("Remove Avatar tools.");
    await expect(host).not.toContainText("null");
    await expect(page.locator(".desktop-list-viewport")).toHaveCSS("isolation", "isolate");
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
    await expect(actions.getByRole("group", { name: "Open Unity", exact: true })).toBeVisible();
    await expect(actions.getByRole("button", { name: "Backups" })).toHaveCount(0);
    await actions.getByRole("button", { name: /More actions for/ }).click();
    await expect(page.getByRole("menuitem", { name: "Edit launch arguments" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Backups" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Open Project Directory" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Copy Project" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Remove from list" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "Delete Project Directory…" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: /Automatic Unity Editor/ })).toHaveCount(0);
});

test("Project workspace keeps permanent deletion behind its destructive review", async ({ page }) => {
    await openHarness(page, "ready");
    await selectActionMenuItem(page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }), page.getByRole("menuitem", { name: "Delete Project Directory…" }));

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

test("closing a repository preserves installed facts and makes its source match a missing repository", async ({ page }) => {
    await openHarness(page, "ready&versions=1");
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    const filters = page.getByRole("dialog", { name: "Package filters", exact: true });
    const repository = filters.getByRole("checkbox", { name: "Example packages", exact: true });
    await expect(repository).toBeChecked();
    await repository.click();
    await expect(repository).not.toBeChecked();
    const installed = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.avatar", exact: true }) });
    await expect(installed).toContainText("1.2.3");
    await expect(installed).toContainText("Unknown source");
    await expect(installed.getByRole("combobox", { name: /^Source for/ })).toHaveCount(0);
    await expect(page.getByRole("checkbox", { name: "Select Remote tools", exact: true })).toHaveCount(0);
    await repository.click();
    await expect(repository).toBeChecked();
    await expect(page.getByRole("checkbox", { name: "Select Avatar tools", exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);

    await openHarness(page, "package-no-repositories");
    const missing = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.avatar", exact: true }) });
    await expect(missing).toContainText("1.2.3");
    await expect(missing).toContainText("Unknown source");
});

test("a changed visibility revision cannot reuse source metadata while the new Core read is pending", async ({ page }) => {
    await openHarness(page, "ready&holdCandidateVisibility=1");
    const table = page.getByRole("table", { name: "Packages", exact: true });
    await expect(table.getByText("Avatar tools", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    const repository = page.getByRole("dialog", { name: "Package filters", exact: true }).getByRole("checkbox", { name: "Example packages", exact: true });
    await expect(repository).toBeChecked();
    await repository.click();
    await expect(repository).not.toBeChecked();
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateVisibility)).toBe("function");
    await expect(table.getByText("Avatar tools", { exact: true })).toHaveCount(0);
    await expect(table.getByText("Remote tools", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("status", { name: "Loading", exact: true })).toBeVisible();
    await expect(table.getByRole("combobox", { name: /^Source for/ })).toHaveCount(0);
    await page.evaluate(() => window.resumeCandidateVisibility?.());
    await expect(table.getByRole("checkbox", { name: "Select com.example.avatar", exact: true })).toBeVisible();
    await expect(table).toContainText("1.2.3");
    await expect(table).toContainText("Unknown source");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Inline version selection pins the chosen source and preserves installed state until Apply", async ({ page }) => {
    await openHarness(page, "package-multiple&versions=1");
    const trigger = page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" });
    await trigger.click();
    await expect(page.getByRole("option", { name: "1.2.3", exact: true })).toHaveCount(1);
    await expect(page.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.2.3", exact: true }) })).toHaveJSProperty("selected", true);
    await expect(page.locator("md-select-option").filter({ hasText: "legacy" })).toHaveCount(0);
    await expect(page.locator("md-select-option").filter({ hasText: "9.0.0" })).toHaveCount(0);
    await expect(page.locator("md-select-option").filter({ hasText: "2.0.0-beta.1" })).toHaveCount(0);
    const closed = await page.locator(".package-version-picker").first().evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true })) }));
    await page.keyboard.press("Escape");
    await closed.evaluate(({ event }) => event);
    await closed.dispose();
    await expect(trigger).toBeFocused();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);

    await page.getByRole("combobox", { name: "Source for Avatar tools", exact: true }).click();
    await page.getByRole("option", { name: "Local packages", exact: true }).click();
    await trigger.click();
    await page.getByRole("option", { name: "1.1.0", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review).toContainText("1.2.3 → 1.1.0");
    const requests = await page.evaluate(() => window.packageRequests);
    expect(requests).toHaveLength(1);
    expect(requests[0]).toMatchObject({ method: "downgrade", params: { projectId: PROJECT_ID, expectedRevision: 2, packageId: "com.example.avatar", version: "1.1.0", source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000112" } } });
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(trigger).toBeVisible();
    await page.getByRole("combobox", { name: "Source for Avatar tools", exact: true }).click();
    await page.getByRole("option", { name: "Example packages", exact: true }).click();
    await trigger.click();
    await page.getByRole("option", { name: "1.10.0", exact: true }).click();
    await expect(review).toContainText("1.2.3 → 1.10.0");
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["downgrade", "upgrade"]);
    expect(await page.evaluate(() => window.packageRequests[1]?.params)).toMatchObject({ versionRange: "=1.10.0" });
});

test("Inline prerelease selection uses daemon classification and shows planning failures in a dialog", async ({ page }) => {
    await openHarness(page, "ready&versions=1&planError=1");
    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("checkbox", { name: "Show prerelease versions" }).click();
    const confirmed = await page.locator("md-dialog").filter({ hasText: "Show prerelease versions?" }).evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true })) }));
    await page.locator("md-dialog").filter({ hasText: "Show prerelease versions?" }).getByRole("button", { name: "Show prerelease versions", exact: true }).click();
    await confirmed.evaluate(({ event }) => event);
    await confirmed.dispose();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Package filters", exact: true })).toBeHidden();
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    await page.getByRole("option", { name: "2.0.0-beta.1", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "upgrade", params: { versionRange: "=2.0.0-beta.1", includePrerelease: true } }]);
    const failure = page.locator("md-dialog[open]").filter({ hasText: "Could not prepare package changes" });
    await expect(failure.getByRole("alert")).toContainText("package_source_ambiguous");
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await failure.getByRole("button", { name: "Close", exact: true }).click();
    await expect(failure).toBeHidden();
    await expect(page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" })).toBeVisible();
});

test("a missing package during planning shows a dismissible error and preserves the original action", async ({ page }) => {
    await openHarness(page, "ready&planError=package_not_found");
    const packages = page.getByRole("table", { name: "Packages", exact: true });
    const install = packages.getByRole("button", { name: "Install", exact: true });
    await expect(install).toBeEnabled();
    const content = await page.getByRole("main").innerText();
    const before = await packages.boundingBox();
    await install.click();
    await expect.poll(() => page.evaluate(() => window.packageRequests.length)).toBe(1);
    const failure = page.locator("md-dialog[open]").filter({ hasText: "Could not prepare package changes" });
    await expect(failure.getByRole("alert")).toContainText("package_not_found");
    await expect(failure.getByRole("button", { name: "Apply changes", exact: true })).toHaveCount(0);
    await failure.getByRole("button", { name: "Close", exact: true }).click();
    await expect(failure).toBeHidden();
    await expect(install).toBeEnabled();
    await expect(page.getByRole("main")).toHaveText(content, { useInnerText: true });
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    expect(await packages.boundingBox()).toEqual(before);
    await install.click();
    await expect.poll(() => page.evaluate(() => window.packageRequests.length)).toBe(2);
    expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(["install", "install"]);
});

test("a removal preparation failure shows its reason and can be dismissed before retry", async ({ page }) => {
    await openHarness(page, "package-multiple&planError=plan_stale");
    const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: "Avatar tools" });
    const remove = row.getByRole("button", { name: "Remove", exact: true });
    for (let attempt = 1; attempt <= 2; attempt++) {
        await remove.click();
        const failure = page.locator("md-dialog[open]").filter({ hasText: "Could not prepare package changes" });
        await expect(failure.getByRole("alert")).toContainText("plan_stale");
        await expect(failure).toContainText("Refresh the package list and try again.");
        await expect(failure.getByRole("button", { name: "Apply changes", exact: true })).toHaveCount(0);
        await failure.getByRole("button", { name: "Close", exact: true }).click();
        await expect(failure).toBeHidden();
        await expect(remove).toBeEnabled();
        expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(Array(attempt).fill("remove"));
    }
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
});

test("an Apply failure stays in its existing review and closing it leaves no page feedback", async ({ page }) => {
    await openHarness(page, "stale");
    const packages = page.getByRole("table", { name: "Packages", exact: true });
    await packages.getByRole("button", { name: "Install", exact: true }).click();
    const review = page.locator("md-dialog[open]").filter({ hasText: "Apply package changes?" });
    await expect(review).toBeVisible();
    await review.getByRole("button", { name: "Apply changes", exact: true }).click();
    await expect(review.getByRole("alert")).toContainText("plan_stale");
    await expect(review.getByRole("button", { name: "Apply changes", exact: true })).toBeDisabled();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await review.getByRole("button", { name: "Close", exact: true }).click();
    await expect(review).toHaveCount(0);
    await expect(packages).toBeVisible();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".operation-follow")).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests.map(({ method }) => method))).toEqual(["install", "apply"]);
});

test("Selected installed packages remain in one bulk request when hidden by search", async ({ page }) => {
    await openHarness(page, "package-user-source&twoInstalled=1");
    await page.getByRole("combobox", { name: "Source for Avatar tools" }).click();
    await page.getByRole("option", { name: "Local avatar tools", exact: true }).click();
    await expect(packageName(page, "Remote tools")).toBeVisible();
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await page.getByRole("checkbox", { name: "Select Remote tools", exact: true }).check();
    await page.getByRole("searchbox", { name: "Search packages" }).fill("Remote tools");
    await expect(packageName(page, "Avatar tools")).toHaveCount(0);
    await page.getByRole("button", { name: "Reinstall selected", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review.locator(".change-list")).toContainText("Avatar tools");
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "Remove selected", exact: true }).click();
    await expect(review).toContainText("Remove Avatar tools.");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([
        { method: "bulk", params: { intents: [{ kind: "reinstall", packageId: "com.example.avatar", source: { kind: "user_package", userPackageId: "00000000-0000-4000-8000-000000000113" } }, { kind: "reinstall", packageId: "com.example.remote" }] } },
        { method: "bulk", params: { intents: [{ kind: "remove", packageId: "com.example.avatar" }, { kind: "remove", packageId: "com.example.remote" }] } }
    ]);
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "Clear selection", exact: true }).click();
    await expect(page.getByRole("region", { name: "Selected package actions" })).toHaveCount(0);
});

test("User Package inline downgrade uses authoritative classification and pins its selected source", async ({ page }) => {
    await openHarness(page, "package-user-source");
    await expect(page.getByRole("combobox", { name: "Source for Avatar tools" })).toBeEnabled();
    await page.getByRole("combobox", { name: "Source for Avatar tools" }).click();
    await page.getByRole("option", { name: "Local avatar tools", exact: true }).click();
    await expect(page.locator("md-filled-select.package-source-picker").filter({ has: page.getByRole("combobox", { name: "Source for Avatar tools" }) })).toHaveJSProperty("displayText", "Local avatar tools");
    await expect(page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" })).toBeEnabled();
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    await page.getByRole("option", { name: "1.1.0", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "downgrade", params: { projectId: PROJECT_ID, expectedRevision: 2, packageId: "com.example.avatar", version: "1.1.0", source: { kind: "user_package" } } }]);
});
