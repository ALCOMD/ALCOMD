import { expect, test } from "@playwright/test";

test("the first delivery keeps the Templates route empty without reading the catalog", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&noProjectTemplates=1");
    await expect(page.getByRole("heading", { name: "Resources", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Templates", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Create template", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.templateRequests)).toEqual([]);
});

test("locked templates show their declaration and have no edit or remove action", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const row = page.getByRole("row").filter({ has: page.getByRole("cell", { name: "com.vrclearn.alcomd.templates.vrchat.avatars.2022", exact: true }) });
    const trigger = row.getByRole("button", { name: "More actions for VRChat Avatars 2022", exact: true });
    await trigger.click();
    await expect(page.locator("md-menu[open] md-menu-item").filter({ hasText: "Replace template" })).toHaveJSProperty("disabled", true);
    await expect(page.locator("md-menu[open] md-menu-item").filter({ hasText: "Remove template" })).toHaveJSProperty("disabled", true);
    await page.getByRole("menuitem", { name: "View template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByLabel("Template ID", { exact: true })).toHaveValue("com.vrclearn.alcomd.templates.vrchat.avatars.2022");
    await expect(dialog.getByLabel("Display name", { exact: true })).toHaveJSProperty("readOnly", true);
    await expect(dialog.getByLabel("Unity version", { exact: true })).toHaveValue("2022.3.22f1");
    await expect(dialog.getByRole("button", { name: "Save template", exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(trigger).toBeFocused();
});

test("creating the first template uses a fresh dialog and Cancel returns focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&state=empty");
    const create = page.getByRole("button", { name: "Create template", exact: true });
    await expect(page.getByText("No templates available", { exact: true })).toBeVisible();
    await create.click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Template ID", { exact: true }).fill("com.example.draft");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(create).toBeFocused();
    await create.click();
    await expect(dialog.getByLabel("Template ID", { exact: true })).toHaveValue("");
});

test("the project template picker displays IDs when names are equal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-existing=1&template-same-name=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.locator("#project-create-template md-select-option")).toHaveCount(6);
    const select = dialog.getByRole("combobox", { name: "Project template", exact: true });
    await select.click();
    await expect(dialog.getByRole("option", { name: "VRChat Avatars 2022 · 2022.3.22f1 (com.vrclearn.alcomd.templates.vrchat.avatars.2022)", exact: true })).toBeVisible();
    await expect(dialog.getByRole("option", { name: "VRChat Worlds 2022 · 2022.3.22f1", exact: true })).toBeVisible();
    await dialog.getByRole("option", { name: "VRChat Avatars 2022 · 2022.3.22f1 (com.example.fixture)", exact: true }).click();
    await expect(select).toContainText("com.example.fixture");
});
