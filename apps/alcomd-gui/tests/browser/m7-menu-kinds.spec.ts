import { expect, test, type Locator } from "@playwright/test";

const projectId = "00000000-0000-4000-8000-000000000101";
test.use({ viewport: { width: 1440, height: 900 } });

async function actionMenu(menu: Locator) {
    await expect(menu).toHaveAttribute("data-menu-kind", "actions");
    await expect(menu.locator(".menu")).toBeVisible();
    await expect(menu.locator(".menu")).toHaveCSS("border-radius", "12px");
    const items = menu.locator(":scope > md-menu-item");
    expect(await items.count()).toBeGreaterThan(0);
    for (const item of await items.all()) {
        const height = (await item.boundingBox())!.height;
        expect(height).toBeGreaterThanOrEqual(40);
        expect(height).toBeLessThanOrEqual(41); // A destructive group may add its divider.
        await expect(item.locator('[slot="headline"]')).toHaveCSS("font-size", "14px");
    }
}

test("project and template action menus use one compact style including every entry", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}`);
    const projectTrigger = page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ });
    await projectTrigger.click();
    await actionMenu(page.locator('md-menu[data-menu-kind="actions"][open]'));
    await page.keyboard.press("Escape");
    await expect(projectTrigger).toBeFocused();

    await page.goto("/browser-harness.html?route=/templates");
    const templateTrigger = page.getByRole("button", { name: "More actions for Avatar starter", exact: true });
    await templateTrigger.click();
    await actionMenu(page.locator('md-menu[data-menu-kind="actions"][open]'));
    await expect(page.locator('md-menu[open] > md-menu-item').filter({ hasText: "Remove template" })).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await expect(templateTrigger).toBeFocused();
});

test("package maintenance and row actions share compact menus without dispatching on Escape", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}&state=package-multiple`);
    const maintenance = page.getByRole("button", { name: "More package actions", exact: true });
    await maintenance.click();
    await actionMenu(page.locator('md-menu[data-menu-kind="actions"][open]'));
    await page.keyboard.press("Escape");
    await expect(maintenance).toBeFocused();
    const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: "Avatar tools" });
    await row.getByRole("button", { name: "More actions for Avatar tools", exact: true }).click();
    await actionMenu(row.locator(".package-actions-menu"));
    await page.keyboard.press("Escape");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("source and version pickers keep selection spacing and never inherit action menu styling", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}&state=package-multiple&versions=1`);
    await page.getByRole("button", { name: "Source for Avatar tools", exact: true }).click();
    const sources = page.locator('md-menu[data-menu-kind="selection"][open]');
    await expect(sources).not.toHaveClass(/alcomd-action-menu/);
    const automatic = sources.locator("md-menu-item").filter({ hasText: "Automatic source selection" });
    expect((await automatic.boundingBox())!.height).toBe(56);
    await expect(automatic.locator('[slot="headline"]')).toHaveCSS("font-size", "16px");
    await page.keyboard.press("Escape");

    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3", exact: true }).click();
    const versions = page.locator(".package-version-menu");
    await expect(versions).toHaveAttribute("data-menu-kind", "selection");
    await expect(versions).not.toHaveClass(/alcomd-action-menu/);
    const version = versions.locator("md-menu-item").filter({ hasText: "1.1.0 · Local packages" });
    await expect(version).toBeVisible();
    expect((await version.boundingBox())!.height).toBe(56);
    await expect(version.locator('[slot="headline"]')).toHaveCSS("font-size", "16px");
    await page.keyboard.press("Escape");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("the ordinary Select retains its value selection and full-size options", async ({ page }) => {
    await page.goto("/browser-harness.html?material=1");
    const select = page.getByRole("combobox", { name: "Project type", exact: true });
    await select.click();
    const choice = page.locator("md-filled-select md-select-option").filter({ hasText: "World" });
    expect((await choice.boundingBox())!.height).toBe(56);
    await page.getByRole("option", { name: "World", exact: true }).click();
    await expect(page.locator("md-filled-select")).toHaveJSProperty("value", "world");
    await expect(page.locator("md-filled-select md-filled-field .container-overflow")).toHaveCSS("border-radius", "9999px");
});
