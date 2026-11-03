import { expect, test, type Locator, type Page } from "@playwright/test";
import guiMetadata from "../../package.json" with { type: "json" };
import { createTranslator } from "../../src/i18n";
import { requestErrorMessage } from "../../src/request-errors";
import { H5_ACTIVITY_CURSOR, H5_CHECK_ID, H5_CORE_VERSION, H5_DIAGNOSTIC_CURSOR, H5_DIAGNOSTIC_ID, H5_FAILED_ID, H5_INSTALL_ID, H5_SECRET } from "./h5-utilities-client";

test.use({ viewport: { width: 1440, height: 900 } });

async function open(page: Page, route: string, flags = "") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready&h5-utilities=1&${flags}`);
    await expect.poll(() => page.evaluate(() => typeof window.h5Utilities)).toBe("object");
}
function modal(page: Page) { return page.locator("md-dialog[open]"); }
function detail(scope: Locator, label: string) { return scope.getByText(label, { exact: true }).locator("..").locator("dd"); }
function logs(page: Page) { return page.getByRole("navigation", { name: "Logs", exact: true }); }
async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.h5Utilities.requests.filter((request) => request.method === name), method);
}

test("H5 operation detail follows one ID through a read failure and the actual failed terminal result", async ({ page }) => {
    await open(page, "/activity");
    await page.getByRole("table", { name: "Activity", exact: true }).getByRole("button", { name: "View operation", exact: true }).first().click();
    await expect(detail(modal(page), "State")).toHaveText("running");
    await page.evaluate(() => window.h5Utilities.deny("operations.get", "daemon_unavailable"));
    await expect(modal(page).getByRole("alert")).toContainText(requestErrorMessage("daemon_unavailable"));
    await expect(detail(modal(page), "Operation")).toHaveText(H5_INSTALL_ID);
    await expect(detail(modal(page), "State")).toHaveText("running");
    await page.evaluate((operationId) => {
        window.h5Utilities.complete(operationId, "failed", "extension_signature_invalid");
        window.h5Utilities.deny("operations.get");
    }, H5_INSTALL_ID);
    await expect(detail(modal(page), "State")).toHaveText("failed");
    await expect(detail(modal(page), "Error")).toHaveText(requestErrorMessage("extension_signature_invalid"));
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    expect((await requests(page, "operations.get")).every((request) => request.params.operationId === H5_INSTALL_ID)).toBe(true);
    await expect(page.locator("body")).not.toContainText(H5_SECRET);
});

test("H5 closing a pending operation dialog isolates its late failure from the next log detail", async ({ page }) => {
    await open(page, "/activity");
    await page.evaluate(() => window.h5Utilities.hold("operations.get", true));
    await page.getByRole("table", { name: "Activity", exact: true }).getByRole("button", { name: "View operation", exact: true }).first().click();
    await expect.poll(() => page.evaluate(() => window.h5Utilities.pending("operations.get"))).toBeGreaterThan(0);
    await modal(page).getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.evaluate(() => window.h5Utilities.hold("operations.get", false));
    await logs(page).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await page.getByRole("table", { name: "Technical Logs", exact: true }).getByRole("button", { name: "View operation", exact: true }).click();
    await expect(detail(modal(page), "Operation")).toHaveText(H5_FAILED_ID);
    expect(await page.evaluate(() => window.h5Utilities.release("operations.get", "permission_denied"))).toBeGreaterThan(0);
    await expect(detail(modal(page), "Operation")).toHaveText(H5_FAILED_ID);
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    await expect(modal(page)).not.toContainText(H5_INSTALL_ID);
});

test("H5 missing Operations disables dependent actions while Core diagnostic permission denial remains an error", async ({ page }) => {
    await open(page, "/activity", "h5-no-operations=1");
    const activity = page.getByRole("table", { name: "Activity", exact: true });
    await expect(activity.getByRole("button", { name: "View operation", exact: true }).first()).toBeDisabled();
    await logs(page).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await expect(page.getByRole("table", { name: "Technical Logs", exact: true }).getByRole("button", { name: "View operation", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Run state check", exact: true })).toBeDisabled();
    expect(await requests(page, "operations.get")).toHaveLength(0);
    expect(await requests(page, "state.check")).toHaveLength(0);

    await open(page, "/activity");
    await page.evaluate(() => window.h5Utilities.deny("diagnostics.list", "permission_denied"));
    await logs(page).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await expect(page.locator("main").getByRole("alert")).toContainText(requestErrorMessage("permission_denied"));
    await expect(page.locator("main")).not.toContainText("No technical logs yet");
    await expect(page.locator("body")).not.toContainText(H5_SECRET);
    await logs(page).getByRole("button", { name: "Activity", exact: true }).click();
    await expect(page.getByRole("table", { name: "Activity", exact: true }).locator("tbody tr")).toHaveCount(2);
});

for (const surface of [
    { route: "/activity", label: "Activity", method: "activity.list", initial: 2, cursor: H5_ACTIVITY_CURSOR },
    { route: "/diagnostics", label: "Technical Logs", method: "diagnostics.list", initial: 1, cursor: H5_DIAGNOSTIC_CURSOR }
]) {
    test(`H5 ${surface.label} pagination retains loaded rows after failure and retries its same cursor`, async ({ page }) => {
        await open(page, surface.route);
        const table = page.getByRole("table", { name: surface.label, exact: true });
        await expect(table.locator("tbody tr")).toHaveCount(surface.initial);
        await page.evaluate((method) => window.h5Utilities.failNext(method, "store_unavailable"), surface.method);
        await page.getByRole("button", { name: "Load more", exact: true }).click();
        await expect(page.locator(".reminder-floating").getByRole("alert")).toContainText(requestErrorMessage("store_unavailable"));
        await expect(table.locator("tbody tr")).toHaveCount(surface.initial);
        await page.getByRole("button", { name: "Retry loading more", exact: true }).click();
        await expect(table.locator("tbody tr")).toHaveCount(surface.initial + 1);
        await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(0);
        const pages = (await requests(page, surface.method)).filter((request) => request.params.cursor !== undefined);
        expect(pages.map((request) => request.params.cursor)).toEqual([surface.cursor, surface.cursor]);
    });
}

for (const locale of ["zh-CN", "ja-JP"] as const) {
    test(`H5 ${locale} shows authored activity reasons and a complete long diagnostic identity without overflow`, async ({ page }) => {
        const t = createTranslator(locale);
        await open(page, "/activity", `h5-locale=${locale}`);
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        const table = page.getByRole("table", { name: t("Activity"), exact: true });
        for (const [kind, state] of [["Install extension", "running"], ["Check state", "succeeded"]] as const) {
            const expected = t("operation {kind} {state}", { kind: t(kind), state: t(state) });
            await expect(table.getByText(expected, { exact: true })).toBeVisible();
            expect(t(kind)).not.toBe(kind);
        }
        await page.getByRole("button", { name: t("Load more"), exact: true }).click();
        await expect(table.getByText(t("operation {kind} {state}", { kind: t("Uninstall extension"), state: t("failed") }), { exact: true })).toBeVisible();
        await page.getByRole("navigation", { name: t("Logs"), exact: true }).getByRole("button", { name: t("Technical Logs"), exact: true }).click();
        const diagnostics = page.getByRole("table", { name: t("Technical Logs"), exact: true });
        const readable = t(requestErrorMessage("extension_signature_invalid"));
        expect(readable).not.toBe(requestErrorMessage("extension_signature_invalid"));
        await expect(diagnostics.locator(".logs-summary strong").first()).toHaveText(readable);
        await expect(diagnostics).toContainText(H5_DIAGNOSTIC_ID);
        await expect(diagnostics).toContainText("extension_signature_invalid");
        expect(await page.locator("main").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
        await diagnostics.getByRole("button", { name: t("View operation"), exact: true }).click();
        await expect(detail(modal(page), t("Diagnostic ID"))).toHaveText(H5_DIAGNOSTIC_ID);
    });
}

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
    test(`About refresh keeps its touch target inside the content at ${width}px`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height: 800 });
        await open(page, "/about");
        const body = page.locator(".utility-workspace-body");
        const bar = body.locator(".refresh-bar");
        const refresh = bar.getByRole("button", { name: "Refresh", exact: true });
        await expect(detail(body, "Daemon")).toHaveText(H5_CORE_VERSION);
        await expect(body).toHaveAttribute("data-overlayscrollbars-viewport", /./);
        await expect.poll(() => body.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0);
        await expect(body.locator(":scope > .os-scrollbar-horizontal.os-scrollbar-visible")).toHaveCount(0);
        const bounds = (await bar.boundingBox())!;
        const touch = await bar.locator("md-icon-button").evaluate(element => element.shadowRoot!.querySelector(".touch")!.getBoundingClientRect().toJSON());
        expect(touch.width).toBe(48);
        expect(touch.height).toBe(48);
        expect(touch.left).toBeGreaterThanOrEqual(bounds.x);
        expect(touch.right).toBeLessThanOrEqual(bounds.x + bounds.width);
        expect(touch.top).toBeGreaterThanOrEqual(bounds.y);
        expect(touch.bottom).toBeLessThanOrEqual(bounds.y + bounds.height);
        const before = (await requests(page, "system.status")).length;
        await refresh.focus();
        await page.keyboard.press("Enter");
        await expect.poll(async () => (await requests(page, "system.status")).length).toBe(before + 1);
        await expect(refresh).toBeEnabled();
        await expect(page.getByRole("heading", { name: "Licenses", exact: true })).toBeVisible();
        await expect.poll(() => body.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0);
        await page.evaluate(() => window.h5Utilities.failNext("system.status", "store_unavailable"));
        await refresh.click();
        await expect(bar.locator(".inline-error")).toContainText(requestErrorMessage("store_unavailable"));
        await expect(detail(body, "Daemon")).toHaveText(H5_CORE_VERSION);
        await expect.poll(() => body.evaluate(element => element.scrollWidth - element.clientWidth)).toBe(0);
        await page.screenshot({ path: testInfo.outputPath(`about-${width}.png`), animations: "disabled" });
    });
}

test("H5 state check accepts one operation and follows its actual terminal state", async ({ page }) => {
    await open(page, "/diagnostics");
    const start = page.getByRole("button", { name: "Run state check", exact: true });
    const initialReads = (await requests(page, "diagnostics.list")).length;
    await start.click();
    await expect(page.locator(".operation-follow")).toContainText("running");
    await expect(start).toBeDisabled();
    expect(await requests(page, "state.check")).toHaveLength(1);
    await page.evaluate((operationId) => window.h5Utilities.complete(operationId, "succeeded"), H5_CHECK_ID);
    await expect(page.locator(".operation-follow")).toContainText("succeeded");
    await expect(start).toBeEnabled();
    await expect.poll(async () => (await requests(page, "diagnostics.list")).length).toBeGreaterThan(initialReads);
    expect((await requests(page, "operations.get")).every((request) => request.params.operationId === H5_CHECK_ID)).toBe(true);
    expect(await requests(page, "state.check")).toHaveLength(1);
});
