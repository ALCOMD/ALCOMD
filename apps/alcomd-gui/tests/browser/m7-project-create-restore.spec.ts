import { expect, test } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { expectRequestError } from "./request-error-assertions";

test("Projects Create submits without a template and opens the registered project", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&noProjectTemplates=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("combobox", { name: "Project template", exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Choose directory" }).click();
    await expect(dialog.getByLabel("Create target parent")).toHaveValue("C:\\Fixture\\Avatar");
    await dialog.getByLabel("Create project name").fill("Created Project");
    await dialog.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(page).toHaveURL(/\/projects$/);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
    expect(await page.evaluate(() => window.templateRequests)).toEqual([{
        method: "projects.create", params: { target: { kind: "absolute", parentPath: "C:\\Fixture\\Avatar", name: "Created Project" }, unityVersion: "2022.3.22f1", idempotencyKey: expect.any(String) }
    }]);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Created Project" })).toBeVisible();
});

test("creation accepts an explicit uninstalled Editor version", async ({ page }) => {
    await openHarness(page, "/projects", "unity-zero");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const version = dialog.getByRole("combobox", { name: "Unity version", exact: true });
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveCount(0);
    await version.click();
    await dialog.getByRole("option", { name: "Custom", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Unity version", exact: true }).fill("2022.3.99f1");
    await dialog.getByLabel("Create project name").fill("Uninstalled Version");
    await dialog.getByRole("button", { name: "Create project", exact: true }).click();
    expect(await page.evaluate(() => window.templateRequests.find((request) => request.method === "projects.create")?.params)).toMatchObject({ unityVersion: "2022.3.99f1", template: { id: "com.vrclearn.alcomd.templates.blank", revision: 1 } });
});

test("the catalog exposes each built-in Editor variant and creation uses its fixed version", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=unity-zero&template-existing=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const template = dialog.getByRole("combobox", { name: "Project template", exact: true });
    await expect(template).toContainText("Blank");
    await expect(dialog.locator("#project-create-template md-select-option")).toHaveCount(8);
    await template.click();
    for (const [year, version] of [["2022", "2022.3.22f1"], ["2019", "2019.4.31f1"]]) {
        for (const kind of ["Avatars", "Worlds"]) {
            await expect(dialog.getByRole("option", { name: `VRChat ${kind} ${year} · ${version}`, exact: true })).toHaveCount(1);
        }
    }
    await dialog.getByRole("option", { name: "VRChat Avatars 2019 · 2019.4.31f1", exact: true }).click();
    await expect(dialog.getByRole("combobox", { name: "Unity version", exact: true })).toHaveCount(0);
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveValue("2019.4.31f1");
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveJSProperty("readOnly", true);
    await dialog.getByLabel("Create project name").fill("Legacy Avatar");
    await dialog.getByRole("button", { name: "Create project", exact: true }).click();
    expect(await page.evaluate(() => window.templateRequests.find((request) => request.method === "projects.create")?.params))
        .toMatchObject({ unityVersion: "2019.4.31f1", template: { id: "com.vrclearn.alcomd.templates.vrchat.avatars.2019", revision: 3 } });
});

for (const family of ["Avatars", "Worlds"]) {
    test(`Unity 6 ${family} creation keeps the draft while enabling prerelease visibility`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/projects&hidePrerelease=1");
        await page.getByRole("button", { name: "Create project", exact: true }).click();
        const dialog = page.locator("md-dialog[open]").filter({ has: page.locator("#project-create-template") });
        const template = dialog.getByRole("combobox", { name: "Project template" });
        await expect(template).toContainText("Blank");
        await template.click();
        await dialog.getByRole("option", { name: `VRChat ${family} · 6000.0.67f1`, exact: true }).click();
        await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveValue("6000.0.67f1");
        await dialog.getByLabel("Create project name").fill("Unity 6 sample");
        await dialog.getByRole("button", { name: "Create project", exact: true }).click();
        await expect(dialog.getByRole("alert")).toContainText("prerelease_packages_hidden");
        await dialog.getByRole("button", { name: "Show prerelease versions" }).click();
        await expect(dialog.getByRole("alert")).toHaveCount(0);
        await expect(dialog.getByLabel("Create project name")).toHaveValue("Unity 6 sample");
        expect(await page.evaluate(() => window.settingsRequests.map(({ update }) => update))).toEqual([{ packages: { showPrerelease: true } }]);
        await dialog.getByRole("button", { name: "Create project", exact: true }).click();
        await expect(dialog).toBeHidden();
        const requests = await page.evaluate(() => window.templateRequests.filter((r) => r.method === "projects.create"));
        expect(requests).toHaveLength(2);
        for (const { params } of requests) expect(params).toMatchObject({ name: "Unity 6 sample", unityVersion: "6000.0.67f1", template: { id: "com.vrclearn.alcomd.templates.vrchat." + family.toLowerCase() + ".6000" } });
    });
}

test("creation prefills one compatible Editor but requires a choice for multiple installations", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const version = page.getByRole("combobox", { name: "Unity version", exact: true });
    await expect(version).toContainText("2022.3.22f1");
    await expect(page.getByRole("textbox", { name: "Unity version", exact: true })).toHaveCount(0);
    await version.click();
    await expect(page.getByRole("option", { name: "2022.3.22f1", exact: true })).toHaveCount(1);
    await page.getByRole("option", { name: "Custom", exact: true }).click();
    const custom = page.getByRole("textbox", { name: "Unity version", exact: true });
    await custom.fill("2022.3.22f1");
    await expect(custom).toBeVisible();
    await version.click();
    await page.getByRole("option", { name: "2022.3.22f1", exact: true }).click();
    await expect(custom).toHaveCount(0);
    await openHarness(page, "/projects", "unity-multiple");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("combobox", { name: "Unity version", exact: true })).toBeVisible();
    await expect(page.getByRole("textbox", { name: "Unity version", exact: true })).toHaveCount(0);
    await expect(page.locator("md-dialog[open]").getByRole("button", { name: "Create project", exact: true })).toBeDisabled();
});

test("Projects restores a ZIP outside the default directory and opens the registered project", async ({ page }) => {
    await openHarness(page, "/projects");

    await selectActionMenuItem(page.getByRole("button", { name: "More project creation options" }), page.getByRole("menuitem", { name: "Restore from backup", exact: true }));
    await expect(page.getByRole("dialog", { name: "Restore project", exact: true })).toBeVisible();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(dialog.getByLabel("Backup", { exact: true })).toHaveValue("C:/Outside/Unity Project.zip");
    await dialog.getByLabel("Project name", { exact: true }).fill("Restored Project");
    await dialog.getByRole("button", { name: "Restore project", exact: true }).click();

    await expect(page.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(page).toHaveURL(/\/projects$/);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000109$/, { timeout: 5_000 });
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Restored Project" })).toBeVisible();
});

test("Create and Restore expose cancellation, missing input, and structured conflict errors", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "Create project" }).click();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("dialog", { name: "Create project" })).toHaveCount(0);

    await openHarness(page, "/projects", "empty");
    await selectActionMenuItem(page.getByRole("button", { name: "More project creation options" }), page.getByRole("menuitem", { name: "Restore from backup", exact: true }));
    await expect(page.locator("md-dialog[open]").getByRole("button", { name: "Restore project", exact: true })).toBeDisabled();
    const restoreDialogBox = await page.getByRole("dialog", { name: "Restore project" }).boundingBox();
    const closeBox = await page.getByRole("button", { name: "Cancel", exact: true }).boundingBox();
    expect(restoreDialogBox).not.toBeNull();
    expect(closeBox).not.toBeNull();
    expect(closeBox!.x + closeBox!.width).toBeLessThanOrEqual(restoreDialogBox!.x + restoreDialogBox!.width);

    await openHarness(page, "/projects", "create-error");
    await page.getByRole("button", { name: "Create project" }).click();
    await page.getByRole("button", { name: "Choose directory" }).click();
    await page.getByLabel("Create project name").fill("Conflict");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Create project", exact: true }).click();
    await expectRequestError(page.getByRole("alert"), "project_create_target_exists");

    await openHarness(page, "/projects", "restore-error");
    await selectActionMenuItem(page.getByRole("button", { name: "More project creation options" }), page.getByRole("menuitem", { name: "Restore from backup", exact: true }));
    await page.locator("md-dialog[open]").getByRole("button", { name: "Select", exact: true }).click();
    await page.getByLabel("Project name", { exact: true }).fill("Conflict");
    await page.getByRole("button", { name: "Restore project", exact: true }).click();
    await expectRequestError(page.getByRole("alert"), "backup_target_exists");
});

async function openHarness(page: import("@playwright/test").Page, route: string, state = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}
