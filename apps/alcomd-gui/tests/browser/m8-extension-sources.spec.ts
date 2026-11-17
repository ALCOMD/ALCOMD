import { expect, test, type Page } from "@playwright/test";

const sourceUrl = "https://example.invalid/extensions.json?b=2&a=1&a=3";
const modal = (page: Page) => page.locator("md-dialog[open]");
async function open(page: Page, extra = "") {
    await page.goto(`/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1${extra}`);
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await modal(page).getByRole("button", { name: "Online source", exact: true }).click();
    await modal(page).getByLabel("Extension source URL", { exact: true }).fill(sourceUrl);
}
async function count(page: Page, method: string) {
    return page.evaluate((method) => window.h4Management.requests.filter((r) => r.method === method).length, method);
}
async function read(page: Page) {
    await modal(page).getByRole("button", { name: "Read source", exact: true }).click();
    await expect(modal(page).getByRole("heading", { name: "Online example", exact: true })).toBeVisible();
}
test("Online source chooses highest supported stable version and preserves the exact source through consent", async ({ page }) => {
    await open(page, "&h4-terms=1"); await read(page);
    await expect(modal(page).locator("md-filled-select")).toHaveJSProperty("value", "1.10.0");
    expect(await count(page, "extensions.applyInstall")).toBe(0);
    await modal(page).getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(modal(page).getByRole("heading", { name: "Extension terms", exact: true })).toBeVisible();
    expect(await count(page, "extensions.applyInstall")).toBe(0);
    await modal(page).getByRole("button", { name: "Agree and continue", exact: true }).click();
    await expect(modal(page).getByRole("heading", { name: "Required permissions", exact: true })).toBeVisible();
    await modal(page).getByRole("button", { name: "Agree and install", exact: true }).click();
    await expect.poll(() => count(page, "extensions.applyInstall")).toBe(1);
    const request = await page.evaluate(() => window.h4Management.requests.find((r) => r.method === "extensions.planInstallFromSource"));
    expect(request?.params).toEqual({ url: sourceUrl, version: "1.10.0", expectedSourceFingerprint: "a".repeat(64) });
    expect(await count(page, "extensions.planInstall")).toBe(0);
});
test("Cancel metadata loading discards its late preview and a reopened dialog starts clean", async ({ page }) => {
    await open(page); await page.evaluate(() => window.h4Management.hold("extensions.inspectSource", true));
    await modal(page).getByRole("button", { name: "Read source", exact: true }).click();
    await expect.poll(() => count(page, "extensions.inspectSource")).toBe(1);
    await modal(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.evaluate(() => window.h4Management.release("extensions.inspectSource"));
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(modal(page).getByLabel("Extension package", { exact: true })).toHaveValue("");
    expect(await count(page, "extensions.planInstallFromSource")).toBe(0);
    expect(await count(page, "extensions.applyInstall")).toBe(0);
});
for (const close of ["button", "escape"] as const) {
    test(`Cancel package preparation by ${close} never applies a late empty-consent Plan`, async ({ page }) => {
        await open(page, "&h4-no-required=1"); await read(page);
        await page.evaluate(() => window.h4Management.hold("extensions.planInstallFromSource", true));
        await modal(page).getByRole("button", { name: "Install extension", exact: true }).click();
        await expect.poll(() => count(page, "extensions.planInstallFromSource")).toBe(1);
        if (close === "button") await modal(page).getByRole("button", { name: "Cancel", exact: true }).click();
        else await page.keyboard.press("Escape");
        // Release during the closing animation, before the owner has unmounted the form.
        await page.evaluate(() => window.h4Management.release("extensions.planInstallFromSource"));
        await expect(page.getByRole("dialog")).toHaveCount(0);
        expect(await count(page, "extensions.applyInstall")).toBe(0);
    });
}
test("Changed source requires rereading instead of silently replanning; failure remains readable", async ({ page }) => {
    await open(page); await read(page);
    await page.evaluate(() => window.h4Management.failNext("extensions.planInstallFromSource", "revision_conflict"));
    await modal(page).getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toContainText("The data changed");
    await expect(modal(page).getByRole("button", { name: "Install extension", exact: true })).toBeDisabled();
    expect(await count(page, "extensions.applyInstall")).toBe(0);
    await read(page);
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    await expect(modal(page).getByRole("button", { name: "Install extension", exact: true })).toBeEnabled();
});
test("Long metadata, loading and download failure remain inside a narrow dialog and retry clears stale feedback", async ({ page }) => {
    await page.setViewportSize({ width: 700, height: 700 }); await open(page); await read(page);
    expect(await modal(page).evaluate((e) => e.scrollWidth - e.clientWidth)).toBeLessThanOrEqual(1);
    await page.evaluate(() => window.h4Management.failNext("extensions.planInstallFromSource", "extension_source_unavailable"));
    await modal(page).getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toContainText("The extension source could not be downloaded");
    await modal(page).getByLabel("Extension source URL", { exact: true }).fill(sourceUrl + "&new=1");
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    await expect(modal(page).getByRole("button", { name: "Install extension", exact: true })).toBeDisabled();
});
