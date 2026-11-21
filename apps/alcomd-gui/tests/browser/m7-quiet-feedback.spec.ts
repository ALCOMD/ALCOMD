import { expect, test } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";

test("create fast initial reads never expose loading or a false empty state", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects");
    await page.getByRole("heading", { level: 1, name: "Projects", exact: true }).waitFor();
    const exposed = await page.evaluateHandle(() => {
        const seen: string[] = [];
        const record = () => {
            for (const host of document.querySelectorAll<HTMLElement & { open: boolean }>("md-dialog")) {
                if (!host.open) continue;
                for (const node of host.querySelectorAll<HTMLElement>('[role="status"]')) {
                    const text = node.textContent?.trim();
                    if (text) seen.push(text);
                }
            }
        };
        const observer = new MutationObserver(record);
        observer.observe(document.body, { subtree: true, childList: true, attributes: true });
        // Keep the observer private to this test and stop it after the ready form is drawn.
        return { seen, observer };
    });
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Create project", exact: true })).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Project template", exact: true })).toBeVisible();
    await expect(page.getByText("No templates available", { exact: true })).toHaveCount(0);
    await expect(page.locator("md-dialog[open] .loading-state-indicator")).toHaveCount(0);
    const states = await exposed.evaluate(({ seen, observer }) => { observer.disconnect(); return seen; });
    expect(states.some((state) => /Loading templates…|No templates available/.test(state))).toBe(false);
    await exposed.dispose();
});

test("create remains editable while its templates are loading", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&holdTemplates=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Create project", exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open] .loading-state-indicator")).toHaveCount(0);
    await expect(page.getByLabel("Create project name", { exact: true })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.heldTemplateReads)).toBe(1);
    await expect(page.getByText("No templates available", { exact: true })).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("test-templates-ready")));
    await expect(page.getByRole("combobox", { name: "Project template", exact: true })).toBeVisible();
});

test("restore opens with a file input and accepts an explicit path immediately", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects");
    await selectActionMenuItem(page.getByRole("button", { name: "More project creation options", exact: true }), page.getByRole("menuitem", { name: "Restore from backup", exact: true }));
    await expect(page.getByRole("dialog", { name: "Restore project", exact: true })).toBeVisible();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.locator(".loading-state-indicator")).toHaveCount(0);
    const submit = dialog.getByRole("button", { name: "Restore project", exact: true });
    await expect(submit).toBeDisabled();
    await dialog.getByRole("textbox", { name: "Backup", exact: true }).fill("C:/Outside/Unity Project.zip");
    await expect(submit).toBeEnabled();
});

test("a held creation request keeps the action label and dialog bounds stable", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&hold-create-request=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await page.getByRole("button", { name: "Choose directory", exact: true }).click();
    await page.getByLabel("Create project name").fill("Quiet Creation");
    await page.screenshot({ animations: "disabled" });
    const dialog = page.getByRole("dialog", { name: "Create project", exact: true });
    const before = (await dialog.boundingBox())!;
    const review = page.locator("md-dialog[open]").getByRole("button", { name: "Create project", exact: true });
    await review.click();
    await expect(review).toBeDisabled();
    await expect(review).toHaveAccessibleName("Create project");
    await expect(page.getByText("Planning…", { exact: true })).toHaveCount(0);
    const after = (await dialog.boundingBox())!;
    for (const key of ["x", "y", "width", "height"] as const) expect(Math.abs(after[key] - before[key])).toBeLessThanOrEqual(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-create-request-ready")));
    const completion = page.locator("md-dialog[open]").filter({ has: page.locator(".operation-follow") });
    await expect(completion.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(page).toHaveURL(/\/projects$/);
    await completion.getByRole("button", { name: "Open project", exact: true }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/);
});
