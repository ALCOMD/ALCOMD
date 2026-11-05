import { expectRequestError } from "./request-error-assertions";
import { expect, test, type Page } from "@playwright/test";
import type { ExtensionUiDispatchParams } from "@alcomd/sdk";
import "./h4-portable-client";

const discord = "com.vrclearn.discord";
const mcp = "com.vrclearn.mcp-management";
const uiRoute = (extensionId: string) => `/extensions/${extensionId}/ui`;

test.use({ viewport: { width: 1440, height: 900 } });

async function openUi(page: Page, extensionId = discord, extra = "") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(uiRoute(extensionId))}&state=ready&h4-portable=1${extra}`);
}

async function navigate(page: Page, route: string) {
    await page.evaluate((next) => {
        window.history.pushState(null, "", next);
        window.dispatchEvent(new PopStateEvent("popstate"));
    }, route);
}

async function pending(page: Page, method: "open" | "identity" | "refresh" | "dispatch" | "settings") {
    await expect.poll(() => page.evaluate((kind) => window.h4Portable.pending.filter((read) => read.method === kind).length, method)).toBeGreaterThan(0);
}

async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.h4Portable.requests.filter((request) => request.method === name), method);
}

async function expectHostErrorsCleared(page: Page) {
    await expect(page.locator(".extension-body [role=alert]:not(.portable-document [role=alert])")).toHaveCount(0);
    // This guest field validation is part of the existing public fixture. A
    // successful host request must not erase an extension's field-level error.
    await expect(page.getByLabel("Custom detail", { exact: true })).toHaveAccessibleDescription(/The host rejected the previous value\./);
}

test("H4 Portable UI closes an open result that arrives after its route has been left", async ({ page }) => {
    await openUi(page, discord, "&h4-hold=open");
    await pending(page, "open");
    const oldSession = await page.evaluate(() => window.h4Portable.pending.find((read) => read.method === "open")!.sessionId!);
    await page.evaluate(() => window.h4Portable.hold("open", false));
    await navigate(page, uiRoute(mcp));
    await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
    await page.evaluate(() => window.h4Portable.release("open"));
    await expect.poll(async () => (await requests(page, "extensions.ui.close")).some((request) => (request.params as { sessionId: string }).sessionId === oldSession)).toBe(true);
    await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toHaveCount(0);
});

test("H4 Portable UI ignores a late identity read from the prior locale session", async ({ page }) => {
    await openUi(page, discord, "&h4-hold=identity,settings");
    await pending(page, "identity");
    await pending(page, "settings");
    await page.evaluate(() => {
        window.h4Portable.hold("identity", false);
        window.h4Portable.hold("settings", false);
        window.h4Portable.locale = "ja-JP";
        window.h4Portable.releaseAll("settings");
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "ja-JP");
    // Same-resource reads remain single-flight; release the old read before the new identity can be queried.
    await page.evaluate(() => window.h4Portable.release("identity"));
    await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toHaveCount(0);
    await expect(page.locator(".extension-identity")).toContainText("1.0.2");
    const opens = await requests(page, "extensions.ui.open");
    expect(opens.map((request) => (request.params as { locale: string }).locale)).toEqual(["en-US", "ja-JP"]);
});

for (const method of ["dispatch"] as const) {
    test(`H4 Portable UI ignores late ${method} completion without clearing the next page's dirty form`, async ({ page }) => {
        await openUi(page, mcp);
        await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
        await page.evaluate((kind) => window.h4Portable.hold(kind, true), method);
        await page.locator(".portable-document").getByRole("button", { name: "Refresh", exact: true }).click();
        await pending(page, method);
        await navigate(page, uiRoute(discord));
        await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
        await page.getByLabel("Enable presence", { exact: true }).uncheck();
        await page.evaluate((kind) => window.h4Portable.release(kind), method);
        await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
        await page.locator("#primary-navigation md-list-item").filter({
            has: page.locator(".navigation-item-label", { hasText: /^Settings$/ })
        }).click();
        await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toBeVisible();
        await page.getByRole("button", { name: "Keep editing", exact: true }).click();
        await expect(page.getByLabel("Enable presence", { exact: true })).not.toBeChecked();
        await expectHostErrorsCleared(page);
    });
}

test("H4 Portable UI has no host refresh and dispatches the current session revision", async ({ page }) => {
    await openUi(page);
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
    await expect(page.locator('.extension-chrome [data-icon-name="refresh"]')).toHaveCount(0);
    await page.getByLabel("Enable presence", { exact: true }).uncheck();
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await requests(page, "extensions.ui.dispatch")).length).toBe(1);
    const dispatch = (await requests(page, "extensions.ui.dispatch"))[0]!.params as ExtensionUiDispatchParams;
    expect(dispatch).toMatchObject({ sessionId: "h4-session-1", sequence: 1, expectedSnapshotRevision: 1 });
    expect(await requests(page, "extensions.ui.refresh")).toHaveLength(0);
});

test("H4 Portable UI preserves the dirty draft after a conflict without inserting a page prompt", async ({ page }) => {
    await openUi(page);
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
    await page.getByLabel("Enable presence", { exact: true }).uncheck();
    await page.evaluate(() => window.h4Portable.failNext("dispatch", "revision_conflict"));
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await requests(page, "extensions.ui.dispatch")).length).toBe(1);
    await expectHostErrorsCleared(page);
    await expect(page.getByRole("button", { name: "Reconnect", exact: true })).toHaveCount(0);
    await navigate(page, "/projects");
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Keep editing", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toBeHidden();
    await expect(page.getByLabel("Enable presence", { exact: true })).not.toBeChecked();
    await navigate(page, "/projects");
    await page.getByRole("button", { name: "Discard changes", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await navigate(page, uiRoute(discord));
    await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
    await expect(page.getByLabel("Enable presence", { exact: true })).toBeChecked();
    expect(await requests(page, "extensions.ui.refresh")).toHaveLength(0);
});

for (const code of ["extension_permission_denied", "extension_quarantined", "extension_ui_document_invalid", "extension_crashed"]) {
    test(`H4 Portable UI removes unusable controls after ${code} and reopens without replaying an action`, async ({ page }) => {
        await openUi(page);
        await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
        await page.getByLabel("Enable presence", { exact: true }).uncheck();
        await page.evaluate((errorCode) => window.h4Portable.failNext("dispatch", errorCode), code);
        await page.getByRole("button", { name: "Save", exact: true }).click();
        await expectRequestError(page.locator(".extension-body [role=alert]:not(.portable-document [role=alert])"), code);
        await expect(page.locator(".portable-document")).toHaveCount(0);
        expect(await requests(page, "extensions.ui.dispatch")).toHaveLength(1);
        await expect.poll(async () => (await requests(page, "extensions.ui.close")).some((request) => (request.params as { sessionId: string }).sessionId === "h4-session-1")).toBe(true);
        await page.getByRole("button", { name: "Reconnect", exact: true }).click();
        await expect(page.getByRole("heading", { name: "Fixture h4-session-2", exact: true })).toBeVisible();
        await expectHostErrorsCleared(page);
        await expect(page.getByLabel("Enable presence", { exact: true })).toBeChecked();
        expect(await requests(page, "extensions.ui.dispatch")).toHaveLength(1);
        await page.getByRole("button", { name: "Save", exact: true }).click();
        await expect.poll(async () => (await requests(page, "extensions.ui.dispatch")).length).toBe(2);
        expect((await requests(page, "extensions.ui.dispatch"))[1]!.params).toMatchObject({ sessionId: "h4-session-2", sequence: 1, expectedSnapshotRevision: 1 });
    });
}

test("H4 Portable UI keeps a refused action's draft and only retries on an explicit new submit", async ({ page }) => {
    await openUi(page);
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
    const interval = page.getByLabel("Refresh interval seconds", { exact: true });
    const form = page.locator(".portable-form");
    const before = await form.boundingBox();
    await interval.fill("-1");
    await form.dispatchEvent("submit");
    expect(await requests(page, "extensions.ui.dispatch")).toHaveLength(0);
    expect((await form.boundingBox())!.height).toBe(before!.height);
    await expect(form).not.toContainText("Check the editable fields before submitting.");
    await interval.fill("15");
    await page.getByLabel("Enable presence", { exact: true }).uncheck();
    await page.evaluate(() => window.h4Portable.failNext("dispatch", "extension_ui_action_invalid"));
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect.poll(async () => (await requests(page, "extensions.ui.dispatch")).length).toBe(1);
    await expectHostErrorsCleared(page);
    await expect(page.getByLabel("Enable presence", { exact: true })).not.toBeChecked();
    expect(await requests(page, "extensions.ui.dispatch")).toHaveLength(1);
    expect(await requests(page, "extensions.ui.close")).toHaveLength(0);
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expectHostErrorsCleared(page);
    const dispatches = (await requests(page, "extensions.ui.dispatch")).map((request) => request.params as ExtensionUiDispatchParams);
    expect(dispatches).toHaveLength(2);
    expect(dispatches.every((request) => request.sequence === 1 && request.expectedSnapshotRevision === 1)).toBe(true);
    expect(dispatches[0]!.requestId).not.toBe(dispatches[1]!.requestId);
});

test("H4 Portable UI reconnect negotiates changed capabilities and never repeats a disconnected action", async ({ page }) => {
    await openUi(page);
    await expect(page.getByRole("heading", { name: "Fixture h4-session-1", exact: true })).toBeVisible();
    const initialStatusReads = (await requests(page, "system.status")).length;
    await page.evaluate(() => window.h4Portable.failNext("dispatch", "daemon_unavailable"));
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expectRequestError(page.locator(".extension-body [role=alert]:not(.portable-document [role=alert])"), "daemon_unavailable");
    await expect(page.locator(".portable-document")).toHaveCount(0);
    await page.evaluate(() => { window.h4Portable.removeUiCapability = true; });
    await page.getByRole("button", { name: "Reconnect", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Feature unavailable", exact: true })).toBeVisible();
    await expect(page.getByText("extensions.ui.portable.v1 was not negotiated by the connected daemon.", { exact: true })).toBeVisible();
    expect((await requests(page, "system.status")).length).toBeGreaterThan(initialStatusReads);
    expect(await requests(page, "extensions.ui.open")).toHaveLength(1);
    expect(await requests(page, "extensions.ui.dispatch")).toHaveLength(1);
});
