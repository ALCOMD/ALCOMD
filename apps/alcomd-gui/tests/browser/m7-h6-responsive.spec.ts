import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Locator, type Page } from "@playwright/test";

for (const width of [320, 640, 800, 1024, 1280]) {
    test(`H6 ${width}px keeps the existing navigation and common page controls reachable`, async ({ page }) => {
        await page.setViewportSize({ width, height: 800 });
        await openHarness(page, "/projects");
        const navigation = page.locator("#primary-navigation");
        expect((await navigation.boundingBox())?.width).toBe(width <= 1100 ? 80 : 260);
        for (const label of ["Projects", "Resources", "Settings", "Logs", "Extensions", "Tasks", "About"]) {
            const item = navigationItem(page, label);
            const name = label === "Tasks" ? /^Tasks(?: \d+ active)?$/ : label === "About" ? /^About \S+$/ : label;
            await expect(item).toHaveAccessibleName(name);
            if (width <= 1100) await expect(item).toHaveAttribute("title", label === "About" ? /^About \S+$/ : label);
        }
        await expectCanvasFits(page);
        await expectHeaderFits(page);
        await expectReachable(page, page.getByRole("button", { name: "Refresh projects", exact: true }));
        await expectReachable(page, page.getByLabel("Search projects", { exact: true }));
        await expectReachable(page, page.getByRole("button", { name: "Create project", exact: true }));
        await expectReachable(page, page.getByRole("button", { name: "More project creation options", exact: true }));
        if (width === 1280) await expectPrimaryEdgeOpensDialog(page, "Create project");

        await test.step("Resources keeps its tools visible and the table locally scrollable", async () => {
            await openHarness(page, "/repositories");
            await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
            await expectCanvasFits(page);
            // Product decision: segmented navigation may overflow at 320px.
            // Keep the independent action and table checks below at that width.
            if (width > 320) await expectHeaderFits(page);
            for (const name of ["Add repository", "Repository list actions", "Refresh"]) {
                await expectReachable(page, page.getByRole("button", { name, exact: true }));
            }
            if (width === 320) await expectPrimaryEdgeOpensDialog(page, "Add repository");
            const list = page.locator(".desktop-list-viewport");
            const table = page.getByRole("table", { name: "Repositories", exact: true });
            const viewportBox = (await list.boundingBox())!;
            const tableBox = (await table.boundingBox())!;
            // A two-dimensional table retains its existing local horizontal scroll contract.
            if (tableBox.width > viewportBox.width + 1) {
                await page.mouse.move(viewportBox.x + viewportBox.width / 2, viewportBox.y + Math.min(80, viewportBox.height / 2));
                await page.mouse.wheel(120, 0);
                await expect.poll(() => list.evaluate((element) => element.scrollLeft)).toBeGreaterThan(0);
                const browse = page.getByRole("button", { name: "Browse packages", exact: true });
                const control = (await browse.boundingBox())!;
                if (control.x < viewportBox.x - 1 || control.x + control.width > viewportBox.x + viewportBox.width + 1) {
                    await wheelHorizontallyIntoView(page, browse, list);
                }
            }
            await expectReachable(page, page.getByRole("button", { name: "Browse packages", exact: true }), list);
            if (width === 320) await browserScreenshot(page, "browser-resources-320.png");
        });

        await test.step("Settings reflows its ordinary rows without page-wide horizontal scrolling", async () => {
            await openHarness(page, "/settings");
            await expect(page.locator("#settings-motion")).toBeAttached();
            const body = page.locator(".utility-workspace-body");
            await expectCanvasFits(page);
            // The owner permits the wider Settings segments to overflow at 640px
            // and below; keep all independent body and action checks at those sizes.
            if (width > 640) await expectHeaderFits(page);
            await expectNoHorizontalOverflow(body);
            if (width === 320) {
                // Only the default group is required at this intentionally narrow width.
                await expectReachable(page, page.getByRole("textbox", { name: "Default project location", exact: true }), body);
                await expectReachable(page, page.getByRole("textbox", { name: "Default backup location", exact: true }), body);
            } else {
                await selectSettingsSection(page, "general");
                for (const checkbox of await page.getByRole("region", { name: "Packages", exact: true }).locator("md-checkbox").all()) {
                    expect((await checkbox.boundingBox())!.width).toBeGreaterThanOrEqual(18);
                }
                await selectSettingsSection(page, "appearance");
                await wheelIntoView(page, page.getByRole("combobox", { name: "Language", exact: true }), body);
                await wheelIntoView(page, page.getByRole("combobox", { name: "GUI animation", exact: true }), body);
            }
            if (width === 1280) await browserScreenshot(page, "browser-settings-1280.png");
        });

        await test.step("Extensions cards reflow rather than requiring a wide canvas", async () => {
            await openHarness(page, "/extensions");
            const cards = page.locator(".extension-card");
            await expect(cards).toHaveCount(2);
            const body = page.locator(".utility-workspace-body");
            await expectCanvasFits(page);
            await expectHeaderFits(page);
            await expectNoHorizontalOverflow(body);
            for (const card of await cards.all()) {
                await wheelIntoView(page, card.getByRole("button", { name: "Manage extension", exact: true }), body);
                await expectHorizontalBounds(card, body);
            }
            if (width === 800) await browserScreenshot(page, "browser-extensions-800.png");
        });
    });
}

test("H7 outer page scrolling avoids a nested scrollbar at the viewport center", async ({ page }) => {
    await page.setViewportSize({ width: 640, height: 800 });
    await openHarness(page, "/settings");
    await selectSettingsSection(page, "unity");
    const body = page.locator(".utility-workspace-body");
    await body.evaluate(element => {
        const target = document.createElement("button");
        target.dataset.testid = "scroll-helper-target";
        target.textContent = "Scroll helper target";
        target.style.marginBlockStart = "900px";
        element.append(target);
    });
    const installations = page.getByRole("region", { name: "Unity installations", exact: true }).locator(".desktop-list-viewport");
    const horizontal = installations.locator(":scope > .os-scrollbar-horizontal");
    await installations.locator("table").evaluate((element) => { element.style.minWidth = "1800px"; });
    await expect(horizontal).toHaveClass(/os-scrollbar-visible/);
    const outerBox = (await body.boundingBox())!, barBox = (await horizontal.boundingBox())!;
    // Keep the nested scrollbar under the old pointer point regardless of font metrics.
    await installations.evaluate((element, offset) => { element.style.transform = `translateY(${offset}px)`; },
        outerBox.y + outerBox.height / 2 - barBox.y - barBox.height / 2);
    const center = { x: outerBox.x + outerBox.width / 2, y: outerBox.y + outerBox.height / 2 };
    expect(await body.evaluate((element, point) => {
        const hit = document.elementFromPoint(point.x, point.y);
        const bar = hit?.closest(".os-scrollbar-horizontal");
        return bar !== undefined && bar !== null && bar.closest(".alcomd-scroll-area") !== element;
    }, center)).toBe(true);
    const target = page.getByTestId("scroll-helper-target");
    expect(await withinBounds(target, body)).toBe(false);
    await wheelIntoView(page, target, body);
    expect(await body.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    expect(await installations.evaluate((element) => [element.scrollLeft, element.scrollTop])).toEqual([0, 0]);
    expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
});

test("Resources segments remain keyboard reachable with visible contextual actions at 800px", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 800 });
    await openHarness(page, "/repositories");
    const navigation = page.getByRole("navigation", { name: "Resources", exact: true });
    const repositories = navigation.getByRole("button", { name: "Repositories", exact: true });
    const userPackages = navigation.getByRole("button", { name: "User Packages", exact: true });
    const templates = navigation.getByRole("button", { name: "Templates", exact: true });
    await repositories.focus();
    await expect(repositories).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(userPackages).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(templates).toBeFocused();
    await expectReachable(page, templates, navigation);
    await expectReachable(page, page.getByRole("button", { name: "Add repository", exact: true }));
    await page.keyboard.press("Enter");
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Resources", level: 1, exact: true })).toBeFocused();
    await expectReachable(page, page.getByRole("button", { name: "Create template", exact: true }));
    await expectReachable(page, page.getByRole("button", { name: "More template creation options", exact: true }));
    await expectReachable(page, page.getByRole("button", { name: "Refresh", exact: true }));
    await expectHeaderFits(page);
    await expectCanvasFits(page);
});

test("Theme keeps its settings rows and fields separate at 800px", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 800 });
    await openTheme(page);
    await browserScreenshot(page, "browser-theme-800-top.png");
    await expectThemeFits(page);
    await browserScreenshot(page, "browser-theme-800.png");
});

test("H6 Japanese Theme keeps its setting rows and color field usable at 621px", async ({ page }) => {
    await page.setViewportSize({ width: 621, height: 800 });
    await openTheme(page, "ja-JP");
    await expectNoHorizontalOverflow(page.locator(".theme-editor"));
    await expectNoHorizontalOverflow(page.locator(".theme-source-row"));
    const rows = page.locator(".theme-editor > .settings-row");
    await expect(rows).toHaveCount(3);
    const color = (await rows.nth(1).boundingBox())!;
    const scheme = (await rows.nth(2).boundingBox())!;
    expect(scheme.y).toBeGreaterThanOrEqual(color.y + color.height);
    expect((await page.locator("#theme-source").boundingBox())!.width).toBeGreaterThan(100);
    await page.getByRole("combobox", { name: "カラーモード", exact: true }).focus();
    await page.keyboard.press("Tab");
    await expect(page.locator("#theme-picker")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(page.locator("#theme-source").getByRole("textbox")).toBeFocused();
    await page.keyboard.press("Tab");
    const schemeField = page.getByRole("combobox", { name: "配色スキーム", exact: true });
    await expect(schemeField).toBeFocused();
    await expectReachable(page, schemeField, page.locator(".utility-workspace-body"));
});

test.describe("H6 200 percent equivalent CSS layout", () => {
    // 720 CSS px at deviceScaleFactor 2 models a 1440 physical-pixel window at 200%.
    // This is reflow evidence, not automation of native browser zoom or screen-reader certification.
    test.use({ viewport: { width: 720, height: 450 }, deviceScaleFactor: 2 });

    test("primary pages retain reachable tools and local scrolling in the shorter canvas", async ({ page }) => {
        for (const route of ["/projects", "/repositories", "/extensions", "/activity", "/diagnostics", "/settings"]) {
            await openHarness(page, route);
            await expectCanvasFits(page);
            await expectHeaderFits(page);
            if (route === "/projects") {
                await expectReachable(page, page.getByRole("button", { name: "Create project", exact: true }));
            } else if (route === "/repositories") {
                await expectReachable(page, page.getByRole("button", { name: "Add repository", exact: true }));
            } else {
                const body = page.locator(".utility-workspace-body");
                await expectNoHorizontalOverflow(body);
                if (route === "/settings") {
                    await selectSettingsSection(page, "appearance");
                    await wheelIntoView(page, page.getByRole("combobox", { name: "GUI animation", exact: true }), body);
                } else if (route === "/extensions") {
                    await wheelIntoView(page, page.getByRole("button", { name: "Manage extension", exact: true }).last(), body);
                } else {
                    await expectReachable(page, page.getByRole("searchbox", { name: route === "/activity" ? "Search activity" : "Search technical logs", exact: true }));
                }
            }
        }
    });

    test("Theme remains usable at 720x450 CSS pixels without a root font-size substitute", async ({ page }) => {
        await openTheme(page);
        await browserScreenshot(page, "browser-theme-200-equivalent-top.png");
        expect(await page.evaluate(() => ({ width: innerWidth, height: innerHeight, scale: devicePixelRatio })))
            .toEqual({ width: 720, height: 450, scale: 2 });
        expect(await page.locator("html").evaluate((element) => element.style.fontSize)).toBe("");
        await expectThemeFits(page);
        await browserScreenshot(page, "browser-theme-200-equivalent.png");
    });
});

for (const viewport of [{ width: 1440, height: 900 }, { width: 640, height: 400 }]) {
    test(`H6 Portable UI really wheels to its last action at ${viewport.width}x${viewport.height}`, async ({ page }) => {
        await page.setViewportSize(viewport);
        await openHarness(page, "/extensions/com.cqmhv.discord/ui");
        await expect(page.locator(".portable-document")).toBeVisible();
        const body = page.locator(".extension-surface .alcomd-scroll-area").filter({ has: page.locator(".portable-document") });
        await expect(body).toHaveCount(1);
        await expectCanvasFits(page);
        expect(await body.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeGreaterThan(40);
        const save = page.locator(".portable-document").getByRole("button", { name: "Save", exact: true });
        const before = (await save.boundingBox())!, bodyBox = (await body.boundingBox())!;
        expect(before.y + before.height).toBeGreaterThan(bodyBox.y + bodyBox.height);
        await wheelIntoView(page, save, body);
        expect(await body.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
        expect(await page.evaluate(() => document.scrollingElement?.scrollTop)).toBe(0);
        await expectNoHorizontalOverflow(body);
        await browserScreenshot(page, `browser-portable-wheel-${viewport.width}x${viewport.height}.png`);
    });
}

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    await page.evaluate(async () => { await document.fonts.ready; });
    await expect(page.getByRole("heading", { level: 1 })).toBeFocused();
}

function navigationItem(page: Page, label: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${label}$`) })
    });
}

async function expectCanvasFits(page: Page) {
    const result = await page.locator("#main-content").evaluate((element) => {
        const box = element.getBoundingClientRect();
        return { width: box.width, left: box.left, right: box.right, top: box.top, bottom: box.bottom,
            viewportWidth: innerWidth, viewportHeight: innerHeight,
            documentWidth: document.documentElement.scrollWidth, documentHeight: document.documentElement.scrollHeight };
    });
    expect(result.width).toBeGreaterThan(100);
    expect(result.left).toBeGreaterThanOrEqual(0);
    expect(result.right).toBeLessThanOrEqual(result.viewportWidth + 1);
    expect(result.top).toBeGreaterThanOrEqual(0);
    expect(result.bottom).toBeLessThanOrEqual(result.viewportHeight + 1);
    expect(result.documentWidth).toBeLessThanOrEqual(result.viewportWidth + 1);
    expect(result.documentHeight).toBeLessThanOrEqual(result.viewportHeight + 1);
}

async function expectHeaderFits(page: Page) {
    const header = page.locator(".page-header");
    await expectNoHorizontalOverflow(header);
    for (const child of await header.locator(":scope > *").all()) await expectHorizontalBounds(child, header);
}

async function expectNoHorizontalOverflow(target: Locator) {
    await expect.poll(() => target.evaluate((element) => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
}

async function expectHorizontalBounds(target: Locator, container: Locator) {
    const box = (await target.boundingBox())!;
    const bounds = (await container.boundingBox())!;
    expect(box.width).toBeGreaterThan(0);
    expect(box.x).toBeGreaterThanOrEqual(bounds.x - 1);
    expect(box.x + box.width).toBeLessThanOrEqual(bounds.x + bounds.width + 1);
}

async function withinBounds(target: Locator, container: Locator) {
    const box = await target.boundingBox(), bounds = await container.boundingBox();
    return box !== null && bounds !== null && box.width > 0 && box.height > 0
        && box.x >= bounds.x - 1 && box.x + box.width <= bounds.x + bounds.width + 1
        && box.y >= bounds.y - 1 && box.y + box.height <= bounds.y + bounds.height + 1;
}

async function expectReachable(page: Page, target: Locator, container: Locator = page.locator("#main-content")) {
    await expect(target).toBeVisible();
    // Check geometry before Playwright's trial click; auto-scroll must not conceal a clipped canvas.
    await expect.poll(() => withinBounds(target, container)).toBe(true);
    const box = (await target.boundingBox())!;
    const viewport = page.viewportSize()!;
    expect(box.x).toBeGreaterThanOrEqual(0);
    expect(box.y).toBeGreaterThanOrEqual(0);
    expect(box.x + box.width).toBeLessThanOrEqual(viewport.width + 1);
    expect(box.y + box.height).toBeLessThanOrEqual(viewport.height + 1);
    await target.click({ trial: true });
}

async function wheelIntoView(page: Page, target: Locator, scroller: Locator) {
    await expect(target).toBeAttached();
    await expectHorizontalBounds(target, scroller);
    for (let attempt = 0; attempt < 6 && !await withinBounds(target, scroller); attempt++) {
        const box = (await scroller.boundingBox())!, control = (await target.boundingBox())!;
        // The center can hit a nested list's faded scrollbar, which consumes wheel events.
        const point = await scroller.evaluate((element) => {
            const bounds = element.getBoundingClientRect();
            const candidates = [8, bounds.width / 2, bounds.width - 8].map((offset) => ({
                x: bounds.x + offset, y: bounds.y + bounds.height / 2
            }));
            return candidates.find((candidate) => {
                const hit = document.elementFromPoint(candidate.x, candidate.y);
                return hit?.closest(".alcomd-scroll-area") === element && hit.closest(".os-scrollbar") === null;
            }) ?? null;
        });
        expect(point, "Wheel must target the intended scroll viewport's content").not.toBeNull();
        const previous = await scroller.evaluate((element) => element.scrollTop);
        const delta = control.y < box.y ? control.y - box.y - 24 : Math.max(120, control.y + control.height - box.y - box.height + 24);
        await page.mouse.move(point!.x, point!.y);
        await page.mouse.wheel(0, delta);
        await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).not.toBe(previous);
    }
    await expectReachable(page, target, scroller);
}

async function wheelHorizontallyIntoView(page: Page, target: Locator, scroller: Locator) {
    const box = (await scroller.boundingBox())!, control = (await target.boundingBox())!;
    const previous = await scroller.evaluate((element) => element.scrollLeft);
    const delta = control.x < box.x ? control.x - box.x - 16 : control.x + control.width - box.x - box.width + 16;
    await page.mouse.move(box.x + box.width / 2, box.y + Math.min(80, box.height / 2));
    await page.mouse.wheel(delta, 0);
    await expect.poll(() => scroller.evaluate((element) => element.scrollLeft)).not.toBe(previous);
    await expectHorizontalBounds(target, scroller);
}

async function openTheme(page: Page, locale: "en-US" | "ja-JP" = "en-US") {
    await openHarness(page, "/settings");
    await selectSettingsSection(page, "appearance");
    if (locale === "ja-JP") {
        await page.locator("#settings-locale").evaluate((element) => {
            (element as HTMLElement & { select(value: string): void }).select("ja-JP");
            element.dispatchEvent(new Event("change", { bubbles: true }));
        });
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
    }
    await expect(page.locator(".theme-editor")).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await wheelIntoView(page, page.locator("#theme-source").getByRole("textbox"), page.locator(".utility-workspace-body"));
}

async function expectThemeFits(page: Page) {
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    const body = page.locator(".utility-workspace-body");
    await expectNoHorizontalOverflow(theme);
    const rows = theme.locator(":scope > .settings-row");
    await expect(rows).toHaveCount(3);
    for (let index = 0; index < 3; index++) {
        await expectHorizontalBounds(rows.nth(index), theme);
        if (index > 0) {
            const previous = (await rows.nth(index - 1).boundingBox())!;
            expect((await rows.nth(index).boundingBox())!.y).toBeGreaterThanOrEqual(previous.y + previous.height);
        }
    }
    await expectHorizontalBounds(theme.getByRole("combobox", { name: "Color scheme", exact: true }), theme);
    await expectHorizontalBounds(page.getByRole("textbox", { name: "Source color", exact: true }), theme);
    const modes = page.locator("#theme-mode");
    await expectHorizontalBounds(modes, theme);
    await theme.getByRole("combobox", { name: "Color mode", exact: true }).focus();
    await page.keyboard.press("Tab");
    await expect(page.locator("#theme-picker")).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(theme.getByRole("textbox", { name: "Source color", exact: true })).toBeFocused();
    await page.keyboard.press("Tab");
    const scheme = theme.getByRole("combobox", { name: "Color scheme", exact: true });
    await expect(scheme).toBeFocused();
    await expectReachable(page, scheme, body);
    await expectCanvasFits(page);
    await expectNoHorizontalOverflow(body);
    const restore = theme.getByRole("button", { name: "Restore defaults", exact: true });
    await wheelIntoView(page, restore, body);
}

async function browserScreenshot(page: Page, name: string) {
    await page.screenshot({ path: test.info().outputPath(name), fullPage: false, animations: "disabled" });
}

async function expectPrimaryEdgeOpensDialog(page: Page, title: string) {
    const primary = page.locator(".alcomd-split-button-primary");
    const box = (await primary.boundingBox())!;
    // The dropdown's native hit area must not steal clicks on the primary's visible edge.
    await primary.click({ position: { x: box.width - 4, y: box.height / 2 } });
    const dialog = page.getByRole("dialog", { name: title, exact: true });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("menu")).toHaveCount(0);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
}
