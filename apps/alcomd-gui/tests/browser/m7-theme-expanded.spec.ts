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
        await expect(host.getByRole("combobox", { name: "Color scheme", exact: true })).toBeVisible();
        await expect(host.locator("md-outlined-text-field#theme-source")).toHaveCount(1);
        await expect(host.locator("md-slider, md-radio, .theme-editor-columns, .theme-color-column, .theme-options-column, .theme-hct-controls")).toHaveCount(0);
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
        for (const field of fields) expect(Math.abs(field.x + field.width - fields[0]!.x - fields[0]!.width)).toBeLessThanOrEqual(1);
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

test("scheme dropdown supports keyboard selection and mode remains single-select", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&theme-mode=light");
    await selectSettingsSection(page, "appearance");
    await expect(page.getByRole("form", { name: "Theme", exact: true })).toBeVisible();
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
    expect(await page.locator("#theme-scheme md-select-option").evaluateAll((elements) => elements.filter((element) => (element as HTMLElement & { selected: boolean }).selected).length)).toBe(1);
    await expect(page.locator("html")).toHaveAttribute("data-theme-scheme", "expressive");
    const modes = page.locator("#theme-mode");
    expect(await modes.locator("md-outlined-segmented-button").evaluateAll((elements) => elements.map((element) => {
        const button = element as HTMLElement & { label: string; noCheckmark: boolean };
        return { label: button.label, noCheckmark: button.noCheckmark };
    }))).toEqual([
        { label: "Light", noCheckmark: false },
        { label: "System", noCheckmark: false },
        { label: "Dark", noCheckmark: false }
    ]);
    await expect(modes.locator("md-icon")).toHaveCount(0);
    await modes.getByRole("button", { name: "Dark", exact: true }).click();
    await expect(modes.getByRole("button", { name: "Dark", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(modes.getByRole("button", { pressed: true })).toHaveCount(1);
    await modes.getByRole("button", { name: "Dark", exact: true }).click();
    await expect(modes.getByRole("button", { name: "Dark", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(modes.getByRole("button", { pressed: true })).toHaveCount(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

for (const locale of ["en-US", "zh-CN", "ja-JP"]) {
    test(`theme segments reserve the longest ${locale} label and checkmark before selection`, async ({ page }) => {
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
        const group = page.locator("#theme-mode");
        await expect(group.locator("md-outlined-segmented-button")).toHaveCount(3);
        const before = (await group.boundingBox())!;
        for (const segment of await group.locator("md-outlined-segmented-button").all()) {
            await segment.click();
            await page.screenshot({ animations: "disabled" });
            const after = (await group.boundingBox())!;
            expect(Math.abs(after.width - before.width)).toBeLessThanOrEqual(1);
            expect(Math.abs(after.height - before.height)).toBeLessThanOrEqual(1);
            for (const button of await group.locator("md-outlined-segmented-button").all()) {
                const geometry = await button.evaluate((element) => {
                    const label = element.shadowRoot!.querySelector<HTMLElement>(".md3-segmented-button__label-text")!;
                    const range = document.createRange();
                    range.selectNodeContents(label);
                    const labelRect = label.getBoundingClientRect();
                    const controlRect = element.getBoundingClientRect();
                    return { lines: range.getClientRects().length, labelLeft: labelRect.left, labelRight: labelRect.right, left: controlRect.left, right: controlRect.right };
                });
                expect(geometry.lines).toBe(1);
                expect(geometry.labelLeft).toBeGreaterThanOrEqual(geometry.left);
                expect(geometry.labelRight).toBeLessThanOrEqual(geometry.right);
            }
        }
        const section = (await group.locator("..").boundingBox())!;
        expect(before.x + before.width).toBeLessThanOrEqual(section.x + section.width + 1);
        if (locale === "zh-CN") {
            await group.locator("md-outlined-segmented-button").nth(1).click();
            await page.screenshot({ path: test.info().outputPath("chinese-system-selected.png"), animations: "disabled", fullPage: true });
        }
    });
}
