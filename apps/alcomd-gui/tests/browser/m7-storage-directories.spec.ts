import { expect, test, type Page } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";

const chosenDirectory = "D:\\项目保存位置\\Projects";
const projectLocation = (page: Page) => page.getByRole("region", { name: "Projects", exact: true });
const backupLocation = (page: Page) => page.getByRole("region", { name: "Backups", exact: true });
const projectField = (page: Page) => projectLocation(page).getByRole("textbox", { name: "Default project location", exact: true });
const writes = (page: Page) => page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.update"));
const navigation = (page: Page, name: string) => page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });

async function openSettings(page: Page, extra = "") {
    await page.goto(`/browser-harness.html?route=/settings&h5-settings=1&${extra}`);
    await expect(projectField(page)).toHaveValue("C:\\Fixture");
}

test("save locations edit only their own setting and Escape discards a draft", async ({ page }) => {
    await openSettings(page);
    await projectField(page).fill(chosenDirectory);
    await projectField(page).press("Enter");
    await expect.poll(() => writes(page)).toHaveLength(1);
    expect((await writes(page))[0]!.params).toEqual({ expectedRevision: 7, update: { paths: { projectDirectory: chosenDirectory } } });
    await expect(projectField(page)).toBeEditable();
    await expect(backupLocation(page).getByRole("textbox", { name: "Default backup location", exact: true })).toHaveValue("C:\\Fixture\\Backups");
    await projectField(page).fill("E:\\Uncommitted");
    await projectField(page).press("Escape");
    await expect(projectField(page)).toHaveValue(chosenDirectory);
    await page.getByRole("heading", { level: 1, name: "Settings", exact: true }).click();
    expect(await writes(page)).toHaveLength(1);
});

test("directory selection cancellation preserves the draft without saving", async ({ page }) => {
    await openSettings(page, "cancel-directory=1");
    await projectField(page).fill("D:\\Draft path");
    await projectLocation(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(projectField(page)).toBeEditable();
    await expect(projectField(page)).toHaveValue("D:\\Draft path");
    expect(await writes(page)).toHaveLength(0);
});

test("long directory drafts stay within the window and composing Enter does not save", async ({ page }) => {
    await page.setViewportSize({ width: 640, height: 800 });
    await openSettings(page, "cancel-directory=1");
    const longPath = `D:\\${"很长的项目目录\\".repeat(35)}Projects`;
    await projectField(page).fill(longPath);
    await projectField(page).evaluate((element) => element.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", isComposing: true, keyCode: 229, bubbles: true, composed: true })));
    expect(await writes(page)).toHaveLength(0);
    await projectLocation(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(projectField(page)).toBeEditable();
    expect(await writes(page)).toHaveLength(0);
    expect(await projectLocation(page).evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await page.getByRole("heading", { level: 1, name: "Settings", exact: true }).click();
    await expect.poll(() => writes(page)).toHaveLength(1);
    expect((await writes(page))[0]!.params).toEqual({ expectedRevision: 7, update: { paths: { projectDirectory: longPath } } });
});

test("selected backup folder saves through Core without changing the project location", async ({ page }) => {
    await openSettings(page);
    await backupLocation(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(backupLocation(page).getByRole("textbox", { name: "Default backup location", exact: true })).toHaveValue("C:\\Fixture\\Avatar");
    await expect.poll(() => writes(page)).toHaveLength(1);
    expect((await writes(page))[0]!.params).toEqual({ expectedRevision: 7, update: { paths: { backupDirectory: "C:\\Fixture\\Avatar" } } });
    await expect(projectField(page)).toHaveValue("C:\\Fixture");
});

test("unknown directory save is reconciled before Retry can write again", async ({ page }) => {
    await openSettings(page);
    await page.evaluate(() => window.h5Settings.failNext("settings.update", "internal_error", true));
    await projectField(page).fill(chosenDirectory);
    await projectField(page).press("Enter");
    const error = page.getByRole("button", { name: "Retry saving settings", exact: true });
    await expect(error).toBeVisible();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(projectLocation(page).getByRole("alert")).toHaveCount(0);
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(error).toHaveCount(0);
    await navigation(page, "Settings").click();
    await error.click();
    await expect(error).toHaveCount(0);
    await expect(projectField(page)).toHaveValue(chosenDirectory);
    expect(await writes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
});

test("a rejected save location can be corrected before the shared Retry", async ({ page }) => {
    await openSettings(page);
    await page.evaluate(() => window.h5Settings.failNext("settings.update", "invalid_request"));
    await projectField(page).fill("relative-directory");
    await projectField(page).press("Enter");
    const error = page.getByRole("button", { name: "Retry saving settings", exact: true });
    await expect(error).toBeVisible();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(projectField(page)).toBeEditable();
    await projectField(page).fill(chosenDirectory);
    await projectField(page).press("Enter");
    await expect.poll(() => writes(page)).toHaveLength(1);
    await error.click();
    await expect(error).toHaveCount(0);
    await expect(projectField(page)).toHaveValue(chosenDirectory);
    expect((await writes(page))[1]!.params).toEqual({ expectedRevision: 7, update: { paths: { projectDirectory: chosenDirectory } } });
});

for (const kind of ["create", "restore", "copy"] as const) {
    test(`${kind} form starts at the configured project directory`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects&project-directory=${encodeURIComponent(chosenDirectory)}`);
        if (kind === "create") await page.getByRole("button", { name: "Create project", exact: true }).click();
        else if (kind === "restore") {
            await selectActionMenuItem(page.getByRole("button", { name: "More project creation options", exact: true }), page.getByRole("menuitem", { name: "Restore from backup", exact: true }));
        } else {
            await selectActionMenuItem(page.getByRole("button", { name: "More actions for <private-project>", exact: true }), page.getByRole("menuitem", { name: "Copy Project", exact: true }));
        }
        const label = kind === "create" ? "Create target parent" : kind === "restore" ? "Restore target parent"
            : "Copy project location";
        await expect(page.locator("md-dialog[open]").getByRole("textbox", { name: label, exact: true })).toHaveValue(chosenDirectory);
    });
}

test("a delayed default directory never replaces the user's target", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects&holdSettings=1&project-directory=${encodeURIComponent(chosenDirectory)}`);
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const parent = page.getByRole("textbox", { name: "Create target parent", exact: true });
    await parent.fill("E:\\Explicit target");
    await expect.poll(() => page.evaluate(() => window.heldSettingsReads)).toBeGreaterThan(0);
    await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
    await expect.poll(() => page.evaluate(() => window.releasedSettingsReads === window.heldSettingsReads)).toBe(true);
    await expect(parent).toHaveValue("E:\\Explicit target");
});
