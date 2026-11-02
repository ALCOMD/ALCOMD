import { expect, test, type Page } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("Templates creation split button exposes derive and import with independent drafts and focus", async ({ page }, testInfo) => {
    await templates(page);
    const group = page.getByRole("group", { name: "Create template", exact: true });
    const primary = group.getByRole("button", { name: "Create template", exact: true });
    const arrow = group.getByRole("button", { name: "More template creation options", exact: true });
    const primaryBox = await group.locator(".alcomd-split-button-primary").boundingBox();
    const arrowBox = await group.locator(".alcomd-split-button-menu").boundingBox();
    expect(primaryBox).not.toBeNull(); expect(arrowBox).not.toBeNull();
    expect(arrowBox!.x - primaryBox!.x - primaryBox!.width).toBeCloseTo(2, 1);
    await expect(group.locator(".alcomd-split-button-primary")).toHaveCSS("border-start-end-radius", "8px");
    await expect(group.locator(".alcomd-split-button-menu")).toHaveCSS("border-start-start-radius", "8px");
    expect(arrowBox!.height).toBe(primaryBox!.height);
    await primary.click();
    await expect(page.getByRole("dialog", { name: "Create template from project", exact: true })).toBeVisible();
    await page.getByLabel("Display name", { exact: true }).fill("Unsaved template");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(primary).toBeFocused();
    await primary.click();
    await expect(page.getByLabel("Display name", { exact: true })).toHaveValue("");
    await page.keyboard.press("Escape");
    await expect(primary).toBeFocused();
    await afterMaterialEvent(page, "md-menu", "opened", async () => {
        await arrow.focus(); await page.keyboard.press("ArrowDown");
    });
    await expect(page.getByRole("menuitem", { name: "Import template", exact: true })).toBeVisible();
    await page.screenshot({ path: testInfo.outputPath("templates-creation-menu.png"), fullPage: false });
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeVisible();
    await expect(page.getByRole("menu")).toBeHidden();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(arrow).toBeFocused();
});

test("Templates creation remains available for an empty catalog and respects manage capability", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Ftemplates&state=empty");
    await expect(page.getByText("No templates available")).toBeVisible();
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    await expect(page.getByText("No registered projects available.")).toBeVisible();
    await expect(page.getByRole("button", { name: "Review", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.goto("/browser-harness.html?route=%2Ftemplates&state=capabilities-missing");
    await expect(page.getByRole("button", { name: "Create template", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    const item = page.locator("md-menu-item").filter({ hasText: "Import template" });
    await expect(item).toHaveJSProperty("disabled", true);
    await expect(item).toHaveAttribute("title", /templates.manage.v1/);
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("Templates primary action uses the existing derive Plan and explicit Apply", async ({ page }) => {
    await templates(page);
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    await page.getByRole("combobox", { name: "Source project", exact: true }).click();
    await page.getByRole("option", { name: "<private-project>", exact: true }).click();
    await page.getByLabel("Display name", { exact: true }).fill("Derived fixture");
    await page.getByLabel("New template ID", { exact: true }).fill("00000000-0000-4000-8000-000000000303");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Review new template", exact: true })).toBeVisible();
    await expect(page.getByText("Plan fingerprint:")).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("button", { name: "Create template", exact: true })).toBeFocused();
});

test("template row actions open a modal and cancel returns to the list", async ({ page }) => {
    await templates(page);
    await expect(page.getByRole("table", { name: "Templates", exact: true }).getByRole("columnheader")).toHaveText(["Template", "ID", "Last modified", "Source", "Actions"]);
    await expect(page.getByRole("button", { name: "Favorite Avatar starter", exact: true })).toBeVisible();
    await expect(page.getByLabel("Export target", { exact: true })).toBeHidden();
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Export template", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Export template", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("menu")).toBeHidden();
    await page.getByLabel("Export target", { exact: true }).fill("C:\\Fixture\\draft.alcomdtemplate");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Export template", exact: true }).click();
    await expect(page.getByLabel("Export target", { exact: true })).toHaveValue("");
});

test("template create stays in one modal for the real plan review", async ({ page }) => {
    await templates(page);
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Create project", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Create project", exact: true });
    await expect(dialog).toBeVisible();
    await page.getByLabel("Target parent", { exact: true }).fill("C:\\Fixture");
    await page.getByLabel("Project folder name", { exact: true }).fill("ReviewOnly");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    const review = page.getByRole("dialog", { name: "Review project creation", exact: true });
    await expect(review).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(page.getByText("Plan fingerprint:")).toBeVisible();
    await page.getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(review).toBeHidden();
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
});

test("built-in template removal cannot be activated and derive reads a project choice", async ({ page }) => {
    await templates(page);
    const more = page.getByRole("button", { name: "More actions for Avatar starter", exact: true });
    await afterMaterialEvent(page, "md-menu", "opened", () => more.click());
    const remove = page.getByRole("menuitem", { name: "Remove template", exact: true });
    const removeHost = page.locator("md-menu-item").filter({ hasText: "Remove template" });
    await expect(removeHost).toHaveJSProperty("disabled", true);
    // A forced disabled-item click can dismiss Material's menu without activating the action.
    await afterMaterialEvent(page, "md-menu", "closed", () => remove.click({ force: true }));
    await expect(page.getByRole("dialog", { name: "Remove template", exact: true })).toBeHidden();
    expect(await page.evaluate(() => window.templateRequests.filter((request) => /templates\.(plan|apply)/.test(request.method)))).toEqual([]);
    await afterMaterialEvent(page, "md-menu", "opened", () => more.click());
    await page.getByRole("menuitem", { name: "Create project", exact: true }).focus();
    // End skips the disabled final item; keyboard activation opens Export, never Remove.
    await page.keyboard.press("End");
    await expect(page.getByRole("menuitem", { name: "Export template", exact: true })).toBeFocused();
    await afterMaterialEvent(page, "md-dialog", "opened", () => page.keyboard.press("Enter"));
    await expect(page.getByRole("dialog", { name: "Export template", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Remove template", exact: true })).toBeHidden();
    await afterMaterialEvent(page, "md-dialog", "closed", () => page.keyboard.press("Escape"));
    await expect(more).toBeFocused();
    await afterMaterialEvent(page, "md-menu", "opened", () => more.click());
    await afterMaterialEvent(page, "md-dialog", "opened", () => page.getByRole("menuitem", { name: "Derive from project", exact: true }).click());
    const dialog = page.getByRole("dialog", { name: "Create template from project", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Source project", exact: true })).toBeVisible();
    await expect(page.getByLabel("Expected project revision", { exact: true })).toHaveCount(0);
    await expect(page.getByLabel("Source project ID", { exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.templateRequests.filter((request) => /templates\.(plan|apply)/.test(request.method)))).toEqual([]);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
});

async function afterMaterialEvent(page: Page, tag: "md-menu" | "md-dialog", event: "opened" | "closed", action: () => Promise<unknown>) {
    const lifecycle = await page.evaluateHandle(({ tag, event }) => ({ finished: new Promise<void>((resolve) => {
        const listener = (change: Event) => {
            if (!(change.target instanceof HTMLElement) || change.target.localName !== tag) return;
            document.removeEventListener(event, listener, true);
            resolve();
        };
        document.addEventListener(event, listener, true);
    }) }), { tag, event });
    await action();
    await lifecycle.evaluate((state) => state.finished);
    await lifecycle.dispose();
}

async function templates(page: Page) {
    await page.goto("/browser-harness.html?route=%2Ftemplates&state=ready");
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
}
