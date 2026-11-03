import { expect, test, type Locator, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const projectId = "00000000-0000-4000-8000-000000000101";
const projectOptions = (page: Page) => page.getByRole("region", { name: "Projects", exact: true });
const backupOptions = (page: Page) => page.getByRole("region", { name: "Backups", exact: true });
const navigation = (page: Page, name: string) => page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });

for (const [locale, labels] of [
    ["en-US", ["0 - Store only", "1 - Fastest compression", "3 - Fast compression", "5 - Normal compression", "7 - Maximum compression", "9 - Ultra compression"]],
    ["zh-CN", ["0 - 仅存储", "1 - 极速压缩", "3 - 快速压缩", "5 - 标准压缩", "7 - 最大压缩", "9 - 极限压缩"]],
    ["ja-JP", ["0 - 無圧縮", "1 - 最速圧縮", "3 - 高速圧縮", "5 - 標準圧縮", "7 - 最高圧縮", "9 - 超圧縮"]]
] as const) {
    test(`six compression choices have the expected labels and can be selected in ${locale}`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/settings&h3=1");
        if (locale !== "en-US") {
            await page.locator(".resource-navigation").getByRole("button").nth(2).click();
            await select(page.locator("#settings-locale"), locale);
            await expect(page.locator("html")).toHaveAttribute("lang", locale);
            await page.locator(".resource-navigation").getByRole("button").first().click();
        }
        const control = page.locator("#settings-backup-compression");
        const writes = await page.evaluate(() => window.settingsWrites.length);
        await expect(control).toHaveJSProperty("value", "fast");
        await control.click();
        await expect(control.locator("md-select-option")).toHaveText([...labels]);
        await control.getByRole("option", { name: labels[4], exact: true }).click();
        await expect(control).toHaveJSProperty("value", "level7");
        await expect.poll(() => page.evaluate(() => window.settingsRequests.at(-1)?.update.backups?.compressionMode)).toBe("level7");
        await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(writes + 1);
    });
}

test("project and backup defaults save independently and remain accepted after navigation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1");
    const discover = projectOptions(page).getByRole("checkbox");
    const packages = backupOptions(page).getByRole("checkbox", { name: "Keep packages by default", exact: true });
    const library = backupOptions(page).getByRole("checkbox", { name: "Keep Library cache by default", exact: true });
    await expect(discover).toBeChecked();
    await expect(packages).toBeChecked();
    await expect(library).not.toBeChecked();
    await expect(page.locator("#settings-backup-compression")).toHaveJSProperty("value", "fast");
    await discover.uncheck();
    await saved(page, 1);
    await select(page.locator("#settings-backup-compression"), "level7");
    await saved(page, 2);
    await packages.uncheck();
    await saved(page, 3);
    await library.check();
    await saved(page, 4);
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([
        { projects: { autoDiscover: false } }, { backups: { compressionMode: "level7" } },
        { backups: { includeVpmPackages: false } }, { backups: { includeLibrary: true } }
    ]);
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await navigation(page, "Settings").click();
    await expect(discover).not.toBeChecked();
    await expect(packages).not.toBeChecked();
    await expect(library).toBeChecked();
    await expect(page.locator("#settings-backup-compression")).toHaveJSProperty("value", "level7");
    await expect(page.getByRole("textbox", { name: "Default project location", exact: true })).toHaveValue("C:\\Fixture");
    await expect(page.getByRole("textbox", { name: "Default backup location", exact: true })).toHaveValue("C:\\Fixture\\Backups");
});

test("new backups inherit saved defaults and a one-time override does not rewrite settings", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1");
    await select(page.locator("#settings-backup-compression"), "level5");
    await saved(page, 1);
    await backupOptions(page).getByRole("checkbox", { name: "Keep packages by default", exact: true }).uncheck();
    await saved(page, 2);
    await backupOptions(page).getByRole("checkbox", { name: "Keep Library cache by default", exact: true }).check();
    await saved(page, 3);
    await navigation(page, "Projects").click();
    await page.getByRole("button", { name: "Manage", exact: true }).click();
    await page.getByRole("navigation", { name: "Project actions", exact: true }).getByRole("button", { name: /More actions for/ }).click();
    await page.getByRole("menuitem", { name: "Backups", exact: true }).click();
    await page.getByRole("button", { name: "Create backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.locator("#backup-compression")).toHaveJSProperty("value", "level5");
    await expect(dialog.getByRole("checkbox", { name: "Keep packages in backups", exact: true })).not.toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "Keep Library cache in backups", exact: true })).toBeChecked();
    await select(dialog.locator("#backup-compression"), "level3");
    await dialog.getByRole("checkbox", { name: "Keep packages in backups", exact: true }).check();
    await dialog.getByRole("checkbox", { name: "Keep Library cache in backups", exact: true }).uncheck();
    await dialog.getByRole("button", { name: "Create backup", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(await page.evaluate(() => window.h3Requests.filter((request) => request.method === "backups.create"))).toEqual([
        { method: "backups.create", params: { projectId, expectedRevision: 2, compressionMode: "level3", excludeVpmPackages: false, includeLibrary: false } }
    ]);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(3);
    await navigation(page, "Settings").click();
    await selectSettingsSection(page, "general");
    await expect(page.locator("#settings-backup-compression")).toHaveJSProperty("value", "level5");
    await expect(backupOptions(page).getByRole("checkbox", { name: "Keep packages by default", exact: true })).not.toBeChecked();
    await expect(backupOptions(page).getByRole("checkbox", { name: "Keep Library cache by default", exact: true })).toBeChecked();
});

test("backup defaults read failure cannot create a backup with guessed values", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}/backups&h3=1&h5-settings=1`);
    await expect(page.getByRole("heading", { level: 1, name: "Backups", exact: true })).toBeVisible();
    await page.evaluate(() => window.h5Settings.setFailure("settings.get", "permission_denied"));
    await page.getByRole("button", { name: "Create backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(page.getByRole("alert")).toContainText("Backup defaults could not be loaded.");
    await expect(dialog.getByRole("button", { name: "Create backup", exact: true })).toBeDisabled();
    expect(await page.evaluate(() => window.h3Requests.filter((request) => request.method === "backups.create"))).toEqual([]);
    await page.evaluate(() => window.h5Settings.setFailure("settings.get"));
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(dialog.locator("#backup-compression")).toHaveJSProperty("value", "fast");
    await expect(dialog.getByRole("checkbox", { name: "Keep packages in backups", exact: true })).toBeChecked();
    await expect(dialog.getByRole("checkbox", { name: "Keep Library cache in backups", exact: true })).not.toBeChecked();
    await expect(dialog.getByRole("button", { name: "Create backup", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.h3Requests.filter((request) => request.method === "backups.create"))).toEqual([]);
});

test("visible project lists follow Core discoveries without an explicit refresh", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/projects&auto-projects=1");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.evaluate(() => {
        const original = window.projectSnapshots.at(-1)![0]!;
        window.autoDiscoveryProjects.push({ ...original, projectId: "00000000-0000-4000-8000-000000000991", rootPath: "C:\\Fixture\\Discovered project" });
    });
    await page.clock.fastForward(3_100);
    await expect(page.getByText("Discovered project", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
});

test("turning off discovery leaves idle lists quiet while committed project changes still update them", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/settings&auto-projects=1");
    await projectOptions(page).getByRole("checkbox").uncheck();
    await saved(page, 1);
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    const before = await page.evaluate(() => window.projectListReads);
    await page.clock.fastForward(10_000);
    expect(await page.evaluate(() => window.projectListReads)).toBe(before);
    await page.evaluate(() => {
        const original = window.projectSnapshots.at(-1)![0]!;
        window.autoDiscoveryProjects.push({ ...original, projectId: "00000000-0000-4000-8000-000000000993", rootPath: "C:\\Fixture\\Registered elsewhere" });
    });
    await page.clock.runFor(500);
    await expect(page.getByText("Registered elsewhere", { exact: true })).toBeVisible();
});

test("a late automatic read cannot overwrite an accepted project change", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/projects&auto-projects=1&hold-project-poll=1");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.evaluate(() => window.coreEvents.emit("project", "00000000-0000-4000-8000-000000000101", "project.favorite_changed", 2));
    await page.clock.runFor(500);
    await expect.poll(() => page.evaluate(() => window.resumeProjectPoll !== undefined)).toBe(true);
    await page.evaluate(() => {
        const original = window.projectSnapshots.at(-1)![0]!;
        window.autoDiscoveryProjects.push({ ...original, projectId: "00000000-0000-4000-8000-000000000992", rootPath: "C:\\Fixture\\Newer project" });
    });
    await page.getByRole("button", { name: "Add to favorites", exact: true }).first().click();
    await expect(page.getByRole("button", { name: "Remove from favorites", exact: true }).first()).toBeVisible();
    await page.evaluate(() => window.resumeProjectPoll!());
    await page.clock.fastForward(100);
    await expect(page.getByRole("button", { name: "Remove from favorites", exact: true }).first()).toBeVisible();
    await page.clock.fastForward(3_100);
    await expect(page.getByText("Newer project", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

test("a held pre-delete project snapshot cannot reinsert a project removed by another client", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/projects&auto-projects=1&hold-project-poll=1");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.evaluate(() => {
        const original = window.projectSnapshots.at(-1)![0]!;
        window.autoDiscoveryProjects.push({ ...original, projectId: "00000000-0000-4000-8000-000000000994", rootPath: "C:\\Fixture\\Deleted before reply" });
    });
    await page.clock.runFor(500);
    await expect.poll(() => page.evaluate(() => window.resumeProjectPoll !== undefined)).toBe(true);
    await page.evaluate(() => {
        window.autoDiscoveryProjects.splice(0);
        window.coreEvents.emit("project", "00000000-0000-4000-8000-000000000994", "project.unregistered", 3);
        const appeared: string[] = [];
        (window as Window & { deletedProjectAppearances?: string[] }).deletedProjectAppearances = appeared;
        new MutationObserver(() => {
            if (document.querySelector("main")?.textContent?.includes("Deleted before reply")) appeared.push("reinserted");
        }).observe(document.querySelector("main")!, { childList: true, subtree: true });
        window.resumeProjectPoll!();
    });
    await page.clock.runFor(1_000);
    await expect(page.getByText("Deleted before reply", { exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => (window as Window & { deletedProjectAppearances?: string[] }).deletedProjectAppearances)).toEqual([]);
    expect(await page.evaluate(() => window.projectListReads)).toBeGreaterThanOrEqual(3);
});

async function select(control: Locator, value: string) {
    await control.evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

async function saved(page: Page, count: number) {
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(count);
}
