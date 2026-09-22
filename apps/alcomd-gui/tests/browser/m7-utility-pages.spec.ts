import { expect, test, type Page } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("Resources sections and detail return controls work through ordinary clicks", async ({ page }) => {
    await openHarness(page, "/projects");
    await navigationItem(page, "Resources").click();
    await expectHeading(page, "Repositories");
    const resources = page.getByRole("navigation", { name: "Resources", exact: true });
    await expect(resources.getByRole("button", { name: "Repositories", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Repositories", exact: true }).getByRole("columnheader")).toHaveText(["Repository", "Source", "Actions"]);
    await page.getByRole("button", { name: "Browse packages" }).click();
    const repositoryDialog = page.getByRole("dialog", { name: "Example packages", exact: true });
    const repositoryHost = dialogHost(page, "Example packages");
    await expect(repositoryDialog).toBeVisible();
    await expect(repositoryHost.getByRole("table", { name: "Repository packages" }).getByRole("columnheader")).toHaveText(["Package", "Version", "Status"]);
    const modalBounds = await repositoryDialog.boundingBox();
    const firstColumn = await repositoryHost.getByRole("columnheader", { name: "Package", exact: true }).boundingBox();
    const closeBounds = await repositoryHost.getByRole("button", { name: "Close", exact: true }).boundingBox();
    expect(modalBounds).not.toBeNull();
    expect(firstColumn).not.toBeNull();
    expect(closeBounds).not.toBeNull();
    expect(firstColumn!.x).toBeGreaterThanOrEqual(modalBounds!.x);
    expect(closeBounds!.x + closeBounds!.width).toBeLessThanOrEqual(modalBounds!.x + modalBounds!.width);
    await page.screenshot({ path: "../../target/m7-v3-recheck-20260919/browser/repository-packages-dialog.png", fullPage: false, animations: "disabled" });
    await repositoryHost.getByRole("button", { name: "Close", exact: true }).click();
    await expect(repositoryDialog).not.toBeVisible();
    await expectHeading(page, "Repositories");
    await resources.getByRole("button", { name: "User Packages", exact: true }).click();
    await expectHeading(page, "User Packages");
    await expect(resources.getByRole("button", { name: "User Packages", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("button", { name: "Enroll folder" })).toBeVisible();
    await resources.getByRole("button", { name: "Templates", exact: true }).click();
    await expectHeading(page, "Templates");
    await expect(resources.getByRole("button", { name: "Templates", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Templates" }).getByRole("columnheader")).toHaveText(["Template", "ID", "Last modified", "Source", "Actions"]);
    await page.getByRole("button", { name: /^More actions for / }).click();
    await page.getByRole("menuitem", { name: "View template", exact: true }).click();
    await expectHeading(page, "Template detail");
    await expect(page.getByRole("button", { name: "Favorite", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Remove template", exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    await expectHeading(page, "Templates");
    await resources.getByRole("button", { name: "Repositories", exact: true }).click();
    await expectHeading(page, "Repositories");
});

test("Settings follows supported v3 group order while Logs retains its two real read surfaces", async ({ page }) => {
    await openHarness(page, "/projects");
    await navigationItem(page, "Settings").click();
    await expectHeading(page, "Settings");
    await expect(page.getByRole("navigation", { name: "Settings sections" })).toHaveCount(0);
    await expect(page.locator(".settings-section-heading h2")).toHaveText(["Unity installations", "Packages", "Appearance", "Theme", "System information"]);
    await expect(page.getByRole("table", { name: "Unity installations" }).getByRole("columnheader")).toHaveText(["Version", "Path", "Source"]);
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    const unityDialog = page.getByRole("dialog", { name: "Manage Unity installations" });
    await expect(unityDialog).toBeVisible();
    await expect(dialogHost(page, "Manage Unity installations").getByLabel("Unity executable", { exact: true })).toBeVisible();
    await dialogHost(page, "Manage Unity installations").getByRole("button", { name: "Cancel", exact: true }).click();
    await navigationItem(page, "Logs").click();
    await expectHeading(page, "Activity");
    const logs = page.getByRole("navigation", { name: "Logs", exact: true });
    await expect(page.getByRole("table", { name: "Activity", exact: true }).getByRole("columnheader")).toHaveText(["Time", "Status", "Activity", "Target", "Actions"]);
    await logs.getByRole("button", { name: "Diagnostics" }).click();
    await expectHeading(page, "Diagnostics");
    await expect(logs.getByRole("button", { name: "Diagnostics", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true }).getByRole("columnheader")).toHaveText(["Time", "Severity", "Subsystem", "Diagnostic", "Actions"]);
    await logs.getByRole("button", { name: "Activity", exact: true }).click();
    await expectHeading(page, "Activity");
});

test("utility actions use modal dialogs and cancel clears drafts and returns focus", async ({ page }) => {
    const cases = [
        ["/repositories", "Add repository", "Repository URL", "https://example.invalid/preserved.json"],
        ["/templates", "Import template", "Template bundle", "C:\\Fixture\\preserved.alcomdtemplate"],
        ["/extensions", "Install extension", "Extension package", "C:\\Fixture\\preserved.alcomdext"],
        ["/unity", "Manage installations", "Unity executable", "C:\\Fixture\\Unity.exe"]
    ] as const;
    for (const [route, title, field, value] of cases) {
        await openHarness(page, route);
        const control = page.getByLabel(field, { exact: true });
        await expect(control).toHaveCount(0);
        const trigger = page.getByRole("button", { name: title, exact: true });
        await trigger.click();
        const dialog = page.getByRole("dialog", { name: title, exact: true });
        await expect(dialog).toBeVisible();
        await expect(dialogHost(page, title).getByLabel(field, { exact: true })).toBeVisible();
        await control.fill(value);
        await page.screenshot({ path: `../../target/m7-v3-recheck-20260919/browser/${route.slice(1)}-action-dialog.png`, fullPage: false, animations: "disabled" });
        await dialogHost(page, title).getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(dialog).not.toBeVisible();
        await expect(control).toHaveCount(0);
        await expect(trigger).toBeFocused();
        await trigger.click();
        await expect(control).toHaveValue("");
        await page.keyboard.press("Escape");
        await expect(dialog).not.toBeVisible();
        await expect(trigger).toBeFocused();
    }
});

test("desktop Settings uses language and theme rows with separate animation and compact controls", async ({ page }) => {
    await openHarness(page, "/settings");
    await expectHeading(page, "Settings");
    const fields = page.locator(".settings-editor md-outlined-select");
    await expect(fields).toHaveCount(3);
    await expect(page.getByRole("checkbox", { name: "GUI animation", exact: true })).toBeVisible();
    await expect(page.getByRole("checkbox", { name: "Compact GUI", exact: true })).toBeVisible();
    await expect(page.locator("#settings-density, #settings-motion")).toHaveCount(0);
    const bounds = await fields.evaluateAll((elements) => elements.map((element) => {
        const rect = element.getBoundingClientRect();
        return { id: element.id, left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom };
    }));
    for (let index = 0; index < bounds.length; index++) {
        const current = bounds[index]!;
        expect(current.left, current.id).toBeGreaterThanOrEqual(260);
        expect(current.right, current.id).toBeLessThanOrEqual(1440);
        for (const other of bounds.slice(index + 1)) {
            const overlap = Math.min(current.right, other.right) > Math.max(current.left, other.left)
                && Math.min(current.bottom, other.bottom) > Math.max(current.top, other.top);
            expect(overlap, `${current.id} must not overlap ${other.id}`).toBe(false);
        }
    }
    const appearanceRows = page.getByRole("region", { name: "Appearance", exact: true }).locator(".settings-section-content > div");
    await expect(appearanceRows).toHaveCount(3);
    const rows = await appearanceRows.evaluateAll((elements) => elements.map((element) => {
        const rect = element.getBoundingClientRect();
        return { top: rect.top, bottom: rect.bottom };
    }));
    expect(rows[0]!.bottom).toBeLessThanOrEqual(rows[1]!.top);
    expect(rows[1]!.bottom).toBeLessThanOrEqual(rows[2]!.top);
});

test("unsaved Settings blocks snapshot refresh and cancelling navigation preserves the draft", async ({ page }) => {
    await openHarness(page, "/settings");
    const compact = page.getByRole("checkbox", { name: "Compact GUI", exact: true });
    await expect(compact).not.toBeChecked();
    await compact.check();
    const refresh = page.locator(".utility-workspace-toolbar").getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeDisabled();
    await navigationItem(page, "Projects").click();
    const discard = page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true });
    await expect(discard).toBeVisible();
    await dialogHost(page, "Discard unsaved changes?").getByRole("button", { name: "Keep editing", exact: true }).click();
    await expect(discard).not.toBeVisible();
    await expectHeading(page, "Settings");
    await expect(compact).toBeChecked();
    await expect(refresh).toBeDisabled();
    await page.getByRole("button", { name: "Discard changes", exact: true }).click();
    await expect(compact).not.toBeChecked();
    await expect(refresh).toBeEnabled();
    await navigationItem(page, "Projects").click();
    await expectHeading(page, "Projects");
});

test("utility desktop evidence includes the full application window", async ({ page }) => {
    const directory = "../../target/m7-v3-recheck-20260919/browser";
    const routes = [
        ["repositories", "/repositories", "Repositories"],
        ["repository", "/repositories/00000000-0000-4000-8000-000000000102", "Repository"],
        ["user-packages", "/user-packages", "User Packages"],
        ["templates", "/templates", "Templates"],
        ["template", "/templates/com.cqmhv.template.avatar", "Template detail"],
        ["settings", "/settings", "Settings"],
        ["unity", "/unity", "Unity"],
        ["backups", "/projects/00000000-0000-4000-8000-000000000101/backups", "Backups"],
        ["backup", "/backups/00000000-0000-4000-8000-000000000104", "Backup detail"],
        ["tasks", "/operations", "Task Center"],
        ["task", "/operations/00000000-0000-4000-8000-000000000105", "Operation detail"],
        ["extensions", "/extensions", "Extensions"],
        ["extension", "/extensions/com.cqmhv.discord", "Extension detail"],
        ["activity", "/activity", "Activity"],
        ["diagnostics", "/diagnostics", "Diagnostics"],
        ["about", "/about", "About"]
    ] as const;
    for (const [name, route, heading] of routes) {
        await openHarness(page, route);
        await expectHeading(page, heading);
        await expect(page.getByRole("main").locator(".route-state--loading")).toHaveCount(0);
        await expect(page.getByRole("main").locator(".route-state--error")).toHaveCount(0);
        await page.evaluate(() => document.fonts.ready);
        await page.screenshot({ path: `${directory}/${name}.png`, fullPage: false, animations: "disabled" });
    }
    await page.goto("/browser-harness.html?route=%2Fuser-packages&state=package-user-source");
    await expectHeading(page, "User Packages");
    await expect(page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" })).toBeVisible();
    await page.screenshot({ path: `${directory}/user-packages-populated.png`, fullPage: false, animations: "disabled" });
});

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
}

function dialogHost(page: Page, title: string) {
    // The accessible native dialog lives inside the Material host's shadow root;
    // form controls are slotted light DOM and must be located from that host.
    return page.locator("md-dialog").filter({ has: page.getByRole("heading", { name: title, exact: true }) });
}

async function expectHeading(page: Page, name: string) {
    await expect(page.getByRole("heading", { level: 1, name, exact: true })).toBeVisible();
}

function navigationItem(page: Page, name: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) })
    });
}
