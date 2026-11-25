import { expect, test, type Locator } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

async function selectExistingTemplate(dialog: Locator) {
    const picker = dialog.locator("#project-create-template");
    const lifecycle = await picker.evaluateHandle((element) => {
        const once = (type: string) => new Promise<void>((resolve) => {
            element.addEventListener(type, () => resolve(), { once: true });
        });
        return { opened: once("opened"), closed: once("closed") };
    });
    try {
        await picker.getByRole("combobox", { name: "Project template", exact: true }).click();
        await lifecycle.evaluate((events) => events.opened);
        const option = picker.getByRole("option", { name: "Existing fixture · 2022.3.22f1", exact: true });
        await expect(option).toBeEnabled();
        await option.click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
    await expect(picker).toHaveJSProperty("value", "com.example.fixture");
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveValue("2022.3.22f1");
}

test("a lost creation reply retries the identical request after a catalog change", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&create-unknown=1&template-existing=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await selectExistingTemplate(dialog);
    await dialog.getByLabel("Create project name", { exact: true }).fill("Retry fixture");
    const submit = dialog.getByRole("button", { name: "Create project", exact: true });
    await submit.click();
    await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
    await expect(dialog.locator("#project-create-template")).toHaveJSProperty("disabled", true);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-advance")));
    await expect.poll(() => page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projectTemplates.list").length)).toBeGreaterThan(1);
    await submit.click();
    await expect(page.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(page).toHaveURL(/\/projects$/);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
    const calls = await page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projects.create"));
    expect(calls).toHaveLength(2);
    expect(calls[0]).toEqual(calls[1]);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Retry fixture" })).toHaveCount(1);
});

test("catalog changes invalidate a selected revision until the user reselects", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&template-existing=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await selectExistingTemplate(dialog);
    await dialog.getByLabel("Create project name", { exact: true }).fill("Selected again");
    const submit = dialog.getByRole("button", { name: "Create project", exact: true });
    await expect(submit).toBeEnabled();
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-advance")));
    await expect(dialog.getByRole("alert")).toContainText("The selected template changed. Select it again.");
    await expect(submit).toBeDisabled();
    await selectExistingTemplate(dialog);
    await submit.click();
    await expect(page.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(page).toHaveURL(/\/projects$/);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
    expect(await page.evaluate(() => window.templateRequests.find(({ method }) => method === "projects.create")?.params))
        .toMatchObject({ target: { kind: "default", name: "Selected again" }, template: { id: "com.example.fixture", revision: 9 } });
});
