import { expect, test, type Page } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { requestErrorMessage } from "../../src/request-errors";
import { expectRequestError } from "./request-error-assertions";

async function openList(page: Page, action: "Import list" | "Export list" | "Compatibility export (.txt)") {
    await selectActionMenuItem(page.getByRole("button", { name: "Repository list actions", exact: true }), page.getByRole("menuitem", { name: action, exact: true }));
    await expect(page.getByRole("dialog", { name: action, exact: true })).toBeVisible();
    return page.locator("md-dialog[open]");
}
async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.resourceRequests.filter((entry) => entry.method === name), method);
}
function expectedImport() {
    return { method: "repositories.importListFile", params: { path: "C:/fixture/repositories.txt", expectedFingerprint: "a".repeat(64), idempotencyKey: expect.any(String) } };
}
async function review(page: Page, candidateCount = 3) {
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([]);
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(host.getByText("Duplicate", { exact: true })).toBeVisible();
    await expect(host.getByText("Invalid", { exact: true })).toBeVisible();
    await expect(host.getByText("Unsupported", { exact: true })).toBeVisible();
    await expect(host.getByRole("button", { name: `Add ${candidateCount} repositories`, exact: true })).toBeEnabled();
    await expect(host.getByRole("checkbox")).toHaveCount(0);
    expect(await requests(page, "repositories.inspect")).toEqual([]);
    expect(await requests(page, "repositories.register")).toEqual([]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
    return host;
}

test("repository rows show only browse, manage and remove, including disabled built-in removal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&builtin-repositories=1");
    const official = page.getByRole("row").filter({ hasText: "Official" });
    const curated = page.getByRole("row").filter({ hasText: "Curated" });
    await expect(official).toContainText("https://vrchat.github.io/packages/index.json");
    await expect(curated).toContainText("https://vrchat-community.github.io/vpm-listing-curated/index.json");
    const rows = page.getByRole("table", { name: "Repositories" }).locator("tbody tr");
    await expect(rows).toHaveCount(3);
    for (const row of await rows.all()) {
        const buttons = row.getByRole("button");
        await expect(buttons).toHaveCount(3);
        await expect(buttons.nth(0)).toHaveAccessibleName("Browse packages");
        await expect(buttons.nth(1)).toHaveAccessibleName("Manage repository");
        await expect(buttons.nth(2)).toHaveAccessibleName("Remove");
    }
    await expect(official.getByRole("button", { name: "Remove", exact: true })).toBeDisabled();
    await expect(curated.getByRole("button", { name: "Remove", exact: true })).toBeDisabled();
    await expect(official.getByRole("button", { name: "Manage repository", exact: true })).toBeDisabled();
    await expect(curated.getByRole("button", { name: "Manage repository", exact: true })).toBeDisabled();
    await expect(official.getByRole("checkbox")).toBeChecked();
    await expect(curated.getByRole("checkbox")).toBeChecked();
    await expect(rows.nth(2).getByRole("button", { name: "Remove", exact: true })).toBeEnabled();
    await official.getByRole("button", { name: "Browse packages" }).click();
    await expect(page.getByRole("dialog", { name: "Official" })).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByRole("button", { name: "Refresh repository", exact: true })).toBeEnabled();
    const headline = page.locator('md-dialog[open] > [slot="headline"]');
    const alignment = await headline.evaluate((element) => {
        const title = element.querySelector(".material-dialog-headline-title")!.getBoundingClientRect();
        const refresh = element.querySelector("md-icon-button")!.getBoundingClientRect();
        return { centerDifference: Math.abs(title.y + title.height / 2 - refresh.y - refresh.height / 2), gap: refresh.x - title.right };
    });
    expect(alignment.centerDifference).toBeLessThan(1);
    expect(alignment.gap).toBeGreaterThan(0);
    await expect(page.locator(".repository-browser-toolbar")).toHaveCount(0);
    await expect(page.locator("md-dialog[open]").getByText("Pending first sync")).toBeVisible();
    expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.locator("md-dialog[open]").getByRole("button", { name: "Avatar tools", exact: true })).toBeVisible();
    await expect(official.getByRole("button", { name: "Manage repository", exact: true })).toBeDisabled();
    expect(await requests(page, "repositories.setDisplayName")).toEqual([]);
});

test("repository visibility persists across resource sections and can be restored", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&builtin-repositories=1");
    const official = page.getByRole("checkbox", { name: "Show packages from Official", exact: true });
    await expect(official).toBeChecked();
    await official.click();
    await expect(official).toBeEnabled();
    await expect(official).not.toBeChecked();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await page.getByRole("button", { name: "Repositories", exact: true }).click();
    await expect(official).not.toBeChecked();
    await official.click();
    await expect(official).toBeEnabled();
    await expect(official).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.map(({ update }) => update.packages?.hiddenRepositoryIds))).toEqual([
        ["7e2233c8-0b3f-4cf2-aeb4-57d3d240b101"], []
    ]);
    expect(await requests(page, "repositories.unregister")).toEqual([]);
});

test("repository rows share the ordinary resource row height", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).locator("tbody tr").first();
    await expect(row.getByRole("button", { name: "Browse packages", exact: true })).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const height = (await row.boundingBox())!.height;
    expect(height).toBe(60);
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    const template = page.getByRole("table", { name: "Templates", exact: true }).locator("tbody tr").first();
    await expect(template).toBeVisible();
    expect((await template.boundingBox())!.height).toBe(height);
});

test("package switches persist per repository across details, pagination and resource sections", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&repository-browser-details=1");
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    const toggle = host.getByRole("checkbox", { name: "Enable Avatar tools from this repository", exact: true });
    await expect(toggle).toBeChecked();
    await toggle.click();
    await expect(toggle).toBeEnabled();
    await expect(toggle).not.toBeChecked();
    await page.screenshot({ path: test.info().outputPath("package-switch-list.png"), animations: "disabled" });
    await host.getByRole("button", { name: "Avatar tools", exact: true }).click();
    await expect(host.getByRole("combobox", { name: "Version", exact: true })).toBeVisible();
    await expect(toggle).not.toBeChecked();
    await page.screenshot({ path: test.info().outputPath("package-switch-detail.png"), animations: "disabled" });
    await host.getByRole("button", { name: "Back to packages", exact: true }).click();
    await expect(toggle).not.toBeChecked();
    await expect(host.getByRole("checkbox", { name: "Enable Remote tools from this repository", exact: true })).toBeChecked();
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await page.getByRole("button", { name: "Repositories", exact: true }).click();
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect(toggle).toBeEnabled();
    await expect(toggle).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.map(({ update }) => update.packages?.hiddenRepositoryPackages))).toEqual([
        [{ repositoryId: "00000000-0000-4000-8000-000000000102", packageIds: ["com.example.avatar"] }], []
    ]);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("package preferences remain editable while their whole repository is closed", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories/00000000-0000-4000-8000-000000000102&repository-browser-details=1");
    const toggle = page.getByRole("checkbox", { name: "Enable Avatar tools from this repository", exact: true });
    await expect(toggle).toBeChecked();
    await toggle.click();
    await expect(toggle).toBeEnabled();
    await page.locator("#primary-navigation .navigation-item").filter({ hasText: "Resources" }).click();
    const repository = page.getByRole("checkbox", { name: /Show packages from/ }).first();
    await expect(repository).toBeChecked();
    await repository.click();
    await expect(repository).toBeEnabled();
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(toggle).not.toBeChecked();
    await toggle.click();
    await expect(toggle).toBeChecked();
    await expect(repository).not.toBeChecked();
});

for (const mode of ["package-filter-denied", "package-filter-conflict"]) {
    test(`package preference ${mode} retains confirmed state and offers reload`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories&state=${mode}`);
        await page.getByRole("button", { name: "Browse packages", exact: true }).click();
        const host = page.locator("md-dialog[open]");
        const toggle = host.getByRole("checkbox", { name: "Enable Avatar tools from this repository", exact: true });
        await expect(toggle).toBeChecked();
        await toggle.click();
        await expect(host.getByRole("button", { name: "Reload filters", exact: true })).toBeVisible();
        await expect(toggle).toBeChecked();
        await expect(toggle).toBeEnabled();
        await host.getByRole("button", { name: "Reload filters", exact: true }).click();
        await expect(toggle).toBeChecked();
    });
}

test("package browsing groups versions and reads only the selected trailing package through its boundary", async ({ page }) => {
    test.setTimeout(45_000);
    await page.goto("/browser-harness.html?route=/repositories&repository-browser-details=1&holdRepositoryDetail=1");
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    const table = host.getByRole("table", { name: "Repository packages", exact: true });
    await expect(table.getByRole("columnheader")).toHaveText(["Enabled", "Package"]);
    await expect(table.locator("tbody tr")).toHaveCount(1);
    await expect(table.locator("tbody")).toHaveText("Avatar tools");
    await table.getByRole("button", { name: "Avatar tools", exact: true }).click();
    await expect(host.getByRole("heading", { name: "Avatar tools", exact: true })).toBeFocused();
    await expect.poll(() => page.evaluate(() => typeof window.resumeRepositoryDetail)).toBe("function");
    await expect(host.getByRole("combobox", { name: "Version", exact: true })).toHaveCount(0);
    await expect(host.getByRole("progressbar", { name: "Loading versions…", exact: true })).toBeVisible();
    await page.evaluate(() => window.resumeRepositoryDetail?.());
    const version = host.getByRole("combobox", { name: "Version", exact: true });
    await expect(version).toBeVisible();
    await expect(host).toContainText("com.example.avatar");
    await expect(host).toContainText("Avatar package description");
    await expect(host).toContainText(">=2022.3");
    expect(await page.evaluate(() => window.repositoryPackageRequests.length)).toBe(2);
    await version.click();
    await expect(host.locator("md-select-option")).toHaveText(["1.0.0", "1.1.0", "2.0.0-beta.1"]);
    await host.getByRole("option", { name: "2.0.0-beta.1", exact: true }).click();
    await expect(host).toContainText("Preview package description");
    await expect(host).toContainText("Prerelease");
    await host.getByRole("button", { name: "Documentation", exact: true }).click();
    await host.getByRole("button", { name: "Changelog", exact: true }).click();
    expect(await requests(page, "openPackageLink")).toEqual([
        { method: "openPackageLink", params: { url: "https://fixture.example/preview-docs" } },
        { method: "openPackageLink", params: { url: "https://fixture.example/preview-changelog" } }
    ]);
    const beforeRefresh = await page.evaluate(() => window.repositoryPackageRequests.length);
    const observed = await host.evaluateHandle((element) => {
        const result = { partial: false, observer: undefined as MutationObserver | undefined };
        result.observer = new MutationObserver(() => {
            if (element.querySelector(".repository-package-detail .loading-state--bar")) result.partial = true;
        });
        result.observer.observe(element, { childList: true, subtree: true });
        return result;
    });
    await host.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.repositoryPackageRequests.length)).toBeGreaterThan(beforeRefresh + 1);
    await expect(host.locator("md-filled-select")).toHaveJSProperty("value", "2.0.0-beta.1");
    await expect(host).toContainText("Preview package description");
    expect(await observed.evaluate((value) => { value.observer?.disconnect(); return value.partial; })).toBe(false);
    await observed.dispose();
    await host.getByRole("button", { name: "Back to packages", exact: true }).click();
    await expect(table.getByRole("button", { name: "Avatar tools", exact: true })).toBeFocused();
    await expect(table.locator("tbody tr")).toHaveCount(2);
    await table.getByRole("button", { name: "Remote tools", exact: true }).click();
    await expect(host.getByRole("combobox", { name: "Version", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.repositoryPackageRequests.at(-1)?.cursor?.packageId)).toBe("com.example.remote");
    await expect(host.getByRole("button", { name: "Documentation", exact: true })).toBeDisabled();
    await expect(host.getByRole("button", { name: "Changelog", exact: true })).toBeDisabled();
    await host.getByRole("button", { name: "Back to packages", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(3);
    await expect(table).toContainText("com.example.unnamed");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("a package detail continuation failure retries without showing an incomplete version selector", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&repository-browser-details=1&repository-browser-next-error=1");
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    await host.getByRole("button", { name: "Avatar tools", exact: true }).click();
    await expect(host.getByRole("button", { name: "Retry loading more", exact: true })).toBeEnabled();
    await expect(host.getByRole("combobox", { name: "Version", exact: true })).toHaveCount(0);
    await host.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(host.getByRole("combobox", { name: "Version", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.repositoryPackageRequests.length)).toBe(3);
});

test("leaving a held package detail discards its late continuation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&repository-browser-details=1&holdRepositoryDetail=1");
    await page.getByRole("button", { name: "Browse packages", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    await host.getByRole("button", { name: "Avatar tools", exact: true }).click();
    await expect.poll(() => page.evaluate(() => typeof window.resumeRepositoryDetail)).toBe("function");
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await page.evaluate(() => window.resumeRepositoryDetail?.());
    await expect(page.getByRole("button", { name: "Enroll folder", exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(page.locator(".repository-package-detail")).toHaveCount(0);
});

test("the pencil saves and clears a user repository display name", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&builtin-repositories=1");
    const official = page.getByRole("row").filter({ hasText: "https://packages.example.invalid/index.json" });
    const edit = official.getByRole("button", { name: "Manage repository", exact: true });
    await edit.click();
    const dialog = page.locator("md-dialog[open]");
    const name = dialog.getByRole("textbox", { name: "Display name", exact: true });
    await name.fill("VRChat SDK");
    await dialog.getByRole("button", { name: "Save name", exact: true }).click();
    await expect(official.getByText("VRChat SDK", { exact: true })).toBeVisible();
    await expect(official.getByRole("button", { name: "Remove", exact: true })).toBeEnabled();
    await edit.click();
    await expect(name).toHaveValue("VRChat SDK");
    await name.fill("");
    await dialog.getByRole("button", { name: "Save name", exact: true }).click();
    await expect(official.getByText("Example packages", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.setDisplayName")).toEqual([
        { method: "repositories.setDisplayName", params: { repositoryId: "00000000-0000-4000-8000-000000000102", expectedRevision: 2, displayName: "VRChat SDK" } },
        { method: "repositories.setDisplayName", params: { repositoryId: "00000000-0000-4000-8000-000000000102", expectedRevision: 3, displayName: null } }
    ]);
});

test("repository visibility and the project package filter share the saved preference", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&state=package-multiple&raw-only-repository-package=1");
    const repository = page.getByRole("checkbox", { name: "Show packages from Example packages", exact: true });
    await expect(repository).toBeChecked();
    await repository.click();
    await expect(repository).toBeEnabled();
    await expect(repository).not.toBeChecked();

    await page.getByRole("navigation", { name: "Primary", exact: true }).getByText("Projects", { exact: true }).click();
    await page.getByRole("button", { name: "Manage", exact: true }).first().click();
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    const filter = page.getByRole("dialog", { name: "Package filters" }).getByRole("checkbox", { name: "Example packages", exact: true });
    await expect(filter).not.toBeChecked();
    const packages = page.getByRole("table", { name: "Packages", exact: true });
    await expect(packages.getByText("Remote tools", { exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
    await filter.click();
    await expect(packages.getByText("Remote tools", { exact: true })).toBeVisible();
    await expect(packages.getByText("Raw-only package", { exact: true })).toHaveCount(0);
});

test("long preview content scrolls independently of actions and closing restores the menu trigger", async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 800 });
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    for (const name of ["Cancel", "Select", "Confirm"]) await expect(host.getByRole("button", { name, exact: true })).toBeInViewport();
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    for (const name of ["Back", "Add 3 repositories"]) await expect(host.getByRole("button", { name, exact: true })).toBeInViewport();
    await expect(host.locator(".file-picker")).toHaveCount(0);
    await host.getByRole("button", { name: "Back", exact: true }).click();
    await host.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("button", { name: "Repository list actions", exact: true })).toBeFocused();
    const exportHost = await openList(page, "Export list");
    for (const name of ["Close", "Preview again", "Save new list file"]) await expect(exportHost.getByRole("button", { name, exact: true })).toBeInViewport();
    expect(await exportHost.locator(".material-dialog-content").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});

test("a typed repository list path must be reviewed again when it changes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Import list");
    const input = host.getByLabel("Repository list file", { exact: true });
    await input.fill("C:/fixture/typed.txt");
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    await expect(input).toHaveCount(0);
    await host.getByRole("button", { name: "Back", exact: true }).click();
    await expect(input).toHaveValue("C:/fixture/typed.txt");
    await input.fill("C:/fixture/changed.txt");
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toHaveCount(0);
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([
        { method: "repositories.inspectListFile", params: { path: "C:/fixture/typed.txt" } },
        { method: "repositories.inspectListFile", params: { path: "C:/fixture/changed.txt" } }
    ]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

test("one confirmation adds every candidate once without downloading a preview", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success");
    const host = await review(page);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 3 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    expect(await requests(page, "repositories.register")).toEqual([]);
    expect(await requests(page, "repositories.inspect")).toEqual([]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
    expect(await requests(page, "repositories.unregister")).toEqual([]);
});

test("large lists are sent to Core in one file request and reload once", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=large");
    const host = await review(page, 67);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect(host.getByText("Added: 67 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
});

test("list import keeps partial failures visible without retrying or claiming rollback", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/repositories&list-import=partial");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 2 · Failed: 1 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    await page.clock.fastForward(10_000);
    await expect(host.getByText(requestErrorMessage("repository_unavailable"), { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeVisible();
});

test("an ordered batch result preserves added, unknown, and unprocessed rows", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=unknown");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 1 · Failed: 0 · Unknown: 1 · Not processed: 1", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect(host.getByText("Result unknown", { exact: true })).toBeVisible();
});

test("stopping forwards the current import key and displays Core outcomes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=hold");
    const host = await review(page, 67);
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect.poll(() => requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    const [submission] = await requests(page, "repositories.importListFile");
    await host.getByRole("button", { name: "Stop import", exact: true }).click();
    const { idempotencyKey } = submission!.params as { idempotencyKey: string };
    await expect.poll(() => requests(page, "repositories.cancelListRegistration")).toEqual([
        { method: "repositories.cancelListRegistration", params: { idempotencyKey } }
    ]);
    await page.evaluate(() => window.dispatchEvent(new Event("test-list-register-ready")));
    await expect(host.getByText("Added: 64 · Failed: 0 · Unknown: 0 · Not processed: 3", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
});

test("transport loss shows an unresolved import without inventing per-line outcomes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=transport");
    const host = await review(page, 67);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect(host.getByRole("alert")).toContainText(requestErrorMessage("daemon_unavailable"));
    await expect(host.getByText("Completed additions are kept. Check the repository list before importing again.", { exact: true })).toBeVisible();
    await expect(host.getByText("Result unknown", { exact: true })).toHaveCount(0);
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
});

test("cancelled list file picker performs no inspection or registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    await expect(host.getByRole("button", { name: "Cancel", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

test("an already registered source is skipped and the batch continues", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=duplicate");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 2 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    await expect(host.getByText("Already added", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
});

test("lists with no candidates cannot start registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=invalid");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(host.getByText("Invalid", { exact: true })).toBeVisible();
    await expect(host.getByRole("button", { name: "Add 0 repositories", exact: true })).toBeDisabled();
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

test("a repository without published checksums adds and displays packages normally", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    const row = host.locator("li").filter({ has: page.getByText("Line 1", { exact: true }) });
    const warning = "Package versions without checksums cannot be installed.";
    await expect(row.getByText("Added", { exact: true })).toBeVisible();
    await expect(row.getByText(warning, { exact: true })).toHaveCount(0);
    await page.goto("/browser-harness.html?route=/repositories/00000000-0000-4000-8000-000000000102");
    await expect(page.getByText("Avatar tools", { exact: true }).first()).toBeVisible();
    await expect(page.getByText(warning, { exact: true })).toHaveCount(0);
});

test("cancelled native save selection never sends a Core file write", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
});

test("export validates a new name near its field and binds it to folder selection", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByLabel("New list file name", { exact: true }).fill("../existing.txt");
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
    await expect(host.locator("#repository-list-file-name")).toHaveJSProperty("error", true);
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
    await host.getByLabel("New list file name", { exact: true }).fill("owner-review.txt");
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByText("Saved 231 repositories (30000 bytes).", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.exportListFile")).toEqual([{ method: "repositories.exportListFile", params: { targetPath: "C:/fixture/owner-review.txt", expectedFingerprint: "1".repeat(64), format:"native" } }]);
});

test("export reviews full Core scope and exclusions and requires address-risk confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Export list");
    await expect(host.getByText("231 repositories can be exported · 0 excluded · 30000 bytes", { exact: true })).toBeVisible();
    await expect(host.getByText("Includes repository sources, display names, order, enabled states and package switches.", {exact:true})).toBeVisible();
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByText("Saved 231 repositories (30000 bytes).", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.exportListFile")).toEqual([{ method: "repositories.exportListFile", params: { targetPath: "C:/fixture/repositories.alcomdrepositories", expectedFingerprint: "1".repeat(64), format:"native" } }]);
});

for (const code of ["repository_list_target_exists", "repository_list_changed", "repository_list_io_error"]) {
    test(`export preserves ${code} in context and never automatically writes again`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories&list-export-error=${code}`);
        const host = await openList(page, "Export list");
        const consent = host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true });
        await consent.check();
        await host.getByRole("button", { name: "Save new list file", exact: true }).click();
        await expectRequestError(host.locator("[role=alert]"), code);
        expect(await requests(page, "repositories.exportListFile")).toHaveLength(1);
        if (code === "repository_list_changed") {
            await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
            await host.getByRole("button", { name: "Preview again", exact: true }).click();
            await expect(consent).not.toBeChecked();
            await consent.check();
        }
        await host.getByRole("button", { name: "Save new list file", exact: true }).click();
        await expect(host.getByText("Saved 231 repositories (30000 bytes).", { exact: true })).toBeVisible();
        const calls = await requests(page, "repositories.exportListFile");
        expect(calls).toHaveLength(2);
        if (code === "repository_list_changed") expect(calls[1]?.params).toEqual({ targetPath: "C:/fixture/repositories.alcomdrepositories", expectedFingerprint: "2".repeat(64), format:"native" });
    });
}


test("compatibility export keeps the old text format and its exclusions", async ({page}) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host=await openList(page,"Compatibility export (.txt)");
    await expect(host.getByText("230 repositories can be exported · 1 excluded · 12000 bytes",{exact:true})).toBeVisible();
    await expect(host.getByText("Local source cannot be represented by this URL list.",{exact:true})).toBeVisible();
    await expect(host.getByLabel("New list file name",{exact:true})).toHaveJSProperty("value","repositories.txt");
    await host.getByRole("checkbox",{name:"I have reviewed the export scope and understand the file can contain credentials.",exact:true}).check();
    await host.getByRole("button",{name:"Save new list file",exact:true}).click();
    expect(await requests(page,"repositories.exportListFile")).toEqual([{method:"repositories.exportListFile",params:{targetPath:"C:/fixture/repositories.txt",expectedFingerprint:"1".repeat(64),format:"legacy"}}]);
});

test("native configuration import reviews existing settings, local sources and package switches", async ({page}) => {
    await page.goto("/browser-harness.html?route=/repositories&native-list=1");
    const host=await openList(page,"Import list");
    await host.getByRole("button",{name:"Select",exact:true}).click();
    await host.getByRole("button",{name:"Confirm",exact:true}).click();
    await expect(host.getByText("Update configuration",{exact:true})).toBeVisible();
    await expect(host.getByText("My repository · Disabled · 1 packages disabled",{exact:true})).toBeVisible();
    await expect(host.getByText("C:/local/repository.json",{exact:true})).toBeVisible();
    await expect(host.getByRole("checkbox",{name:"Overwrite existing repository configuration",exact:true})).toBeChecked();
    await host.getByRole("button",{name:"Apply 2 repository configurations",exact:true}).click();
    await expect(host.getByText("Added: 1 · Updated: 1 · Failed: 0 · Unknown: 0 · Not processed: 0",{exact:true})).toBeVisible();
    expect(await requests(page,"repositories.importListFile")).toEqual([{method:"repositories.importListFile",params:{path:"C:/fixture/repositories.alcomdrepositories",expectedFingerprint:"a".repeat(64),idempotencyKey:expect.any(String),overwriteExisting:true}}]);
});

test("native import can keep existing configuration and add only new sources",async ({page})=>{
    await page.goto("/browser-harness.html?route=/repositories&native-list=1");
    const host=await openList(page,"Import list");
    await host.getByRole("button",{name:"Select",exact:true}).click();
    await host.getByRole("button",{name:"Confirm",exact:true}).click();
    await host.getByRole("checkbox",{name:"Overwrite existing repository configuration",exact:true}).uncheck();
    await expect(host.getByText("1 new repositories will be added. Existing repository configuration is kept.",{exact:true})).toBeVisible();
    await expect(host.getByText("Update configuration",{exact:true})).toHaveCount(0);
    await host.getByRole("button",{name:"Apply 1 repository configurations",exact:true}).click();
    await expect(host.getByText("Added: 1 · Updated: 0 · Failed: 0 · Unknown: 0 · Not processed: 0",{exact:true})).toBeVisible();
    expect(await requests(page,"repositories.importListFile")).toEqual([{method:"repositories.importListFile",params:{path:"C:/fixture/repositories.alcomdrepositories",expectedFingerprint:"a".repeat(64),idempotencyKey:expect.any(String),overwriteExisting:false}}]);
});
