import { expect, test, type Page } from "@playwright/test";

const templateId = "00000000-0000-4000-8000-000000000302";

test.use({ viewport: { width: 1440, height: 900 } });

test("template favorite changes persist across both list reloads without Plan or Apply", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await page.getByRole("button", { name: "Favorite Avatar starter", exact: true }).click();
    await expect(page.getByRole("button", { name: "Remove Avatar starter from favorites", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Remove Avatar starter from favorites", exact: true }).click();
    await expect(page.getByRole("button", { name: "Favorite Avatar starter", exact: true })).toBeVisible();
    expect((await requests(page, "templates.setFavorite")).map((request) => request.params)).toEqual([
        { templateId: "00000000-0000-4000-8000-000000000301", favorite: true, expectedRevision: 2 },
        { templateId: "00000000-0000-4000-8000-000000000301", favorite: false, expectedRevision: 3 }
    ]);
    expect(await page.evaluate(() => window.templateRequests.filter((request) => /templates\.(plan|apply)/.test(request.method)))).toEqual([]);
});

test("template export sends the selected identity, revision and destination through the existing RPC", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1");
    await action(page, "Existing fixture", "Export template");
    await page.getByLabel("Export target", { exact: true }).fill("C:\\Fixture\\exported.alcomdtemplate");
    await page.getByRole("button", { name: "Export", exact: true }).click();
    await expect(page.getByRole("status")).toContainText("Template exported.");
    expect(await requests(page, "templates.export")).toEqual([{ method: "templates.export", params: { templateId, expectedRevision: 2, targetPath: "C:\\Fixture\\exported.alcomdtemplate" } }]);
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toContainText("Existing fixture");
});

test("user template removal requires confirmation and preserves the built-in row", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1");
    await action(page, "Existing fixture", "Remove template");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Remove template", exact: true })).toBeHidden();
    expect(await requests(page, "templates.remove")).toEqual([]);
    await action(page, "Existing fixture", "Remove template");
    await page.getByRole("button", { name: "Remove template", exact: true }).click();
    const table = page.getByRole("table", { name: "Templates", exact: true });
    await expect(table).not.toContainText("Existing fixture");
    await expect(table).toContainText("Avatar starter");
    expect(await requests(page, "templates.remove")).toEqual([{ method: "templates.remove", params: { templateId, expectedRevision: 2 } }]);
});

test("different-digest import requires explicit replacement and reviews Core's existing revision", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1&template-tasks=1");
    await importPlan(page);
    await expect(page.getByRole("alert")).toContainText("template_conflict");
    expect(await requests(page, "templates.planImport")).toHaveLength(1);
    expect(await requests(page, "templates.applyImport")).toEqual([]);
    await page.getByRole("checkbox", { name: "Replace an existing matching template", exact: true }).check();
    await page.getByRole("button", { name: "Review import", exact: true }).click();
    const summary = page.locator("md-dialog[open] .dialog-summary");
    await expect(summary).toContainText(templateId);
    await expect(summary).toContainText("Expected revision2");
    await expect(summary).toContainText("Replace existing templateYes");
    await expect(summary).toContainText("Imported fixture");
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-template-finish", { detail: { operationId: "00000000-0000-4000-8000-000000005000" } })));
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toContainText("Imported fixture");
    await expect(page.getByRole("table", { name: "Templates", exact: true })).not.toContainText("Existing fixture");
    expect(await requests(page, "templates.applyImport")).toHaveLength(1);
});

test("built-in bundle replacement remains rejected even with explicit replacement intent", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-builtin-import=1");
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    await page.getByLabel("Template bundle", { exact: true }).fill("C:\\Fixture\\builtin.alcomdtemplate");
    await page.getByRole("checkbox", { name: "Replace an existing matching template", exact: true }).check();
    await page.getByRole("button", { name: "Review import", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("template_builtin_immutable");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toHaveCount(0);
    expect(await requests(page, "templates.applyImport")).toEqual([]);
});

test("a stale template Apply is displayed once without a new Plan, task or automatic retry", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-apply-error=1&template-tasks=1");
    await importPlan(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("template_plan_stale");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeDisabled();
    await page.keyboard.press("Enter");
    expect(await requests(page, "templates.planImport")).toHaveLength(1);
    expect(await requests(page, "templates.applyImport")).toHaveLength(1);
    expect(await requests(page, "operations.get")).toEqual([]);
    await expect(page.getByText("The result of this request is unknown. Check Tasks and refresh before reviewing it again.", { exact: true })).toHaveCount(0);
});

async function requests(page: Page, method: string) { return page.evaluate((name) => window.templateRequests.filter((request) => request.method === name), method); }

async function action(page: Page, name: string, action: string) {
    await page.getByRole("button", { name: `More actions for ${name}`, exact: true }).click();
    await page.getByRole("menuitem", { name: action, exact: true }).click();
    await expect(page.getByRole("dialog", { name: action, exact: true })).toBeVisible();
}

async function importPlan(page: Page) {
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    await page.getByLabel("Template bundle", { exact: true }).fill("C:\\Fixture\\input.alcomdtemplate");
    await page.getByRole("button", { name: "Review import", exact: true }).click();
}
