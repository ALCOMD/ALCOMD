import { expect, test, type Page } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

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
    await expect(page.getByRole("heading", { level: 1, name: "Templates", exact: true })).toBeVisible();
});

test("built-in template removal cannot be activated and derive reads a project choice", async ({ page }) => {
    await templates(page);
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    const remove = page.getByRole("menuitem", { name: "Remove template", exact: true });
    const removeHost = page.locator("md-menu-item").filter({ hasText: "Remove template" });
    await expect(removeHost).toHaveJSProperty("disabled", true);
    await remove.click({ force: true });
    await expect(page.getByRole("dialog", { name: "Remove template", exact: true })).toBeHidden();
    // Disabled metadata and keyboard protection are not screen-reader evidence.
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: "Remove template", exact: true })).toBeHidden();
    await page.keyboard.press("Escape");
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Derive from project", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Create template from project", exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Source project", exact: true })).toBeVisible();
    await expect(page.getByLabel("Expected project revision", { exact: true })).toHaveCount(0);
    await expect(page.getByLabel("Source project ID", { exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
});

async function templates(page: Page) {
    await page.goto("/browser-harness.html?route=%2Ftemplates&state=ready");
    await expect(page.getByRole("heading", { level: 1, name: "Templates", exact: true })).toBeVisible();
}
