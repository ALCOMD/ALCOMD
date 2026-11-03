import { expect, test, type Page } from "@playwright/test";
import type { DiagnosticLogItem } from "@alcomd/sdk";
import { selectSettingsSection } from "./settings-sections";
import { DIAGNOSTIC_LOG_FIXTURE_IDS as IDS } from "./diagnostic-log-client";

test.use({ viewport: { width: 1280, height: 800 } });

async function open(page: Page, route = "/diagnostics") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&diagnostic-log=1&h3-unity-discovery=1`);
    await expect.poll(() => page.evaluate(() => window.diagnosticLogRequests.length)).toBe(2);
}

async function emit(page: Page, value: Partial<DiagnosticLogItem>) {
    await page.evaluate((detail) => window.dispatchEvent(new CustomEvent("test-diagnostic-log", { detail })), value);
}

test("historical logs stay silent and a new error appears once in the shared outlet and log table", async ({ page }) => {
    await open(page);
    const table = page.getByRole("table", { name: "Diagnostics", exact: true });
    await expect(table).toContainText(IDS.historic);
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toBeHidden();
    await emit(page, { level: "error", diagnosticId: IDS.newLog });
    await expect(outlet.getByRole("alert")).toHaveCount(1);
    await expect(outlet).toContainText("Background error");
    await expect(table).toContainText(IDS.newLog);
    await outlet.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(outlet.getByText("diagnostic.error", { exact: true })).toBeVisible();
    await expect(outlet.getByText(IDS.newLog, { exact: true })).toBeVisible();
    const bounds = (await outlet.boundingBox())!;
    expect(bounds.x + bounds.width).toBe(1280 - 24);
    expect(bounds.y + bounds.height).toBe(800 - 24);
    await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(outlet).toBeHidden();
    await expect(table).toContainText(IDS.newLog);
    expect(await page.evaluate(() => window.diagnosticLogRequests.map((request) => request.cursor?.sequence))).toEqual([undefined, 1, 2]);
});

test("warning opt-in and error opt-out control popup delivery while every record stays in the log", async ({ page }) => {
    await open(page);
    const table = page.getByRole("table", { name: "Diagnostics", exact: true });
    await emit(page, { level: "warning", diagnosticId: IDS.quietWarning });
    await expect(table).toContainText(IDS.quietWarning);
    await emit(page, { level: "error", notify: false, diagnosticId: IDS.quietError });
    await expect(table).toContainText(IDS.quietError);
    await emit(page, { level: "info", notify: true, diagnosticId: IDS.quietInformation });
    await expect(table).toContainText(IDS.quietInformation);
    const outlet = page.locator(".reminder-floating");
    await expect(outlet).toBeHidden();
    await emit(page, { level: "warning", notify: true, diagnosticId: IDS.visibleWarning });
    await expect(outlet.getByRole("alert")).toHaveCount(1);
    await expect(outlet).toContainText("Background warning");
    await expect(table).toContainText(IDS.visibleWarning);
    await expect(outlet).not.toContainText(IDS.quietWarning);
});

test("the app-owned log outlet survives navigation and stays interactive inside a native modal", async ({ page }) => {
    await open(page, "/projects");
    await emit(page, { diagnosticId: IDS.navigation });
    await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(1);
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Settings$/ }) }).click();
    await selectSettingsSection(page, "unity");
    await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(1);
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    const modal = page.locator("md-dialog[open]");
    const outlet = page.locator(".reminder-floating");
    await expect(modal.locator(".reminder-floating")).toHaveCount(1);
    await outlet.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(outlet.getByText(IDS.navigation, { exact: true })).toBeVisible();
    await emit(page, { diagnosticId: IDS.modal });
    await expect(outlet.getByRole("alert")).toHaveCount(2);
    await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).first().click();
    await expect(outlet.getByRole("alert")).toHaveCount(1);
    await modal.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("body > .reminder-floating").getByRole("alert")).toHaveCount(1);
    expect(await page.evaluate(() => window.diagnosticLogRequests.filter((request) => request.cursor === undefined).length)).toBe(1);
});

test("a failed log read retains entries and reconnects from its cursor without a recursive popup", async ({ page }) => {
    await open(page);
    await page.clock.install();
    await page.evaluate(() => window.dispatchEvent(new Event("test-diagnostic-log-failure")));
    await expect(page.getByText("New logs could not be loaded. The last loaded entries are still shown.", { exact: true })).toBeVisible();
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true })).toContainText(IDS.historic);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    await page.clock.fastForward(1000);
    await expect.poll(() => page.evaluate(() => window.diagnosticLogRequests.length)).toBe(3);
    await emit(page, { diagnosticId: IDS.resumed });
    await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(1);
    await expect(page.getByText("New logs could not be loaded. The last loaded entries are still shown.", { exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.diagnosticLogRequests[2]?.cursor)).toEqual({ instanceId: IDS.coreA, sequence: 1 });
    await expect(page.locator("body")).not.toContainText("never render raw failure detail");
});

test("a restarted Core replaces its log baseline without replaying old or startup errors", async ({ page }) => {
    await open(page);
    await emit(page, { diagnosticId: IDS.oldInstance });
    await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-diagnostic-log-reset")));
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true })).toContainText(IDS.restarted);
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true })).not.toContainText(IDS.oldInstance);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    await emit(page, { diagnosticId: IDS.newInstance });
    await expect(page.locator(".reminder-floating").getByRole("alert")).toHaveCount(1);
});

test("a Core without the log capability starts no log read or cancellation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/diagnostics");
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.diagnosticLogRequests)).toEqual([]);
    expect(await page.evaluate(() => window.diagnosticLogCancels)).toBe(0);
});

test("an initial log read failure cannot be presented as an empty diagnostic history", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/diagnostics&state=empty&diagnostic-log=1&log-initial-failure=1");
    await expect(page.getByText("New logs could not be loaded. The last loaded entries are still shown.", { exact: true })).toBeVisible();
    const table = page.getByRole("table", { name: "Diagnostics", exact: true });
    await expect(table).toContainText("No diagnostic records loaded.");
    await expect(table).not.toContainText("No diagnostics reported");
    await expect(page.locator(".reminder-floating")).toBeHidden();
});
