import { expect, test, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const requests = (page: Page) => page.evaluate(() => window.nativePickerRequests);

test("adding a project starts at the configured directory and later uses the untouched current input", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&cancel-directory=1&project-directory=C%3A%5CConfigured%5CProjects");
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Add existing project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const field = dialog.getByRole("textbox", { name: "Project location", exact: true });
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue("");
    expect(await requests(page)).toEqual([{ kind: "directory", currentPath: "C:\\Configured\\Projects" }]);
    const current = "\\\\server\\share\\My Project";
    await field.fill(current);
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue(current);
    expect((await requests(page)).at(-1)).toEqual({ kind: "directory", currentPath: current });
});

test("an unavailable configured directory does not prevent adding a project by manual selection", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&settings-read-error=1");
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Add existing project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(dialog.getByRole("textbox", { name: "Project location", exact: true })).toHaveValue("C:\\Fixture\\Avatar");
    expect(await requests(page)).toEqual([{ kind: "directory" }]);
});

for (const mode of ["create", "restore"] as const) {
    test(`${mode} sends the edited target directory and cancellation preserves it`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/projects&cancel-directory=1");
        if (mode === "create") await page.getByRole("button", { name: "Create project", exact: true }).click();
        else {
            await page.getByRole("button", { name: "More project creation options" }).click();
            await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
        }
        const dialog = page.locator("md-dialog[open]");
        const field = dialog.getByLabel(mode === "create" ? "Create target parent" : "Restore target parent", { exact: true });
        await expect(field).toHaveValue("C:\\Fixture");
        await field.fill("D:\\Not Created\\Nested Target");
        await dialog.getByRole("button", { name: "Choose directory", exact: true }).click();
        await expect(field).toHaveValue("D:\\Not Created\\Nested Target");
        expect(await requests(page)).toEqual([{ kind: "directory", currentPath: "D:\\Not Created\\Nested Target" }]);
    });
}

test("User Package directory selection uses its current path and cancellation does not enroll it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1&cancel-directory=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const field = dialog.getByRole("textbox", { name: "Source folder", exact: true });
    await field.fill("D:\\Packages\\Edited Folder");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue("D:\\Packages\\Edited Folder");
    expect(await requests(page)).toEqual([{ kind: "directory", currentPath: "D:\\Packages\\Edited Folder" }]);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "packages.userPackages.enroll"))).toEqual([]);
});

test("repository list selection receives the current filename and cancellation keeps it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    await page.getByRole("button", { name: "Repository list actions", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import list", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const field = dialog.getByRole("textbox", { name: "Repository list file", exact: true });
    await field.fill("D:\\Lists\\custom.txt");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue("D:\\Lists\\custom.txt");
    expect(await requests(page)).toEqual([{ kind: "repositoryList", mode: "open", currentPath: "D:\\Lists\\custom.txt" }]);
});

test("template file import forwards an edited path without consuming it on cancellation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-file-cancel=1");
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const field = dialog.getByRole("textbox", { name: "Template file", exact: true });
    await field.fill("D:\\Templates\\current.alcomdtemplate");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue("D:\\Templates\\current.alcomdtemplate");
    expect(await requests(page)).toEqual([{ kind: "projectTemplate", mode: "open", currentPath: "D:\\Templates\\current.alcomdtemplate" }]);
});

test("template export keeps the chosen directory when the new filename changes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const row = page.getByRole("row").filter({ hasText: "com.vrclearn.alcomd.templates.vrchat.avatars.2022" });
    await row.getByRole("button", { name: /More actions/ }).click();
    await page.getByRole("menuitem", { name: "Export template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("File name", { exact: true }).fill("first.alcomdtemplate");
    await dialog.getByRole("button", { name: "Choose directory", exact: true }).click();
    await expect(dialog.getByLabel("Destination file", { exact: true })).toHaveValue("C:\\Fixture\\first.alcomdtemplate");
    await dialog.getByLabel("File name", { exact: true }).fill("second.alcomdtemplate");
    await expect(dialog.getByLabel("Destination file", { exact: true })).toHaveValue("");
    await dialog.getByRole("button", { name: "Choose directory", exact: true }).click();
    expect((await requests(page)).at(-1)).toEqual({ kind: "projectTemplate", mode: "save", fileName: "second.alcomdtemplate", currentPath: "C:\\Fixture\\first.alcomdtemplate" });
    await expect(dialog.getByLabel("Destination file", { exact: true })).toHaveValue("C:\\Fixture\\second.alcomdtemplate");
});

test("settings directory pickers start at their effective project and backup directories", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&cancel-directory=1");
    await selectSettingsSection(page, "general");
    for (const [id, currentPath] of [["projectDirectory", "C:\\Fixture"], ["backupDirectory", "C:\\Fixture\\Backups"]]) {
        const field = page.locator(`#settings-${id}`);
        await field.locator("..").getByRole("button", { name: "Select", exact: true }).click();
        await expect(field).toHaveJSProperty("value", currentPath);
        expect((await requests(page)).at(-1)).toEqual({ kind: "directory", currentPath });
    }
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

test("copying a project starts at its displayed target and reuses a newly selected directory", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=ready");
    await page.getByRole("button", { name: /More actions for/ }).first().click();
    await page.getByRole("menuitem", { name: "Copy Project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const field = dialog.getByRole("textbox", { name: "Copy project location", exact: true });
    await expect(field).toHaveValue("C:\\Fixture");
    await dialog.getByRole("button", { name: "Choose…", exact: true }).click();
    await expect(field).toHaveValue("C:\\Fixture\\Avatar");
    await dialog.getByRole("button", { name: "Choose…", exact: true }).click();
    expect(await requests(page)).toEqual([
        { kind: "directory", currentPath: "C:\\Fixture" },
        { kind: "directory", currentPath: "C:\\Fixture\\Avatar" }
    ]);
});

test("Unity Hub selection starts at an unsaved draft and cancellation leaves it untouched", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&hub-picker=cancel");
    await selectSettingsSection(page, "unity");
    const region = page.getByRole("region", { name: "Unity Hub Path", exact: true });
    const field = region.getByRole("textbox", { name: "Unity Hub Path", exact: true });
    const currentPath = "D:\\Unsaved Hub\\Unity Hub.exe";
    await field.fill(currentPath);
    await region.getByRole("button", { name: "Select", exact: true }).click();
    await expect(field).toHaveValue(currentPath);
    expect(await requests(page)).toEqual([{ kind: "unityHub", currentPath }]);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});
