import { expect, test, type Page } from "@playwright/test";

const importedId = "00000000-0000-4000-8000-000000000302";
const builtinId = "00000000-0000-4000-8000-000000000301";
const operationId = "00000000-0000-4000-8000-000000005000";

test.use({ viewport: { width: 1440, height: 900 } });

test("template import review uses the flat Core identity, revision and replacement evidence", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1");
    await importReview(page);
    const summary = page.locator("md-dialog .dialog-summary");
    await expect(summary).toContainText(importedId);
    await expect(summary).toContainText("Expected revision");
    await expect(summary).toContainText("0");
    await expect(summary).toContainText("Replace existing template");
    await expect(summary).toContainText("No");
    await expect(summary).toContainText("Imported fixture");
    expect(await applies(page)).toEqual([]);
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeEnabled();
    await expect(page.locator("md-dialog[open]")).not.toContainText("opaque-selected-bundle");
});

test("an accepted import survives dialog close, retries its first progress read and refreshes only on success", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&template-first-read-error=1");
    await importReview(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeHidden();
    const table = page.getByRole("table", { name: "Templates", exact: true });
    await expect(table).not.toContainText("Imported fixture");
    await expect.poll(() => reads(page, operationId)).toBeGreaterThan(2);
    const before = await listReads(page);
    await finish(page, operationId);
    await expect(table).toContainText("Imported fixture");
    expect(await listReads(page)).toBe(before + 1);
    expect(await applies(page)).toHaveLength(1);
    expect(await page.evaluate(() => [...new Set(window.templateRequests.filter((request) => request.method === "operations.get").map((request) => (request.params as { operationId: string }).operationId))])).toEqual([operationId]);
});

test("both project creation reviews require the actual frozen template and absent destination evidence", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1");
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Create project", exact: true }).click();
    await page.getByLabel("Target parent", { exact: true }).fill("C:\\Fixture");
    await page.getByLabel("Project folder name", { exact: true }).fill("Frozen project");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    const summary = page.locator("md-dialog .dialog-summary");
    await expect(summary).toContainText(builtinId);
    await expect(summary).toContainText("Template revision");
    await expect(summary).toContainText("2");
    await expect(summary).toContainText("Frozen project");
    await expect(summary).toContainText("Destination must be absent");
    await expect(summary).toContainText("Yes");
    await expect(page.locator("md-dialog[open]")).not.toContainText("opaque-parent");
    expect(await applies(page)).toEqual([]);
});

test("a create plan missing its destination safety evidence cannot be applied", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-invalid-plan=1");
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: "Create project", exact: true }).click();
    await page.getByLabel("Target parent", { exact: true }).fill("C:\\Fixture");
    await page.getByLabel("Project folder name", { exact: true }).fill("Unsafe evidence");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("internal_error");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toHaveCount(0);
    expect(await applies(page)).toEqual([]);
});

test("a known template operation keeps reading after its initial progress request fails", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&template-first-read-error=1");
    await importReview(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect.poll(() => reads(page, operationId)).toBeGreaterThan(2);
    expect(await applies(page)).toHaveLength(1);
});

test("a late accepted import belongs to its page while terminal refresh preserves a new row draft and focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&hold-template-apply=1");
    await importReview(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect.poll(async () => (await applies(page)).length).toBe(1);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").evaluate((element) => { (element as HTMLElement & { open: boolean }).open = false; });
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeHidden();
    await rowAction(page, "Export template");
    await page.getByLabel("Export target", { exact: true }).fill("C:\\Fixture\\keep-this-draft.alcomdtemplate");
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-apply-ready")));
    await expect.poll(() => reads(page, operationId)).toBeGreaterThan(0);
    const before = await listReads(page);
    await finish(page, operationId);
    await expect.poll(() => listReads(page)).toBe(before + 1);
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toContainText("Imported fixture");
    await expect(page.getByLabel("Export target", { exact: true })).toHaveValue("C:\\Fixture\\keep-this-draft.alcomdtemplate");
    await expect(page.getByRole("dialog", { name: "Export template", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("button", { name: "Create template", exact: true })).toBeFocused();
    expect(await applies(page)).toHaveLength(1);
});

test("two explicitly accepted template operations retain separate IDs through a row refresh", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&hold-template-apply=1");
    await importReview(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect.poll(async () => (await applies(page)).length).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-apply-ready")));
    await expect(page.getByRole("button", { name: "Close", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Close", exact: true }).click();
    await rowAction(page, "Create project");
    await page.getByLabel("Target parent", { exact: true }).fill("C:\\Fixture");
    await page.getByLabel("Project folder name", { exact: true }).fill("Second operation");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect.poll(async () => (await applies(page)).length).toBe(2);
    const before = await listReads(page);
    await finish(page, operationId);
    await expect.poll(() => listReads(page)).toBe(before + 1);
    await expect(page.locator("md-dialog[open] .dialog-summary")).toContainText("Second operation");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeDisabled();
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-apply-ready")));
    const secondId = "00000000-0000-4000-8000-000000005001";
    await expect.poll(() => reads(page, secondId)).toBeGreaterThan(0);
    await finish(page, secondId);
    await expect(page.getByRole("button", { name: "Open created project", exact: true })).toBeVisible();
    expect(await applies(page)).toHaveLength(2);
    await page.getByRole("button", { name: "Open created project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000007001$/);
});

test("an older created project refresh cannot close or navigate a newly opened Projects draft", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-tasks=1&hold-project-refresh=1");
    const host = await projectReview(page, "First project");
    await host.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeEnabled();
    const before = await page.evaluate(() => window.templateRequests.filter((request) => request.method === "projects.list").length);
    await finish(page, operationId);
    await expect.poll(() => page.evaluate(() => window.templateRequests.filter((request) => request.method === "projects.list").length)).toBe(before + 1);
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(host.getByLabel("Create project name", { exact: true })).toBeVisible();
    await host.getByLabel("Create project name", { exact: true }).fill("New untouched draft");
    await page.evaluate(() => window.dispatchEvent(new Event("test-project-refresh-ready")));
    await expect(page.getByRole("row").filter({ hasText: "First project" })).toBeVisible();
    await expect(host.getByLabel("Create project name", { exact: true })).toHaveValue("New untouched draft");
    await expect(page).toHaveURL(/\/projects$/);
    expect(await applies(page)).toHaveLength(1);
});

for (const state of ["failed", "cancelled"]) {
    test(`a ${state} template task does not refresh or report success`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/templates&template-tasks=1");
        await importReview(page);
        await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
        await page.getByRole("button", { name: "Close", exact: true }).click();
        await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeHidden();
        const before = await listReads(page);
        await finish(page, operationId, state);
        await expect(page.getByRole("status").filter({ hasText: "Operation" }).locator("strong")).toHaveText(state);
        expect(await listReads(page)).toBe(before);
        await expect(page.getByRole("table", { name: "Templates", exact: true })).not.toContainText("Imported fixture");
        expect(await applies(page)).toHaveLength(1);
    });
}

test("same-digest imports retain Core's override action without requiring a new replacement intent", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&template-existing=1&template-same-digest=1");
    await importReview(page);
    const summary = page.locator("md-dialog[open] .dialog-summary");
    await expect(summary).toContainText("Expected revision2");
    await expect(summary).toContainText("Replace existing templateYes");
    await expect(summary).toContainText("Existing fixture");
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await applies(page)).toEqual([]);
});

test("an unknown Apply response is not retried or treated as a rejected task", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&template-apply-unknown=1");
    await importReview(page);
    await page.getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("daemon_unavailable");
    await expect(page.getByText("The result of this request is unknown. Check Tasks and refresh before reviewing it again.", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeDisabled();
    await page.keyboard.press("Enter");
    expect(await applies(page)).toHaveLength(1);
    expect(await reads(page, operationId)).toBe(0);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeHidden();
    await page.getByRole("navigation", { name: "Utilities", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Tasks$/ }) }).click();
    await expect(page.getByRole("table", { name: "Tasks", exact: true }).getByRole("cell", { name: "templates import", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "View operation", exact: true }).click();
    await expect(page).toHaveURL(/\/operations\/00000000-0000-4000-8000-000000005000$/);
});

test("derive uses a source project from page two and the revision returned by Core", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&template-project-pages=1");
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    await page.getByRole("button", { name: "Load more projects", exact: true }).click();
    await page.getByRole("combobox", { name: "Source project", exact: true }).click();
    await page.getByRole("option", { name: "C:\\Fixture\\Source 101", exact: true }).click();
    await page.getByLabel("Display name", { exact: true }).fill("Derived fixture");
    await page.getByLabel("New template ID", { exact: true }).fill("00000000-0000-4000-8000-000000000303");
    await page.getByRole("button", { name: "Review", exact: true }).click();
    const summary = page.locator("md-dialog[open] .dialog-summary");
    await expect(summary).toContainText("00000000-0000-4000-8000-000000002101");
    await expect(summary).toContainText("Source project revision7");
    expect(await page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.planDerive").map((request) => request.params))).toEqual([{ projectId: "00000000-0000-4000-8000-000000002101", expectedProjectRevision: 7, templateId: "00000000-0000-4000-8000-000000000303", templateVersion: "1.0.0", displayName: "Derived fixture" }]);
    expect(await applies(page)).toEqual([]);
});

test("a late derive Plan cannot replace a newly opened draft", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-tasks=1&hold-template-derive-plan=1");
    await deriveReview(page, "Old draft");
    await expect.poll(() => page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.planDerive").length)).toBe(1);
    await page.locator("md-dialog[open]").evaluate((element) => { (element as HTMLElement & { open: boolean }).open = false; });
    await expect(page.getByRole("dialog", { name: "Create template from project", exact: true })).toBeHidden();
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    await page.getByLabel("Display name", { exact: true }).fill("New draft");
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-derive-plan-ready")));
    await expect(page.getByLabel("Display name", { exact: true })).toHaveValue("New draft");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toHaveCount(0);
    expect(await applies(page)).toEqual([]);
});

test("import metadata must match the inspected digest before Apply becomes available", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-invalid-plan=import-hash");
    await importPlan(page);
    await expect(page.getByRole("alert")).toContainText("internal_error");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toHaveCount(0);
    expect(await applies(page)).toEqual([]);
});

test("derive rejects a Plan whose source revision has the wrong wire type", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-invalid-plan=derive-revision");
    await deriveReview(page, "Invalid revision");
    await expect(page.getByRole("alert")).toContainText("internal_error");
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toHaveCount(0);
    expect(await applies(page)).toEqual([]);
});

async function importReview(page: Page) {
    await importPlan(page);
    await expect(page.getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeVisible();
}

async function importPlan(page: Page) {
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeVisible();
    await page.getByLabel("Template bundle", { exact: true }).fill("C:\\Fixture\\input.alcomdtemplate");
    await page.getByRole("button", { name: "Review import", exact: true }).click();
}

async function deriveReview(page: Page, name: string) {
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    await page.getByRole("combobox", { name: "Source project", exact: true }).click();
    await page.getByRole("option", { name: "<private-project>", exact: true }).click();
    await page.getByLabel("Display name", { exact: true }).fill(name);
    await page.getByLabel("New template ID", { exact: true }).fill("00000000-0000-4000-8000-000000000303");
    await page.getByRole("button", { name: "Review", exact: true }).click();
}

async function rowAction(page: Page, action: string) {
    await page.getByRole("button", { name: "More actions for Avatar starter", exact: true }).click();
    await page.getByRole("menuitem", { name: action, exact: true }).click();
    await expect(page.getByRole("dialog", { name: action, exact: true })).toBeVisible();
}

async function projectReview(page: Page, name: string) {
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Create project", exact: true })).toBeVisible();
    const host = page.locator("md-dialog[open]");
    await host.getByLabel("Create target parent", { exact: true }).fill("C:\\Fixture");
    await host.getByLabel("Create project name", { exact: true }).fill(name);
    await host.getByRole("button", { name: "Review creation", exact: true }).click();
    await expect(host.locator(".dialog-summary")).toContainText(name);
    return page.locator("md-dialog").filter({ has: page.locator('[slot="headline"]').filter({ hasText: /^Create project$/ }) });
}

async function applies(page: Page) { return page.evaluate(() => window.templateRequests.filter((request) => request.method.startsWith("templates.apply"))); }
async function reads(page: Page, id: string) { return page.evaluate((ownedId) => window.templateRequests.filter((request) => request.method === "operations.get" && (request.params as { operationId: string }).operationId === ownedId).length, id); }
async function listReads(page: Page) { return page.evaluate(() => window.templateRequests.filter((request) => request.method === "templates.list").length); }
async function finish(page: Page, id: string, state = "succeeded") { await page.evaluate((detail) => window.dispatchEvent(new CustomEvent("test-template-finish", { detail })), { operationId: id, state }); }
