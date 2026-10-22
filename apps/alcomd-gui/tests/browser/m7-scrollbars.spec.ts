import { expect, test, type Locator } from "@playwright/test";

test.use({
    viewport: { width: 1440, height: 900 },
    // Headless Chromium otherwise hides the actual hit area we must drag.
    launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] }
});

async function expectScrollbarTheme(scroller: Locator) {
    await expect(scroller).toHaveAttribute("data-overlayscrollbars-viewport", /scrollbarHidden/);
    await expect(scroller.locator(":scope > .os-scrollbar")).toHaveCount(2);
    await expect(scroller.locator(":scope > .os-scrollbar .os-scrollbar-handle")).toHaveCount(2);
    const vertical = scroller.locator(":scope > .os-scrollbar-vertical");
    await expect(vertical).toHaveCSS("width", "10px");
    await expect(vertical.locator(".os-scrollbar-handle")).toHaveCSS("border-radius", "999px");
    expect(await scroller.evaluate((element) => getComputedStyle(element).scrollbarWidth === "none"
        || getComputedStyle(element, "::-webkit-scrollbar").display === "none")).toBe(true);
    const colors = await vertical.evaluate((element) => {
        const probe = document.createElement("span");
        probe.style.color = getComputedStyle(element).getPropertyValue("--md-sys-color-outline");
        document.body.append(probe);
        const colors = { actual: getComputedStyle(element.querySelector(".os-scrollbar-handle")!).backgroundColor, expected: getComputedStyle(probe).color };
        probe.remove();
        return colors;
    });
    expect(colors.actual).toBe(colors.expected);
}

test("all shared lists and page scrollers use the application scrollbar theme", async ({ page }) => {
    const routes = [
        ["/projects", "ready"],
        ["/projects/00000000-0000-4000-8000-000000000101/packages", "package-multiple"],
        ["/repositories", "ready"],
        ["/repositories/00000000-0000-4000-8000-000000000102", "ready"],
        ["/user-packages", "package-user-source"],
        ["/templates", "ready"],
        ["/unity", "ready"],
        ["/settings", "ready"],
        ["/projects/00000000-0000-4000-8000-000000000101/backups", "ready"],
        ["/operations", "ready"],
        ["/activity", "ready"],
        ["/diagnostics", "ready"]
    ];
    for (const [route, state] of routes) {
        await page.goto(`/browser-harness.html?route=${encodeURIComponent(route!)}&state=${state}`);
        const list = page.locator(".desktop-list-viewport").first();
        await expect(list.locator("tbody tr").first()).toBeVisible();
        await page.mouse.move(1439, 899);
        await expectScrollbarTheme(list);
        await expectScrollbarTheme(page.locator(".primary-navigation"));
    }
    await page.locator("html").evaluate((element) => { element.dataset.appearance = "dark"; });
    await expectScrollbarTheme(page.locator(".desktop-list-viewport").first());
});

test("painted scrollbars retain wheel, horizontal scrolling and thumb dragging", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready");
    const viewport = page.locator(".desktop-list-viewport");
    await expect(viewport.locator("tbody tr").first()).toBeVisible();
    await viewport.evaluate((element) => {
        const body = element.querySelector("tbody")!;
        const row = body.firstElementChild!;
        for (let index = 0; index < 60; index++) body.append(row.cloneNode(true));
        element.querySelector("table")!.style.minWidth = "1800px";
    });
    const vertical = viewport.locator(":scope > .os-scrollbar-vertical");
    const horizontal = viewport.locator(":scope > .os-scrollbar-horizontal");
    await page.mouse.move(1439, 899);
    await expect(vertical).toHaveCSS("opacity", "0");
    const box = (await viewport.boundingBox())!;
    // Sample during movement: an eventual hidden assertion would miss a transient reveal.
    const movementOpacity: string[][] = [];
    for (let index = 0; index < 6; index++) {
        await page.mouse.move(box.x + box.width / 2 + index * 10, box.y + 100 + index * 5, { steps: 4 });
        movementOpacity.push(await viewport.evaluate(async (element) => {
            await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
            return Array.from(element.querySelectorAll(":scope > .os-scrollbar"), (bar) => getComputedStyle(bar).opacity);
        }));
    }
    expect(movementOpacity).toEqual(Array.from({ length: 6 }, () => ["0", "0"]));
    // Both hidden tracks must be discoverable without scrolling or clicking.
    for (const bar of [vertical, horizontal]) {
        const track = (await bar.locator(".os-scrollbar-track").boundingBox())!;
        await page.mouse.move(track.x + track.width / 2, track.y + track.height / 2);
        await expect(bar).toHaveCSS("opacity", "1");
        expect(await viewport.evaluate((element) => [element.scrollLeft, element.scrollTop])).toEqual([0, 0]);
        await page.mouse.move(box.x + box.width / 2, box.y + 100);
        await expect(bar).toHaveCSS("opacity", "0");
    }
    await page.mouse.move(1439, 899);
    await expect(vertical).toHaveCSS("opacity", "0");
    await expect(horizontal).toHaveCSS("opacity", "0");
    // A scroll also reveals the controls when the pointer is outside the viewport.
    await viewport.evaluate((element) => { element.scrollTop = 50; });
    await expect(vertical).toHaveCSS("opacity", "1");
    await expect(vertical).toHaveCSS("opacity", "0");
    expect(await viewport.evaluate((element) => {
        if (!(element instanceof HTMLElement)) throw new Error("Expected an HTML scroll viewport");
        return { x: element.offsetHeight - element.clientHeight, y: element.offsetWidth - element.clientWidth };
    })).toEqual({ x: 0, y: 0 });
    await page.mouse.move(box.x + box.width / 2, box.y + 100);
    await page.mouse.wheel(0, 400);
    await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(200);
    await page.mouse.wheel(350, 0);
    await expect.poll(() => viewport.evaluate((element) => element.scrollLeft)).toBeGreaterThan(100);
    await viewport.evaluate((element) => { element.scrollTop = 0; element.scrollLeft = 0; });
    const handle = vertical.locator(".os-scrollbar-handle");
    await expect(handle).toHaveCSS("pointer-events", "auto");
    await handle.hover();
    // The idle timer may expire, but hovering the scrollbar itself keeps it visible.
    await expect(vertical).toHaveClass(/os-scrollbar-auto-hide-hidden/);
    await expect(vertical).toHaveCSS("opacity", "1");
    await page.mouse.move(box.x + box.width / 2, box.y + 100);
    await expect(vertical).toHaveCSS("opacity", "0");
    await viewport.evaluate((element) => { element.scrollTop = 1; });
    await expect(vertical).toHaveCSS("opacity", "1");
    await handle.hover();
    const thumbBox = (await handle.boundingBox())!;
    await page.mouse.move(thumbBox.x + thumbBox.width / 2, thumbBox.y + thumbBox.height / 2);
    await page.mouse.down();
    await page.mouse.move(thumbBox.x + thumbBox.width / 2, thumbBox.y + 180, { steps: 12 });
    await page.mouse.move(box.x + box.width / 2, thumbBox.y + 180);
    await expect(vertical).toHaveClass(/os-scrollbar-auto-hide-hidden/);
    await expect(vertical).toHaveCSS("opacity", "1");
    await page.mouse.up();
    await expect.poll(() => viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(300);
    await page.mouse.move(1439, 899);
    await page.screenshot({ path: "../../target/m7-v3-recheck-20260919/browser/custom-scrollbars.png" });
    await viewport.locator("tbody").evaluate((element) => {
        while (element.children.length > 1) element.lastElementChild!.remove();
    });
    await expect(vertical).toHaveCSS("opacity", "0");
    const detached = await viewport.elementHandle();
    await page.locator("#primary-navigation .navigation-item").filter({ hasText: "Resources" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
    expect(await detached!.evaluate((element) => ({
        connected: element.isConnected,
        bars: element.querySelectorAll(".os-scrollbar").length,
        initialized: element.hasAttribute("data-overlayscrollbars-viewport")
    }))).toEqual({ connected: false, bars: 0, initialized: false });
});

test("Material dialog and select use real handles while native text editing stays intact", async ({ page }) => {
    await page.goto("/browser-harness.html?material=1");
    await page.getByRole("button", { name: "Open dialog", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    const content = host.locator(".material-dialog-content");
    await content.evaluate((element) => {
        for (let index = 0; index < 40; index++) {
            const paragraph = document.createElement("p");
            paragraph.textContent = `Scrollable dialog evidence ${index}`;
            element.append(paragraph);
        }
    });
    const scroller = host.locator(".scroller");
    await expect.poll(() => scroller.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
    await page.mouse.move(1439, 899);
    await expectScrollbarTheme(scroller);
    await scroller.hover();
    await page.mouse.wheel(0, 320);
    await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await page.keyboard.press("Escape");
    await expect(host).toHaveCount(0);
    const select = page.locator("md-outlined-select");
    await select.evaluate((element) => {
        const option = element.querySelector("md-select-option")!;
        for (let index = 0; index < 24; index++) {
            const clone = option.cloneNode(true) as HTMLElement;
            clone.setAttribute("value", `scroll-option-${index}`);
            clone.removeAttribute("selected");
            element.append(clone);
        }
    });
    await page.getByRole("combobox", { name: "Project type" }).click();
    const menu = select.locator("md-menu .items");
    await expect(menu).toBeVisible();
    await page.mouse.move(1439, 899);
    await expectScrollbarTheme(menu);
    await page.keyboard.press("Escape");
    await page.locator("md-outlined-text-field").evaluate((element) => {
        (element as HTMLElement & { type: string; value: string }).type = "textarea";
        (element as HTMLElement & { value: string }).value = "Multiline content\n".repeat(30);
    });
    const textarea = page.locator("md-outlined-text-field textarea");
    await expect(textarea).toBeVisible();
    await textarea.focus();
    await page.keyboard.press("Control+End");
    await expect.poll(() => textarea.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    await expect(page.locator("md-outlined-text-field style[data-alcomd-scrollbars]")).toHaveCount(0);
    await expect(page.locator("md-dialog style[data-alcomd-scrollbars]")).toHaveCount(1);
    await expect(select.locator("md-menu style[data-alcomd-scrollbars]")).toHaveCount(1);
    await page.emulateMedia({ forcedColors: "active" });
    await page.getByRole("combobox", { name: "Project type" }).click();
    await expect(menu.locator(":scope > .os-scrollbar-vertical .os-scrollbar-handle")).toHaveCSS("background-color", "rgb(0, 0, 0)");
});
