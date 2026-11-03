import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

async function open(page: Page, error = false) {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto(`/browser-harness.html?route=/settings&h3-unity-discovery=1${error ? "&h3-unity-discovery-error=1" : ""}`);
    await selectSettingsSection(page, "unity");
    await expect(page.getByText("No Unity installations registered", { exact: true })).toBeVisible();
    await expect(page.getByText("No Unity installations found.", { exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    return page.locator("md-dialog[open]");
}

async function discoveryRequests(page: Page) {
    return page.evaluate(() => window.unityRequests.filter((request) => request.method === "unityInstallationsRefresh").length);
}

test("Unity discovery populates the registered installation list and list refresh keeps it visible", async ({ page }) => {
    const dialog = await open(page);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("Unity installations refreshed.");
    await expect(page.getByText("No Unity installations registered", { exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    const installations = page.getByRole("table", { name: "Unity installations", exact: true });
    await expect(installations).toContainText("2022.3.22f1");
    await page.getByRole("button", { name: "Refresh Unity installations", exact: true }).click();
    await expect(installations).toContainText("2022.3.22f1");
    expect(await discoveryRequests(page)).toBe(1);
});

test("Unity discovery refusal uses the shared outlet and an explicit discovery retry clears it", async ({ page }) => {
    const dialog = await open(page, true);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toHaveCount(1);
    await expect(dialog.locator(".reminder-floating")).toHaveCount(1);
    const feedback = outlet.getByRole("alert");
    await expect(feedback).toContainText("Unity Editor version could not be verified.");
    const bounds = (await outlet.boundingBox())!;
    expect(bounds.x + bounds.width).toBe(1280 - 24);
    expect(bounds.y + bounds.height).toBe(800 - 24);
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(feedback).toBeVisible();
    await expectRequestError(feedback, "unity_version_unverified");
    expect(await discoveryRequests(page)).toBe(1);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect(outlet.getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toContainText("Unity installations refreshed.");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("table", { name: "Unity installations", exact: true })).toContainText("2022.3.22f1");
    expect(await discoveryRequests(page)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

test("closing Unity installation management returns the shared outlet to the page without stale failure", async ({ page }) => {
    const dialog = await open(page, true);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toHaveCount(1);
    await expect(dialog.locator(".reminder-floating")).toHaveCount(1);
    await expectRequestError(outlet.getByRole("alert"), "unity_version_unverified");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(outlet).toHaveCount(1);
    await expect(page.locator("body > .reminder-floating")).toHaveCount(1);
    await expect(outlet.locator(".reminder")).toHaveCount(0);
    await expect(outlet).toBeHidden();
    expect(await discoveryRequests(page)).toBe(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});
