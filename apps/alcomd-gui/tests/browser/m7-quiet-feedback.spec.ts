import { expect, test } from "@playwright/test";

for (const flow of ["create", "restore"] as const) {
    test(`${flow} fast initial reads never expose loading or a false empty state`, async ({ page }) => {
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
        if (flow === "create") await page.getByRole("button", { name: "Create project", exact: true }).click();
        else {
            await page.getByRole("button", { name: "More project creation options", exact: true }).click();
            await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
        }
        const label = flow === "create" ? "Create project" : "Restore project";
        await expect(page.getByRole("dialog", { name: label, exact: true })).toBeVisible();
        await expect(page.getByRole("combobox", { name: flow === "create" ? "Project template" : "Project backup", exact: true })).toBeVisible();
        await expect(page.getByText(flow === "create" ? "No templates available" : "No backups available", { exact: true })).toHaveCount(0);
        await expect(page.locator("md-dialog[open] .loading-state-indicator")).toHaveCount(0);
        const states = await exposed.evaluate(({ seen, observer }) => { observer.disconnect(); return seen; });
        expect(states.some((state) => /Loading (templates|backups)…|No (templates|backups) available/.test(state))).toBe(false);
        await exposed.dispose();
    });

    test(`${flow} slow initial reads show delayed loading without a false empty state`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects&${flow === "create" ? "holdTemplates" : "holdBackups"}=1`);
        if (flow === "create") await page.getByRole("button", { name: "Create project", exact: true }).click();
        else {
            await page.getByRole("button", { name: "More project creation options", exact: true }).click();
            await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
        }
        const label = flow === "create" ? "Create project" : "Restore project";
        await expect(page.getByRole("dialog", { name: label, exact: true })).toBeVisible();
        await expect(page.locator("md-dialog[open] .loading-state-indicator")).toBeVisible();
        await expect(page.getByText(flow === "create" ? "No templates available" : "No backups available", { exact: true })).toHaveCount(0);
        await page.evaluate((name) => window.dispatchEvent(new Event(name)), flow === "create" ? "test-templates-ready" : "test-backups-ready");
        await expect(page.getByRole("combobox", { name: flow === "create" ? "Project template" : "Project backup", exact: true })).toBeVisible();
    });
}

test("a held creation Plan keeps the action label and dialog bounds stable", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&holdCreatePlan=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await page.getByRole("button", { name: "Choose directory", exact: true }).click();
    await page.getByLabel("Create project name").fill("Quiet Plan");
    await page.screenshot({ animations: "disabled" });
    const dialog = page.getByRole("dialog", { name: "Create project", exact: true });
    const before = (await dialog.boundingBox())!;
    const review = page.getByRole("button", { name: "Review creation", exact: true });
    await review.click();
    await expect(review).toBeDisabled();
    await expect(review).toHaveAccessibleName("Review creation");
    await expect(page.getByText("Planning…", { exact: true })).toHaveCount(0);
    const after = (await dialog.boundingBox())!;
    for (const key of ["x", "y", "width", "height"] as const) expect(Math.abs(after[key] - before[key])).toBeLessThanOrEqual(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-create-plan-ready")));
    await expect(page.getByText(/ALCOMD Core will validate the frozen template/)).toBeVisible();
});
