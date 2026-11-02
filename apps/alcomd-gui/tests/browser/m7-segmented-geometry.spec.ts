import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Locator } from "@playwright/test";

async function expectSegmentsVisible(group: Locator, boundary: Locator) {
    await expect(group).toBeVisible();
    const wrapper = group.locator("..");
    await expect(wrapper).toHaveCSS("overflow", "visible");
    await expect(wrapper.locator(".os-scrollbar")).toHaveCount(0);
    const bounds = (await boundary.boundingBox())!;
    for (const segment of await group.locator("md-outlined-segmented-button").all()) {
        const box = (await segment.boundingBox())!;
        expect(box.x).toBeGreaterThanOrEqual(bounds.x);
        expect(box.x + box.width).toBeLessThanOrEqual(bounds.x + bounds.width);
        await segment.click({ trial: true });
    }
}

for (const route of ["/repositories", "/unity", "/activity"]) {
    test(`${route} desktop segments stay visible without a scroll container`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1920, height: 1000 });
        await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready&theme-mode=dark`);
        const navigation = page.locator(".resource-navigation");
        const group = navigation.locator("md-outlined-segmented-button-set");
        await expect(group.getByRole("button").first()).toBeVisible();
        await page.evaluate(async () => { await document.fonts.ready; });
        await expect(navigation).not.toHaveAttribute("data-overlayscrollbars-viewport");
        await expect(navigation).toHaveCSS("overflow", "visible");
        await expect(navigation.locator(".os-scrollbar")).toHaveCount(0);
        await expectSegmentsVisible(group, page.locator("#main-content"));
        for (const zoom of [1, 0.8, 1.25]) {
            await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
            await group.getByRole("button").first().focus();
            await expectSegmentsVisible(group, page.locator("#main-content"));
            await page.keyboard.press("Tab");
            await expect(group.getByRole("button").nth(1)).toBeFocused();
            await expectSegmentsVisible(group, page.locator("#main-content"));
            await group.screenshot({ path: testInfo.outputPath(`segments-${zoom}.png`), animations: "disabled" });
        }
    });
}

test("Chinese theme segments stay visible without a scroll container before and after selection", async ({ page }, testInfo) => {
    await page.goto("/browser-harness.html?route=/settings&theme-mode=dark");
    await selectSettingsSection(page, "appearance");
    await page.locator("#settings-locale").evaluate(element => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    const group = page.locator("#theme-mode");
    const theme = page.getByRole("form", { name: "主题", exact: true });
    await expect(group.locator("../..")).toHaveClass("theme-color-column");
    await expect(group.getByRole("button")).toHaveText(["浅色", "跟随系统", "深色"]);
    await expectSegmentsVisible(group, theme);
    for (const zoom of [1, 0.8, 1.25]) {
        await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
        for (const segment of await group.getByRole("button").all()) {
            await segment.click();
            await expect(segment).toHaveAttribute("aria-pressed", "true");
            await expectSegmentsVisible(group, theme);
        }
        await group.screenshot({ path: testInfo.outputPath(`theme-segments-${zoom}.png`), animations: "disabled" });
    }
});

test("Chinese Resources segments stay clear after selecting each section", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1600, height: 900 });
    await page.goto("/browser-harness.html?route=/settings&theme-mode=dark");
    await selectSettingsSection(page, "appearance");
    await page.locator("#settings-locale").evaluate(element => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: /^资源$/ })
    }).click();
    const group = page.locator(".resource-navigation md-outlined-segmented-button-set");
    await expectSegmentsVisible(group, page.locator("#main-content"));
    for (const zoom of [1, 0.8, 1.25]) {
        await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
        for (const name of ["存储库", "用户软件包", "模板"]) {
            const segment = group.getByRole("button", { name, exact: true });
            await segment.click();
            await expect(segment).toHaveAttribute("aria-pressed", "true");
            await expectSegmentsVisible(group, page.locator("#main-content"));
        }
        await group.screenshot({ path: testInfo.outputPath(`resources-zh-${zoom}.png`), animations: "disabled" });
    }
});

test("Resources segments remain fully usable in an 800px desktop window", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 800 });
    await page.goto("/browser-harness.html?route=/repositories&state=ready");
    const group = page.locator(".resource-navigation md-outlined-segmented-button-set");
    await expectSegmentsVisible(group, page.locator("#main-content"));
    const last = group.getByRole("button").last();
    await last.focus();
    await expect(last).toBeFocused();
    await expectSegmentsVisible(group, page.locator("#main-content"));
    await page.keyboard.press("Enter");
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toBeVisible();
});
