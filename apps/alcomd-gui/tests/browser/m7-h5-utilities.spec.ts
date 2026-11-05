import { expect, test, type Locator, type Page } from "@playwright/test";
import guiMetadata from "../../package.json" with { type: "json" };
import { createTranslator } from "../../src/i18n";
import { requestErrorMessage } from "../../src/request-errors";
import { H5_CHECK_ID, H5_CORE_VERSION, H5_DIAGNOSTIC_ID, H5_FAILED_ID, H5_INSTALL_ID, H5_SECRET } from "./h5-utilities-client";

test.use({ viewport: { width: 1440, height: 900 } });

async function open(page: Page, route: string, flags = "") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready&h5-utilities=1&${flags}`);
    await expect.poll(() => page.evaluate(() => typeof window.h5Utilities)).toBe("object");
}
function modal(page: Page) { return page.locator("md-dialog[open]"); }
function detail(scope: Locator, label: string) { return scope.getByText(label, { exact: true }).locator("..").locator("dd"); }
async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.h5Utilities.requests.filter((request) => request.method === name), method);
}

test("H5 task detail follows one ID through a read failure and the actual failed terminal result", async ({ page }) => {
    await open(page, "/operations/" + H5_INSTALL_ID);
    await expect(page.locator(".operation-follow")).toContainText("running");
    await page.evaluate((operationId) => {
        window.h5Utilities.deny("operations.get", "daemon_unavailable");
        window.h5Utilities.recordOperation({ operationId, kind: "extensions.install", state: "running", revision: 2,
            createdAtMs: 1_700_000_000_000, updatedAtMs: 1_700_000_003_000, progress: { phase: "prepared" } });
    }, H5_INSTALL_ID);
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeVisible();
    await expect(page.locator(".operation-follow")).not.toContainText("Progress is temporarily unavailable");
    await expect(page.locator(".operation-follow")).toContainText("running");
    await page.evaluate((operationId) => {
        window.h5Utilities.complete(operationId, "failed", "extension_signature_invalid");
        window.h5Utilities.deny("operations.get");
    }, H5_INSTALL_ID);
    await expect(page.locator(".operation-follow")).toContainText("failed");
    await expect(page.getByRole("alert")).toContainText(requestErrorMessage("extension_signature_invalid"));
    expect((await requests(page, "operations.get")).every((request) => request.params.operationId === H5_INSTALL_ID)).toBe(true);
    await expect(page.locator("body")).not.toContainText(H5_SECRET);
});

test("H5 leaving a pending task isolates its late failure from another task", async ({ page }) => {
    await open(page, "/operations");
    await page.evaluate(() => window.h5Utilities.hold("operations.get", true));
    await page.locator('tr[data-row-key="' + H5_INSTALL_ID + '"]').getByRole("button", { name: "View operation", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.get"))).toBeGreaterThan(0);
    await page.getByRole("navigation", { name: "Utilities", exact: true }).locator("md-list-item").filter({ hasText: /^Tasks/ }).click();
    await page.evaluate(() => window.h5Utilities.hold("operations.get", false));
    await page.locator('tr[data-row-key="' + H5_CHECK_ID + '"]').getByRole("button", { name: "View operation", exact: true }).click();
    await expect(page.locator(".operation-follow")).toContainText("succeeded");
    await page.evaluate(() => window.h5Utilities.release("operations.get", "permission_denied"));
    await expect(page.locator(".operation-follow")).toContainText("succeeded");
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
});

test("retired activity and diagnostic routes do not call their old interfaces", async ({ page }) => {
    for (const route of ["/activity", "/diagnostics"]) {
        await open(page, route);
        await expect(page.getByRole("heading", { name: "Page not found", exact: true })).toBeVisible();
        await expect(page.getByRole("navigation", { name: "Primary", exact: true }).getByText("Logs", { exact: true })).toBeVisible();
        expect(await requests(page, "activity.list")).toHaveLength(0);
        expect(await requests(page, "diagnostics.log.read")).toHaveLength(0);
    }
});

test("H5 About keeps local GUI version and licenses available with slow Operations and an offline Core", async ({ page }) => {
    await open(page, "/about", "h5-operations-slow=1");
    const main = page.locator("main");
    await expect(detail(main, "GUI version")).toHaveText(guiMetadata.version);
    await expect(detail(main, "Daemon")).toHaveText(H5_CORE_VERSION);
    expect(H5_CORE_VERSION).not.toBe(guiMetadata.version);
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.list"))).toBeGreaterThan(0);
    await expect(page.locator(".navigation-footer .navigation-item-meta").last()).toHaveText(guiMetadata.version);
    await expect(page.getByRole("heading", { name: "Licenses", exact: true })).toBeVisible();

    await open(page, "/about", "h5-status-offline=1");
    await expect(detail(main, "GUI version")).toHaveText(guiMetadata.version);
    await expect(page.getByRole("heading", { name: "Licenses", exact: true })).toBeVisible();
    await expect(main).toContainText("AGPL-3.0-only");
    await expect(main.getByRole("alert")).toContainText(requestErrorMessage("daemon_unavailable"));
    await expect(page.locator(".navigation-footer .navigation-item-meta").last()).toHaveText(guiMetadata.version);
    await expect(main).not.toContainText(H5_CORE_VERSION);
    await expect(page.locator("body")).not.toContainText(H5_SECRET);
});

for (const width of [800, 1280]) {
    test(`About shows version and licenses without a redundant refresh at ${width}px`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height: 800 });
        await open(page, "/about");
        const body = page.locator(".utility-workspace-body");
        await expect(detail(body, "Daemon")).toHaveText(H5_CORE_VERSION);
        await expect(body).toHaveAttribute("data-overlayscrollbars-viewport", /./);
        await expect.poll(() => body.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0);
        await expect(body.locator(":scope > .os-scrollbar-horizontal.os-scrollbar-visible")).toHaveCount(0);
        await expect(page.getByRole("main").getByRole("button", { name: "Refresh", exact: true })).toHaveCount(0);
        await expect(page.getByRole("heading", { name: "Licenses", exact: true })).toBeVisible();
        await page.screenshot({ path: testInfo.outputPath(`about-${width}.png`), animations: "disabled" });
    });
}
