import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Locator, type Page } from "@playwright/test";

test.use({ viewport: { width: 720, height: 450 }, deviceScaleFactor: 2 });

test("H6 skip navigation reaches the canvas and keyboard focus reveals the table's last-column action", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 450 });
    await open(page, "/repositories");
    const viewport = page.locator(".desktop-list-viewport");
    const action = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("button", { name: "Remove", exact: true });
    const initialAction = (await action.boundingBox())!;
    const initialViewport = (await viewport.boundingBox())!;
    expect(initialAction.x + initialAction.width).toBeGreaterThan(initialViewport.x + initialViewport.width);
    await page.getByRole("link", { name: "Skip to content", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(page.locator("#main-content")).toBeFocused();
    await tabTo(page, action);
    await expectFocusVisible(page, action);
    expect(await viewport.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
    expect(await page.locator("#main-content").evaluate((element) => element.scrollLeft)).toBe(0);
    expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
});

test("H6 keyboard edits inline Theme through autosave at 200 percent equivalent layout", async ({ page }) => {
    await open(page, "/settings");
    await selectSettingsSection(page, "appearance");
    await page.getByRole("link", { name: "Skip to content", exact: true }).focus();
    await page.keyboard.press("Enter");
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    const source = page.getByRole("textbox", { name: "Source color", exact: true });
    await tabTo(page, source);
    await expectFocusVisible(page, source);
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.type("#315DA8");
    await page.keyboard.press("Tab");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#315DA8");
    await expect(source).toHaveValue("#315DA8");
    await expect(theme.getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { appearance: { sourceColor: "#315DA8" } } }]);
});

async function open(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    await page.evaluate(() => document.fonts.ready);
    await expect(page.locator("#route-title")).toBeFocused();
}

async function tabTo(page: Page, target: Locator) {
    for (let step = 0; step < 48 && !await target.evaluate((element) => element.matches(":focus-within")); step++) {
        await page.keyboard.press("Tab");
    }
    await expect(target).toBeFocused();
}

async function expectFocusVisible(page: Page, target: Locator) {
    const box = (await target.boundingBox())!;
    const viewport = page.viewportSize()!;
    expect(box.width).toBeGreaterThan(0);
    expect(box.height).toBeGreaterThan(0);
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(viewport.width + 1);
    expect(box.y + box.height).toBeLessThanOrEqual(viewport.height + 1);
}
