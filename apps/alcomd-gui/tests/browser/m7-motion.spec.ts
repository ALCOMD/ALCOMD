import { expect, test, type Page } from "@playwright/test";

for (const preference of ["system", "full", "reduced"] as const) {
    for (const system of ["reduce", "no-preference"] as const) {
        test(`${preference} animation preference with system ${system} reaches Material and scrollbars`, async ({ page }) => {
            await page.emulateMedia({ reducedMotion: system });
            await openSettings(page, `&motion=${preference}`);
            const reduced = preference === "reduced" || (preference === "system" && system === "reduce");
            await expect(page.locator("html")).toHaveAttribute("data-motion", preference);
            await expect(page.locator("html")).toHaveAttribute("data-reduced-motion", String(reduced));
            await expect(page.locator("#settings-motion")).toHaveJSProperty("value", preference);
            await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", reduced);
            // Use real existing elements in light and shadow DOM, without injected test CSS.
            const scrollbar = page.locator(".main-content .os-scrollbar").first();
            // Scrollbars initialize after a rendering opportunity, independently of the controls above.
            await expect(scrollbar).toHaveCSS("transition-property", "opacity, visibility, top, right, bottom, left");
            const duration = expect.poll(async () => maxDuration(await scrollbar.evaluate((element) => getComputedStyle(element).transitionDuration)));
            if (reduced) await duration.toBeLessThan(0.001);
            else await duration.toBeGreaterThan(0.01);
            const checkbox = page.locator("md-checkbox").first();
            expect(await checkbox.evaluate((element) => {
                const sheet = element.shadowRoot!.querySelector<HTMLStyleElement>("style[data-alcomd-motion]")!;
                return sheet.media;
            })).toBe(reduced ? "all" : "not all");
            await page.getByRole("button", { name: "Theme", exact: true }).click();
            await expect(page.locator("md-dialog[open]")).toHaveJSProperty("quick", reduced);
            await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeVisible();
            const menu = page.locator("#settings-motion").locator("md-menu");
            await expect(menu).toHaveJSProperty("quick", reduced);
            await page.keyboard.press("Escape");
            await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeHidden();
            await expect(page.getByRole("button", { name: "Theme", exact: true })).toBeFocused();
            expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
        });
    }
}

test("system follows live OS changes while explicit preferences stay fixed", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await openSettings(page);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", true);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", false);
    for (const [value, quick] of [["full", false], ["reduced", true]] as const) {
        await selectOption(page, value);
        await expect(page.locator("html")).toHaveAttribute("data-motion", value);
        for (const system of ["reduce", "no-preference"] as const) {
            await page.emulateMedia({ reducedMotion: system });
            await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", quick);
        }
    }
    await selectOption(page, "system");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update.appearance?.motion))).toEqual(["full", "reduced", "system"]);
});

test("a failed animation write retains the latest control intent without claiming an accepted preference", async ({ page }) => {
    await openSettings(page, "&theme-save=error");
    await selectOption(page, "reduced");
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "reduced");
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
});

test("Chinese animation choices use the requested labels", async ({ page }) => {
    await openSettings(page);
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "system");
    await expect(page.locator("#settings-motion md-select-option")).toHaveText(["System", "Enable animations", "Disable animations"]);
    await page.locator("#settings-locale").evaluate((element) => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.getByRole("combobox", { name: "界面动画", exact: true })).toBeVisible();
    await page.getByRole("combobox", { name: "界面动画", exact: true }).click();
    await expect(page.locator("#settings-motion md-select-option")).toHaveText(["跟随系统", "启用动画", "关闭动画"]);
    await page.keyboard.press("Escape");
});

async function openSettings(page: Page, suffix = "") {
    await page.goto(`/browser-harness.html?route=/settings${suffix}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await expect(page.locator("#settings-motion")).toBeVisible();
}

async function selectOption(page: Page, value: string) {
    const select = page.locator("#settings-motion");
    const lifecycle = await select.evaluateHandle((element) => ({
        opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
        closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
    }));
    try {
        await select.getByRole("combobox", { name: "GUI animation", exact: true }).click();
        await lifecycle.evaluate((events) => events.opened);
        const label = { full: "Enable animations", reduced: "Disable animations", system: "System" }[value]!;
        await select.getByRole("option", { name: label, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
    await expect(select).toHaveJSProperty("value", value);
}

function maxDuration(value: string) {
    return Math.max(...value.split(",").map((duration) => duration.trim().endsWith("ms") ? parseFloat(duration) / 1000 : parseFloat(duration)));
}
