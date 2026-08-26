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

    await expect(page.getByRole("button", { name: "Project evidence" })).toBeVisible();
    const iconButton = page.locator("md-icon-button").first();
    const icon = iconButton.locator(".alcomd-icon");
    await expect(icon).toHaveAttribute("aria-hidden", "true");
    await expect(icon).toHaveAttribute("data-filled", "false");
    const lightColors = await icon.evaluate((element) => {
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, foreground: style.color };
    });
    expect(lightColors.background).toBe(lightColors.foreground);
    expect(await icon.evaluate((element) => getComputedStyle(element).webkitMaskImage)).not.toBe("none");
    await page.locator("html").evaluate((element) => { element.dataset.appearance = "dark"; });
    const darkColors = await icon.evaluate((element) => {
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, foreground: style.color };
    });
    expect(darkColors.background).toBe(darkColors.foreground);
    expect(darkColors.background).not.toBe(lightColors.background);
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
    await expect(page.getByRole("main").locator("md-text-button").filter({ hasText: "Refresh" })).toBeVisible();
    const corePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());

    await openHarness(page, "/extensions/com.cqmhv.mcp-management/ui");
    await expect(page.getByRole("region", { name: "MCP Management" }).locator("md-filled-tonal-button").filter({ hasText: "Refresh" })).toBeVisible();
    const portablePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());
    expect(portablePrimary).toBe(corePrimary);
});

test("official navigation uses offline decorative Rounded icons with filled selection state", async ({ page }) => {
    const remoteFontRequests: string[] = [];
    page.on("request", (request) => {
        if (/fonts\.(?:googleapis|gstatic)\.com/.test(request.url())) remoteFontRequests.push(request.url());
    });
    await openHarness(page, "/projects");

    const navigation = page.locator("#primary-navigation");
    await expect(navigation.locator(".alcomd-icon")).toHaveCount(5);
    await expect(page.getByRole("button", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Packages & Templates", exact: true })).toBeVisible();

    const projects = navigation.locator("md-text-button.navigation-item").filter({ hasText: "Projects" });
    const packages = navigation.locator("md-text-button.navigation-item").filter({ hasText: "Packages & Templates" });
    await expect(projects.locator(".alcomd-icon")).toHaveAttribute("aria-hidden", "true");
    await expect(projects.locator(".alcomd-icon")).toHaveAttribute("data-filled", "true");
    await expect(packages.locator(".alcomd-icon")).toHaveAttribute("data-filled", "false");
    expect(remoteFontRequests).toEqual([]);
});

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
}
