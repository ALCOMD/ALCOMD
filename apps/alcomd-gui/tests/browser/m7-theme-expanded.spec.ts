import { selectSettingsSection } from "./settings-sections";
import { expect, test } from "@playwright/test";

for (const mode of ["light", "dark"] as const) {
    test(`inline ${mode} theme editor uses aligned settings rows without sliders or empty columns`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1400, height: 800 });
        await page.goto(`/browser-harness.html?route=/settings&theme-mode=${mode}`);
        await selectSettingsSection(page, "appearance");
        const host = page.getByRole("form", { name: "Theme", exact: true });
        await expect(host).toBeVisible();
        await expect(page.getByRole("button", { name: "Theme", exact: true })).toHaveCount(0);
        await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toHaveCount(0);
        await expect(host.getByRole("combobox", { name: "Color mode", exact: true })).toBeVisible();
        await expect(host.getByRole("combobox", { name: "Color scheme", exact: true })).toBeVisible();
        await expect(host.locator("md-outlined-text-field#theme-source")).toHaveCount(1);
        await expect(host.locator("md-slider, md-radio, md-outlined-segmented-button-set, .theme-editor-columns, .theme-color-column, .theme-options-column, .theme-hct-controls")).toHaveCount(0);
        await expect(host.locator("#theme-mode md-select-option")).toHaveCount(3);
        await expect(host.locator("#theme-scheme md-select-option")).toHaveCount(9);
        const rows = host.locator(".settings-row");
        await expect(rows).toHaveCount(3);
        await expect(rows.nth(0)).toContainText("Color mode");
        await expect(rows.nth(1)).toContainText("Source color");
        await expect(rows.nth(2)).toContainText("Color scheme");
        await page.screenshot({ animations: "disabled" });
        const rowBounds = await rows.evaluateAll(elements => elements.map(element => {
            const { x, y, width, height } = element.getBoundingClientRect();
            return { x, y, width, height };
        }));
        for (let index = 1; index < rowBounds.length; index++) {
            const previous = rowBounds[index - 1]!;
            const current = rowBounds[index]!;
            expect(current.y).toBeGreaterThanOrEqual(previous.y + previous.height - 1);
            expect(current.y - previous.y - previous.height).toBeLessThanOrEqual(24);
        }
        const fields = await host.locator("#theme-mode, .theme-source-row, #theme-scheme").evaluateAll(elements => elements.map(element => {
            const { x, width } = element.getBoundingClientRect();
            return { x, width };
        }));
        for (const field of fields) {
            expect(Math.abs(field.x + field.width - fields[0]!.x - fields[0]!.width)).toBeLessThanOrEqual(1);
            expect(Math.abs(field.width - fields[0]!.width)).toBeLessThanOrEqual(1);
        }
        const source = (await host.locator(".theme-source-row").boundingBox())!;
        const scheme = (await host.locator("#theme-scheme").boundingBox())!;
        expect(Math.abs(source.width - scheme.width)).toBeLessThanOrEqual(1);
        const hostBounds = (await host.boundingBox())!;
        const footer = (await host.locator(".theme-editor-footer").boundingBox())!;
        const last = rowBounds.at(-1)!;
        expect(footer.y).toBeGreaterThanOrEqual(last.y + last.height - 1);
        expect(footer.y - last.y - last.height).toBeLessThanOrEqual(32);
        expect(hostBounds.height - footer.y - footer.height + hostBounds.y).toBeLessThanOrEqual(1);
        const save = host.getByRole("button", { name: "Save", exact: true });
        await save.scrollIntoViewIfNeeded();
        await expect(save).toBeInViewport();
        await page.screenshot({ path: testInfo.outputPath(`theme-${mode}-window.png`), animations: "disabled", fullPage: true });
        expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    });
}

test("mode and scheme dropdowns support keyboard selection and return focus without writing", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&theme-mode=light");
    await selectSettingsSection(page, "appearance");
    await expect(page.getByRole("form", { name: "Theme", exact: true })).toBeVisible();
    const mode = page.getByRole("combobox", { name: "Color mode", exact: true });
    await mode.focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator("#theme-mode")).toHaveJSProperty("open", true);
    await expect(page.getByRole("option", { name: "Light", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(page.getByRole("option", { name: "System", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Light", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Dark", exact: true })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("#theme-mode")).toHaveJSProperty("value", "dark");
    await expect(page.locator("#theme-mode")).toHaveJSProperty("open", false);
    await expect(mode).toBeFocused();
    expect(await page.locator("#theme-mode md-select-option").evaluateAll(elements => elements.filter(element => (element as HTMLElement & { selected: boolean }).selected).length)).toBe(1);
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    const scheme = page.getByRole("combobox", { name: "Color scheme", exact: true });
    await scheme.focus();
    await page.keyboard.press("ArrowDown");
    await expect(page.locator("#theme-scheme")).toHaveJSProperty("open", true);
    await expect(page.getByRole("option", { name: "Vibrant", exact: true })).toBeFocused();
    await page.keyboard.press("ArrowDown");
    await expect(page.getByRole("option", { name: "Expressive", exact: true })).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.locator("#theme-scheme")).toHaveJSProperty("value", "expressive");
    await expect(page.locator("#theme-scheme")).toHaveJSProperty("open", false);
    await expect(scheme).toBeFocused();
    expect(await page.locator("#theme-scheme md-select-option").evaluateAll((elements) => elements.filter((element) => (element as HTMLElement & { selected: boolean }).selected).length)).toBe(1);
    await expect(page.locator("html")).toHaveAttribute("data-theme-scheme", "expressive");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

for (const { locale, label, options } of [
    { locale: "en-US", label: "Color mode", options: ["System", "Light", "Dark"] },
    { locale: "zh-CN", label: "颜色模式", options: ["跟随系统", "浅色", "深色"] },
    { locale: "ja-JP", label: "カラーモード", options: ["システム設定", "ライト", "ダーク"] }
]) {
    test(`theme mode dropdown exposes three translated choices and previews without writing in ${locale}`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1400, height: 800 });
        await page.goto("/browser-harness.html?route=/settings&theme-mode=light");
        await selectSettingsSection(page, "appearance");
        const language = page.locator("#settings-locale");
        if (await language.evaluate((element) => (element as HTMLElement & { value: string }).value) !== locale) {
            await language.evaluate((element, value) => {
                (element as HTMLElement & { select(value: string): void }).select(value);
                element.dispatchEvent(new Event("change", { bubbles: true }));
            }, locale);
        }
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        const control = page.locator("#theme-mode");
        const dropdown = page.getByRole("combobox", { name: label, exact: true });
        await expect(dropdown).toBeVisible();
        await expect(control.locator("md-select-option")).toHaveText(options);
        await expect(page.locator(".theme-editor .alcomd-segmented-control, .alcomd-segmented-sizer")).toHaveCount(0);
        const writes = await page.evaluate(() => window.settingsRequests.length);
        for (const [index, value] of ["system", "light", "dark"].entries()) {
            const lifecycle = await control.evaluateHandle(element => ({
                opened: new Promise<void>(resolve => element.addEventListener("opened", () => resolve(), { once: true })),
                closed: new Promise<void>(resolve => element.addEventListener("closed", () => resolve(), { once: true }))
            }));
            await dropdown.click();
            await lifecycle.evaluate(events => events.opened);
            await expect(control).toHaveJSProperty("open", true);
            await page.getByRole("option", { name: options[index]!, exact: true }).click();
            await lifecycle.evaluate(events => events.closed);
            await lifecycle.dispose();
            await expect(control).toHaveJSProperty("value", value);
            await expect(control).toHaveJSProperty("displayText", options[index]!);
            await expect(control).toHaveJSProperty("open", false);
            expect(await control.locator("md-select-option").evaluateAll(elements => elements.filter(element => (element as HTMLElement & { selected: boolean }).selected).length)).toBe(1);
            expect(await page.evaluate(() => window.settingsRequests.length)).toBe(writes);
            if (locale === "zh-CN" && value === "system") {
                await page.screenshot({ path: testInfo.outputPath("chinese-system-selected.png"), animations: "disabled", fullPage: true });
            }
        }
    });
}
