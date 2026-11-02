import { expect, test, type Locator } from "@playwright/test";

async function expectSegmentsFit(viewport: Locator) {
    await expect(viewport).toHaveAttribute("data-overlayscrollbars-viewport", /./);
    const size = await viewport.evaluate(element => ({
        width: element.clientWidth, contentWidth: element.scrollWidth,
        height: element.clientHeight, contentHeight: element.scrollHeight
    }));
    expect(size.contentWidth).toBeLessThanOrEqual(size.width);
    expect(size.contentHeight).toBeLessThanOrEqual(size.height);
    await expect(viewport.locator(".os-scrollbar-visible")).toHaveCount(0);
    // Read a single frame: the theme dialog can still be opening at this point.
    const geometry = await viewport.evaluate(element => ({
        bounds: element.getBoundingClientRect().toJSON(),
        targets: Array.from(element.querySelectorAll("md-outlined-segmented-button"), segment =>
            segment.shadowRoot!.querySelector(".md3-segmented-button__touch")!.getBoundingClientRect().toJSON())
    }));
    for (const touch of geometry.targets) {
        expect(touch.top).toBeGreaterThanOrEqual(geometry.bounds.top);
        expect(touch.bottom).toBeLessThanOrEqual(geometry.bounds.bottom);
    }
}

for (const route of ["/repositories", "/unity", "/activity"]) {
    test(`${route} segments keep touch targets and focus inside the scroll viewport`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1920, height: 1000 });
        await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready&theme-mode=dark`);
        const viewport = page.locator(".resource-navigation");
        await expect(viewport.getByRole("button").first()).toBeVisible();
        await page.evaluate(async () => { await document.fonts.ready; });
        await expectSegmentsFit(viewport);
        for (const zoom of [1, 0.8, 1.25]) {
            await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
            await viewport.getByRole("button").first().focus();
            await expectSegmentsFit(viewport);
            await page.keyboard.press("Tab");
            await expect(viewport.getByRole("button").nth(1)).toBeFocused();
            await expectSegmentsFit(viewport);
            await viewport.screenshot({ path: testInfo.outputPath(`segments-${zoom}.png`), animations: "disabled" });
        }
    });
}

test("Chinese theme segments fit their scroll viewport before and after selection", async ({ page }, testInfo) => {
    await page.goto("/browser-harness.html?route=/settings&theme-mode=dark");
    await page.locator("#settings-locale").evaluate(element => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await page.locator("#settings-theme-button").click();
    const viewport = page.locator(".theme-mode-scroll");
    await expectSegmentsFit(viewport);
    for (const zoom of [1, 0.8, 1.25]) {
        await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
        for (const segment of await viewport.getByRole("button").all()) {
            await segment.click();
            await expect(segment).toHaveAttribute("aria-pressed", "true");
            await expectSegmentsFit(viewport);
        }
        await viewport.screenshot({ path: testInfo.outputPath(`theme-segments-${zoom}.png`), animations: "disabled" });
    }
});

test("Chinese Resources segments stay clear after selecting each section", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1600, height: 900 });
    await page.goto("/browser-harness.html?route=/settings&theme-mode=dark");
    await page.locator("#settings-locale").evaluate(element => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: /^资源$/ })
    }).click();
    const viewport = page.locator(".resource-navigation");
    await expectSegmentsFit(viewport);
    for (const zoom of [1, 0.8, 1.25]) {
        await page.evaluate(value => { document.documentElement.style.zoom = String(value); }, zoom);
        for (const name of ["存储库", "用户软件包", "模板"]) {
            const segment = viewport.getByRole("button", { name, exact: true });
            await segment.click();
            await expect(segment).toHaveAttribute("aria-pressed", "true");
            await expectSegmentsFit(viewport);
        }
        await viewport.screenshot({ path: testInfo.outputPath(`resources-zh-${zoom}.png`), animations: "disabled" });
    }
});

test("narrow Resources scroll horizontally for real content without vertical overflow", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 800 });
    await page.goto("/browser-harness.html?route=/repositories&state=ready");
    const viewport = page.locator(".resource-navigation");
    await expect(viewport).toHaveAttribute("data-overlayscrollbars-viewport", /./);
    const size = await viewport.evaluate(element => [element.clientWidth, element.scrollWidth, element.clientHeight, element.scrollHeight]);
    expect(size[1]!).toBeGreaterThan(size[0]!);
    expect(size[3]!).toBeLessThanOrEqual(size[2]!);
    await expect(viewport.locator(".os-scrollbar-vertical.os-scrollbar-visible")).toHaveCount(0);
    const last = viewport.getByRole("button").last();
    await last.focus();
    await expect(last).toBeFocused();
    expect(await viewport.evaluate(element => element.scrollLeft)).toBeGreaterThan(0);
    const bounds = (await viewport.boundingBox())!, button = (await last.boundingBox())!;
    expect(button.x).toBeGreaterThanOrEqual(bounds.x);
    expect(button.x + button.width).toBeLessThanOrEqual(bounds.x + bounds.width);
    await page.keyboard.press("Enter");
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toBeVisible();
});
