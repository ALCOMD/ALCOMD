import { expect, test, type Page } from "@playwright/test";

async function open(page: Page, error = false) {
    await page.goto(`/browser-harness.html?route=/settings&h3-unity-discovery=1${error ? "&h3-unity-discovery-error=1" : ""}`);
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

test("Unity version verification refusal stays in its dialog and an explicit discovery retry clears it", async ({ page }) => {
    const dialog = await open(page, true);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("Unity Editor version could not be verified.");
    await expect(dialog.getByRole("alert")).toContainText("unity_version_unverified");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    expect(await discoveryRequests(page)).toBe(1);
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toContainText("Unity installations refreshed.");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("table", { name: "Unity installations", exact: true })).toContainText("2022.3.22f1");
    expect(await discoveryRequests(page)).toBe(2);
});
