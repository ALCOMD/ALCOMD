import { expect, test, type Page } from "@playwright/test";

const cursor = "opaque template cursor:100/+=";
const lastTemplateId = "00000000-0000-4000-8000-000000001101";

test.use({ viewport: { width: 1440, height: 900 } });

test("Templates exposes item 101 through the existing opaque cursor without planning", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-pages=1");
    const table = page.getByRole("table", { name: "Templates", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(100);
    await expect(page.getByText("Catalog template 101", { exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(table.locator("tbody tr")).toHaveCount(101);
    await table.getByRole("row").filter({ hasText: "Catalog template 101" }).scrollIntoViewIfNeeded();
    await expect(table.getByText("Catalog template 101", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.list" && Object.keys(request.params as object).length > 0))).toEqual([
        { method: "templates.list", params: { cursor } }
    ]);
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method.includes("plan")))).toEqual([]);
});

test("Projects can select template 101 and create the project returned by Core", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-pages=1");
    const dialog = await openCreation(page);
    await dialog.getByRole("button", { name: "Load more templates", exact: true }).click();
    const select = dialog.getByRole("combobox", { name: "Project template", exact: true });
    await select.click();
    await dialog.getByRole("option", { name: "Catalog template 101 · 1.0.0", exact: true }).click();
    await expect(select).toContainText("Catalog template 101");
    await dialog.getByRole("button", { name: "Choose directory", exact: true }).click();
    await dialog.getByLabel("Create project name", { exact: true }).fill("From page two");
    await dialog.getByRole("button", { name: "Review creation", exact: true }).click();
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.planCreateProject"))).toEqual([
        { method: "templates.planCreateProject", params: { templateId: lastTemplateId, expectedTemplateRevision: 2, targetParent: "C:\\Fixture\\Avatar", targetLeaf: "From page two" } }
    ]);
    await dialog.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "From page two" })).toBeVisible();
});

test("Templates page failure retains all loaded rows and retries the same cursor once", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-pages=1&template-page-error=1");
    const rows = page.getByRole("table", { name: "Templates", exact: true }).locator("tbody tr");
    await expect(rows).toHaveCount(100);
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Your loaded items are still available.");
    await expect(rows).toHaveCount(100);
    await page.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(rows).toHaveCount(101);
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.list" && Object.keys(request.params as object).length > 0).map((request) => request.params))).toEqual([{ cursor }, { cursor }]);
});

test("Create template paging retains the selected template and draft after failure and retry", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-pages=1&template-page-error=1");
    const dialog = await openCreation(page);
    await dialog.getByRole("combobox", { name: "Project template", exact: true }).click();
    await dialog.getByRole("option", { name: "Catalog template 050 · 1.0.0", exact: true }).click();
    await dialog.getByLabel("Create project name", { exact: true }).fill("Keep this draft");
    await dialog.getByRole("button", { name: "Load more templates", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("Your loaded items are still available.");
    await expect(dialog.locator("#project-create-template")).toHaveJSProperty("value", "00000000-0000-4000-8000-000000001050");
    await expect(dialog.locator("md-select-option")).toHaveCount(100);
    await dialog.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(dialog.locator("md-select-option")).toHaveCount(101);
    await expect(dialog.locator("#project-create-template")).toHaveJSProperty("value", "00000000-0000-4000-8000-000000001050");
    await expect(dialog.getByLabel("Create project name", { exact: true })).toHaveValue("Keep this draft");
    expect(await dialog.locator("md-select-option").evaluateAll((options) => new Set(options.map((option) => (option as HTMLElement & { value: string }).value)).size)).toBe(101);
});

test("A closed template picker rejects the old page response after a new draft opens", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-pages=1&hold-template-page=1");
    const dialog = await openCreation(page);
    await dialog.getByRole("button", { name: "Load more templates", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.list" && Object.keys(request.params as object).length > 0).length)).toBe(1);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await openCreation(page);
    await expect(dialog.locator("md-select-option")).toHaveCount(100);
    await dialog.getByLabel("Create project name", { exact: true }).fill("New draft");
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-page-ready")));
    await expect.poll(() => page.evaluate(() => window.templatePageCompletions)).toBe(1);
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(dialog.locator("md-select-option")).toHaveCount(100);
    await expect(dialog.getByLabel("Create project name", { exact: true })).toHaveValue("New draft");
    await expect(dialog.getByRole("button", { name: "Load more templates", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method.includes("plan")))).toEqual([]);
});

test("A template read failure stays distinct from an empty catalog and can be explicitly retried", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-list-error=1&holdTemplates=1");
    const dialog = await openCreation(page);
    await expect(dialog.getByRole("alert")).toContainText("Templates could not be loaded.");
    await expect(dialog.getByText("No templates available", { exact: true })).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-list-readable")));
    await dialog.getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.heldTemplateReads)).toBe(1);
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => resolve())));
    expect(await dialog.evaluate(async (element) => { await (element as HTMLElement & { updateComplete: Promise<void> }).updateComplete; return (element as HTMLElement & { open: boolean }).open; })).toBe(true);
    await page.evaluate(() => window.dispatchEvent(new Event("test-templates-ready")));
    await expect(dialog.getByRole("combobox", { name: "Project template", exact: true })).toContainText("Avatar starter");
    await expect(dialog.getByRole("alert")).toHaveCount(0);
});

async function openCreation(page: Page) {
    const host = page.locator("md-dialog").filter({ has: page.locator('[slot="headline"]').filter({ hasText: /^Create project$/ }) });
    const opened = await host.evaluateHandle((element) => ({ opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
    try {
        await page.getByRole("button", { name: "Create project", exact: true }).click();
        await opened.evaluate((events) => events.opened);
    } finally { await opened.dispose(); }
    const dialog = page.getByRole("dialog", { name: "Create project", exact: true });
    await expect(dialog).toBeVisible();
    // Material's native dialog is in its shadow root; the form belongs to its public host's light DOM.
    return host;
}
