import { expect, test, type Page } from "@playwright/test";

test("shared Material foundation renders real controls with React 19 interaction semantics", async ({ page }) => {
    await page.goto("/browser-harness.html?material=1");
    const open = page.locator("md-filled-button").filter({ hasText: "Open dialog" });
    const disabled = page.locator("md-filled-tonal-button").filter({ hasText: "Disabled action" });
    await expect(open).toBeVisible();
    await expect(disabled).toHaveAttribute("disabled");
    await open.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByText("The dialog is hosted by the shared Material foundation.")).toBeVisible();
    await page.locator("md-text-button").filter({ hasText: "Close" }).click();
    await expect(page.getByText("The dialog is hosted by the shared Material foundation.")).toBeHidden();

    await expect(page.getByRole("button", { name: "Refresh evidence" })).toBeVisible();
    await expect(page.locator("md-outlined-text-field")).toHaveJSProperty("label", "Project name");
    await expect(page.locator("md-outlined-select")).toHaveJSProperty("label", "Project type");
    await expect(page.locator("md-switch")).toHaveJSProperty("selected", true);
    await expect(page.locator("md-checkbox")).toHaveJSProperty("checked", false);
    await page.locator("md-checkbox").click();
    await expect(page.locator("md-checkbox")).toHaveJSProperty("checked", true);
    await expect(page.locator("md-linear-progress")).toHaveJSProperty("value", 0.62);
});

test("Core and Portable UI share Material controls and the same MD3 theme source", async ({ page }) => {
    await openHarness(page, "/projects");
    await expect(page.getByRole("main").locator("md-text-button").filter({ hasText: "View project" })).toBeVisible();
    const corePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());

    await openHarness(page, "/extensions/com.cqmhv.mcp-management/ui");
    await expect(page.getByRole("region", { name: "MCP Management" }).locator("md-filled-tonal-button").filter({ hasText: "Refresh" })).toBeVisible();
    const portablePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());
    expect(portablePrimary).toBe(corePrimary);
});

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
}
