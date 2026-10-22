import { expect, test, type Page } from "@playwright/test";

declare global {
    interface Window {
        pageAnimationCalls: number;
        pageFrames: { populated: boolean; opaque: boolean; settings: boolean; unityRows: number }[];
    }
}

test.use({ viewport: { width: 1440, height: 900 }, video: { mode: "on", size: { width: 1440, height: 900 } } });

async function open(page: Page, params = "") {
    await page.goto(`/browser-harness.html?route=%2Fprojects&state=ready&${params}`);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.locator(".route-content").evaluate((content) => {
        window.pageAnimationCalls = 0;
        const animate = Element.prototype.animate;
        Element.prototype.animate = function (frames, options) {
            if (content.contains(this)) window.pageAnimationCalls += 1;
            return animate.call(this, frames, options);
        };
    });
}

function navigation(page: Page, name: string) {
    return page.locator("#primary-navigation .navigation-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) })
    });
}

async function frames(page: Page) {
    await page.evaluate(() => {
        window.pageFrames = [];
        const sample = () => {
            const content = document.querySelector(".route-content")!;
            const settings = content.querySelector(".settings-workspace");
            window.pageFrames.push({
                populated: content.querySelector("table, .settings-workspace, .logs-workspace") !== null,
                opaque: [content, content.firstElementChild!].every((node) => getComputedStyle(node).opacity === "1"),
                settings: settings !== null,
                unityRows: settings?.querySelectorAll('table[aria-label="Unity installations"] tbody tr').length ?? 0
            });
            requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
    });
}

for (const origin of ["Projects", "Resources", "Logs"]) {
    test(`${origin} to Settings keeps populated frames until the settings read settles`, async ({ page }) => {
        await open(page, "holdSettings=1");
        if (origin !== "Projects") {
            await navigation(page, origin).click();
            await expect(page.getByRole("heading", { level: 1, name: origin, exact: true })).toBeFocused();
        }
        await frames(page);
        await navigation(page, "Settings").click();
        await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
        await expect(page.locator(".route-content")).toHaveAttribute("inert", "");
        await expect(page.locator(".route-content #route-title")).toHaveText(origin);
        await expect(page.locator(".route-content .settings-workspace")).toHaveCount(0);
        await page.waitForTimeout(220);
        await expect(page.locator(".page-navigation-progress .loading-state-indicator")).toBeVisible();
        await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
        await expect(page.getByRole("table", { name: "Unity installations", exact: true }).locator("tbody tr")).toHaveCount(1);
        await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
        const samples = await page.evaluate(() => window.pageFrames);
        expect(samples.every((frame) => frame.populated && frame.opaque && (!frame.settings || frame.unityRows === 1))).toBe(true);
        expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
    });
}

test("Settings waits for Unity and repository options without exposing a partial page", async ({ page }) => {
    await open(page, "holdSettings=1&holdUnity=1&holdRepositories=1");
    await frames(page);
    await navigation(page, "Settings").click();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
    await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
    await page.waitForTimeout(100);
    await expect(page.locator(".route-content .projects-page")).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new Event("test-unity-ready")));
    await page.waitForTimeout(100);
    await expect(page.locator(".route-content .settings-workspace")).toHaveCount(0);
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
    await expect(page.locator(".settings-source-options")).toContainText("Example packages");
    expect(await page.evaluate(() => window.pageFrames.every((frame) => frame.populated && frame.opaque && (!frame.settings || frame.unityRows === 1)))).toBe(true);
});

test("abandoned pending reads cannot replace the latest route", async ({ page }) => {
    await open(page, "holdSettings=1");
    await navigation(page, "Settings").click();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
    await navigation(page, "Resources").click();
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
    await page.evaluate(async () => {
        window.dispatchEvent(new Event("test-settings-ready"));
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    });
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeFocused();
    await expect(page.locator(".settings-workspace")).toHaveCount(0);
    expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
});

test("a failed pending read commits a real error with a retry action", async ({ page }) => {
    await open(page, "holdRepositories=1");
    await navigation(page, "Resources").click();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-repositories-ready", { detail: "error" })));
    await expect(page.getByRole("alert")).toContainText("daemon_unavailable");
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeEnabled();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
});

test("Settings keeps healthy sections and an explicit auxiliary read error", async ({ page }) => {
    await open(page, "holdRepositories=1");
    await navigation(page, "Settings").click();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-repositories-ready", { detail: "error" })));
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
    await expect(page.getByRole("table", { name: "Unity installations", exact: true }).locator("tbody tr")).toHaveCount(1);
    await expect(page.getByRole("alert")).toContainText("Repository visibility options could not be loaded.");
    await expect(page.getByRole("button", { name: "Retry repository options", exact: true })).toBeEnabled();
});

test("quick reads and snapshot refresh never start page animations or replace current rows with loading", async ({ page }) => {
    await open(page);
    await navigation(page, "Resources").click();
    const rows = page.getByRole("table", { name: "Repositories", exact: true });
    await expect(rows).toBeVisible();
    const table = await rows.elementHandle();
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    expect(await table!.evaluate((element) => element.isConnected)).toBe(true);
    expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
    await expect(page.locator(".loading-state")).toHaveCount(0);
});

test("filter opens without custom motion and Escape restores focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects%2F00000000-0000-4000-8000-000000000101%2Fpackages&state=package-multiple");
    const trigger = page.getByRole("button", { name: "Filter packages", exact: true });
    await trigger.click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("transition-duration", "0s");
    await expect(panel).toHaveCSS("transform", "none");
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
});
