import { expect, test } from "@playwright/test";

for (const mode of ["light", "dark"] as const) {
    test(`expanded ${mode} theme editor keeps all choices visible in two columns`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1400, height: 800 });
        await page.goto(`/browser-harness.html?route=/settings&theme-mode=${mode}`);
        await expect(page.getByRole("button", { name: "Theme", exact: true })).toBeVisible();
        await page.screenshot({ path: testInfo.outputPath(`theme-entry-${mode}.png`), animations: "disabled" });
        await page.getByRole("button", { name: "Theme", exact: true }).click();
        const dialog = page.getByRole("dialog", { name: "Theme", exact: true });
        const host = page.locator("md-dialog[open]");
        await expect(dialog).toBeVisible();
        await expect(host.getByRole("combobox")).toHaveCount(0);
        await expect(host.locator("md-outlined-text-field#theme-source")).toHaveCount(1);
        await expect(host.locator(".theme-hct-spectrum, .alcomd-segmented-tonal, .alcomd-pill-field")).toHaveCount(0);
        // Material radios expose their role/state through ElementInternals, which
        // Playwright's DOM-based role/checked helpers do not read in this version.
        await expect(host.locator("md-radio")).toHaveCount(9);
        for (const radio of await host.locator("md-radio").all()) await expect(radio).toBeVisible();
        await page.screenshot({ animations: "disabled" });
        const left = (await page.locator(".theme-color-column").boundingBox())!;
        const right = (await page.locator(".theme-options-column").boundingBox())!;
        expect(Math.abs(left.y - right.y)).toBeLessThanOrEqual(1);
        expect(right.x - left.x - left.width).toBeGreaterThanOrEqual(11);
        expect(left.width).toBeGreaterThan(right.width);
        const sliders = await page.locator(".theme-hct-controls md-slider").evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().y));
        expect(sliders[1]! - sliders[0]!).toBeGreaterThan(60);
        expect(sliders[2]! - sliders[1]!).toBeGreaterThan(60);
        const panel = page.locator(".theme-color-column .theme-panel").first();
        await expect(panel).toHaveCSS("border-top-left-radius", "28px");
        const panelColor = await panel.evaluate((element) => getComputedStyle(element).backgroundColor);
        const dialogColor = await page.locator("md-dialog[open]").evaluate((element) => getComputedStyle(element.shadowRoot!.querySelector("dialog")!).backgroundColor);
        expect(panelColor).not.toBe(dialogColor);
        const footer = (await page.locator(".theme-editor-footer").boundingBox())!;
        const dialogBounds = (await dialog.boundingBox())!;
        const leftInset = left.x - dialogBounds.x;
        const rightInset = dialogBounds.x + dialogBounds.width - right.x - right.width;
        expect(Math.abs(leftInset - rightInset)).toBeLessThanOrEqual(1);
        expect(footer.y + footer.height).toBeLessThanOrEqual(dialogBounds.y + dialogBounds.height - 16);
        expect(footer.y + footer.height).toBeLessThanOrEqual(800);
        // Pin geometry after finite Material opening animations finish.
        await page.screenshot({ path: `../../target/m7-theme-md3-20261030/theme-${mode}-window.png`, animations: "disabled" });
        await dialog.screenshot({ path: `../../target/m7-theme-md3-20261030/theme-${mode}-dialog.png`, animations: "disabled" });
        expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    });
}

test("expanded scheme choices support keyboard selection and mode remains single-select", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&theme-mode=light");
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    const vibrant = page.locator("#theme-scheme-vibrant");
    await vibrant.focus();
    await page.keyboard.press("ArrowRight");
    await expect(page.locator("#theme-scheme-expressive")).toHaveJSProperty("checked", true);
    expect(await page.locator("#theme-scheme md-radio").evaluateAll((elements) => elements.filter((element) => (element as HTMLElement & { checked: boolean }).checked).length)).toBe(1);
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
        const language = page.locator("#settings-locale");
        if (await language.evaluate((element) => (element as HTMLElement & { value: string }).value) !== locale) {
            await language.evaluate((element, value) => {
                (element as HTMLElement & { select(value: string): void }).select(value);
                element.dispatchEvent(new Event("change", { bubbles: true }));
            }, locale);
        }
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        await page.locator("#settings-theme-button").click();
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
        const section = (await page.locator(".theme-options-column").boundingBox())!;
        expect(before.x + before.width).toBeLessThanOrEqual(section.x + section.width + 1);
        if (locale === "zh-CN") {
            await group.locator("md-outlined-segmented-button").nth(1).click();
            await page.screenshot({ path: "../../target/m7-segment-sizing-20261030/chinese-system-selected.png", animations: "disabled" });
        }
    });
}
