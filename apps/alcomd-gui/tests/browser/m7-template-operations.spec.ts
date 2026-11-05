import { expect, test } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

test("a lost creation reply retries the identical request after a catalog change", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&create-unknown=1&template-existing=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const template = dialog.getByRole("combobox", { name: "Project template", exact: true });
    await template.click();
    await dialog.getByRole("option", { name: "Existing fixture", exact: true }).click();
    await dialog.getByRole("combobox", { name: "Unity version", exact: true }).click();
    await dialog.getByRole("option", { name: "2022.3.6f1", exact: true }).click();
    await dialog.getByLabel("Create project name", { exact: true }).fill("Retry fixture");
    const submit = dialog.getByRole("button", { name: "Create project", exact: true });
    await submit.click();
    await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
    await expect(dialog.locator("#project-create-template")).toHaveJSProperty("disabled", true);
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-advance")));
    await expect.poll(() => page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projectTemplates.list").length)).toBeGreaterThan(1);
    await submit.click();
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
    const template = dialog.getByRole("combobox", { name: "Project template", exact: true });
    const chooseTemplate = async () => {
        await template.click();
        await dialog.getByRole("option", { name: /Existing fixture/ }).click();
    };
    await chooseTemplate();
    await dialog.getByLabel("Create project name", { exact: true }).fill("Selected again");
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-advance")));
    await expect(dialog.getByRole("alert")).toContainText("The selected template changed. Select it again.");
    const submit = dialog.getByRole("button", { name: "Create project", exact: true });
    await expect(submit).toBeDisabled();
    await chooseTemplate();
    const version = dialog.getByRole("combobox", { name: "Unity version", exact: true });
    await version.click();
    await dialog.getByRole("option", { name: "2022.3.6f1", exact: true }).click();
    await submit.click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
    expect(await page.evaluate(() => window.templateRequests.find(({ method }) => method === "projects.create")?.params))
        .toMatchObject({ name: "Selected again", template: { id: "com.example.fixture", revision: 5 } });
});
