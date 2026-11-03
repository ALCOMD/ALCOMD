import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";

// Fixed values verified against the upstream Material Color Utilities Vibrant scheme.
// These tests deliberately do not import the production palette generator.
const defaultRoles = {
    light: { primary: "#0062a0", "on-primary": "#ffffff", secondary: "#515e7d", tertiary: "#565b8b", surface: "#f8f9ff", "on-surface": "#151c24" },
    dark: { primary: "#9bcaff", "on-primary": "#003256", secondary: "#b8c6ea", tertiary: "#bfc2fa", surface: "#0d141c", "on-surface": "#dde3ee" }
};
const customRoles = {
    light: { primary: "#b5007c", secondary: "#7f525d", tertiary: "#8b4c4f", surface: "#fff8f8" },
    dark: { primary: "#ffafd5", secondary: "#f1b7c4", tertiary: "#ffb3b5", surface: "#1c1015" }
};
const roles = [
    "primary", "on-primary", "primary-container", "on-primary-container", "inverse-primary",
    "primary-dim", "primary-fixed", "primary-fixed-dim", "on-primary-fixed", "on-primary-fixed-variant",
    "secondary", "on-secondary", "secondary-container", "on-secondary-container",
    "secondary-dim", "secondary-fixed", "secondary-fixed-dim", "on-secondary-fixed", "on-secondary-fixed-variant",
    "tertiary", "on-tertiary", "tertiary-container", "on-tertiary-container",
    "tertiary-dim", "tertiary-fixed", "tertiary-fixed-dim", "on-tertiary-fixed", "on-tertiary-fixed-variant",
    "error", "error-dim", "on-error", "error-container", "on-error-container",
    "background", "on-background", "surface", "surface-variant", "surface-dim", "surface-bright",
    "surface-container-lowest", "surface-container-low", "surface-container", "surface-container-high", "surface-container-highest",
    "on-surface", "on-surface-variant", "surface-tint", "outline", "outline-variant", "inverse-surface", "inverse-on-surface", "shadow", "scrim"
] as const;
type Palette = Record<string, string>;

for (const mode of ["light", "dark"] as const) {
    test(`static default ${mode} palette matches the applied default Vibrant palette`, async ({ page }) => {
        await page.emulateMedia({ colorScheme: mode });
        await page.goto("/browser-harness.html?material=1");
        await expect(page.locator("md-filled-button").filter({ hasText: "Open dialog" })).toBeVisible();
        expect(await page.locator("html").getAttribute("data-source-color")).toBeNull();
        expect(await page.locator("html").evaluate((element) => element.style.getPropertyValue("--md-sys-color-primary"))).toBe("");
        const staticPalette = await palette(page);
        expect(staticPalette).toMatchObject(defaultRoles[mode]);
        await expect(page.locator("html")).toHaveCSS("color-scheme", mode);
        assertReadableRoles(staticPalette);

        await openTheme(page, { "theme-mode": mode });
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#6CB6FF");
        expect(await palette(page)).toEqual(staticPalette);
        await expect(page.locator("html")).toHaveCSS("color-scheme", mode);
        await page.emulateMedia({ colorScheme: mode === "light" ? "dark" : "light" });
        expect(await palette(page)).toEqual(staticPalette);
        await expect(page.locator("html")).toHaveCSS("color-scheme", mode);
    });

    test(`custom ${mode} source updates the whole palette and preserves lowercase HEX meaning`, async ({ page }) => {
        await page.emulateMedia({ colorScheme: mode });
        await openTheme(page, { "theme-mode": mode });
        const defaults = await palette(page);
        await openTheme(page, { "theme-mode": mode, "theme-source": "#E040A0" });
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
        const custom = await palette(page);
        expect(custom).toMatchObject(customRoles[mode]);
        assertReadableRoles(custom);
        for (const role of ["primary", "secondary", "tertiary", "surface", "surface-container", "on-surface", "on-surface-variant", "outline"]) {
            expect(custom[role], `custom ${role} must not retain the default blue palette`).not.toBe(defaults[role]);
        }
        for (const role of ["error", "on-error", "error-container", "on-error-container"]) {
            expect(custom[role], `the standard error palette is independent of the source`).toBe(defaults[role]);
        }
        await expect(page.locator("html")).toHaveCSS("color-scheme", mode);
        await openTheme(page, { "theme-mode": mode, "theme-source": "#e040a0" });
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
        expect(await palette(page)).toEqual(custom);
    });
}

for (const source of [undefined, "#E040A0"]) {
    test(`system appearance tracks media changes for ${source ?? "the product default"} without reloading settings`, async ({ page }) => {
        await page.emulateMedia({ colorScheme: "light" });
        await openTheme(page, { "theme-mode": "system", ...(source === undefined ? {} : { "theme-source": source }) });
        const light = await palette(page);
        await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
        await page.emulateMedia({ colorScheme: "dark" });
        await expect.poll(async () => (await palette(page)).primary).not.toBe(light.primary);
        const dark = await palette(page);
        assertReadableRoles(dark);
        for (const role of ["primary", "on-primary", "secondary", "on-secondary", "tertiary", "on-tertiary", "error", "on-error", "surface", "on-surface", "primary-container", "on-primary-container"]) {
            expect(dark[role], `${role} must follow the system mode`).not.toBe(light[role]);
        }
        await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
        await page.emulateMedia({ colorScheme: "light" });
        await expect.poll(() => palette(page)).toEqual(light);
        await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
        expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
    });
}

test("Appearance contains inline Theme controls and all nine schemes save automatically", async ({ page }) => {
    await openSettings(page, { "theme-mode": "light" });
    const appearance = page.getByRole("region", { name: "Appearance", exact: true });
    await expect(page.getByRole("button", { name: "Theme", exact: true })).toHaveCount(0);
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toHaveCount(0);
    const languageLabel = appearance.locator('label[for="settings-locale"]');
    await expect(languageLabel).toHaveText("Language");
    await expect(languageLabel).toBeVisible();
    const labelBox = (await languageLabel.boundingBox())!;
    const languageBox = (await appearance.getByRole("combobox", { name: "Language", exact: true }).boundingBox())!;
    expect(languageBox.x).toBeGreaterThan(labelBox.x + labelBox.width);
    expect(Math.min(labelBox.y + labelBox.height, languageBox.y + languageBox.height) - Math.max(labelBox.y, languageBox.y)).toBeGreaterThan(0);
    await expect(page.getByRole("region", { name: "Theme", exact: true })).toBeVisible();
    await expect(themeEditor(page)).toBeVisible();
    const schemes = ["tonal-spot", "fidelity", "monochrome", "neutral", "vibrant", "expressive", "content", "rainbow", "fruit-salad"];
    await expect(page.locator("#theme-scheme md-select-option")).toHaveCount(schemes.length);
    const combinations = new Set<string>();
    for (const mode of ["light", "dark"]) {
        await selectThemeOption(page, "theme-mode", mode);
        for (const scheme of schemes) {
            await selectThemeOption(page, "theme-scheme", scheme);
            await expect(page.locator("#theme-scheme")).toHaveJSProperty("value", scheme);
            await expect(page.locator("html")).toHaveCSS("color-scheme", mode);
            const colors = await palette(page);
            assertReadableRoles(colors);
            combinations.add([colors.primary, colors.secondary, colors.tertiary].join("/"));
            expect(await page.evaluate(() => window.settingsRequests.every(request => Object.keys(request.update).every(key => key === "appearance")))).toBe(true);
        }
    }
    expect(combinations.size).toBeGreaterThan(9);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(19);
    await expect(themeEditor(page).getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ appearance: { scheme: "fruit-salad" } });
});

test("HEX and color picker stay linked without sliders and invalid HEX cannot write", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light" });
    const hex = page.locator("#theme-source").getByRole("textbox");
    await hex.fill("  #ff0000  ");
    await hex.blur();
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#FF0000");
    await expect(hex).toHaveValue("#FF0000");
    await expect(page.locator("#theme-picker")).toHaveValue("#ff0000");
    await expect(themeEditor(page).locator("md-slider, #theme-hue, #theme-chroma, #theme-tone")).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    const writes = await page.evaluate(() => window.settingsRequests.length);
    const red = await palette(page);
    await hex.fill("#nothex");
    await hex.blur();
    await expect(hex).toHaveValue("#FF0000");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(writes);
    expect(await palette(page)).toEqual(red);

    await setNativeControl(page, "theme-picker", "#00ff00");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#00FF00");
    await expect(hex).toHaveValue("#00FF00");
    assertReadableRoles(await palette(page));
    await setNativeControl(page, "theme-picker", "#ffffff");
    await expect(hex).toHaveValue("#FFFFFF");
    await expect(page.locator("#theme-picker")).toHaveValue("#ffffff");
    await setNativeControl(page, "theme-picker", "#000000");
    await expect(hex).toHaveValue("#000000");
    await expect(page.locator("#theme-picker")).toHaveValue("#000000");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(4);
});

test("theme changes remain editable and coalesce behind an in-flight automatic save", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "hold" });
    await enterHex(page, "#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await enterHex(page, "#112233");
    await enterHex(page, "#445566");
    await selectThemeOption(page, "theme-scheme", "neutral");
    await selectThemeOption(page, "theme-mode", "dark");
    await expect(page.locator("#theme-source").getByRole("textbox")).toBeEditable();
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#445566");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await expect(themeEditor(page).getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Theme", exact: true }).getByRole("button", { name: "Restore defaults", exact: true })).toBeDisabled();
    await releaseSave(page);
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(2);
    await releaseSave(page);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([
        { expectedRevision: 7, update: { appearance: { sourceColor: "#E040A0" } } },
        { expectedRevision: 8, update: { appearance: { sourceColor: "#445566", scheme: "neutral", mode: "dark" } } }
    ]);
});

test("automatic saving keeps theme geometry stable without footer controls", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "hold" });
    await settleFrames(page);
    const editor = themeEditor(page);
    const before = (await editor.boundingBox())!;
    await enterHex(page, "#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await settleFrames(page);
    await expect(editor.locator(".theme-editor-footer")).toHaveCount(0);
    await expect(editor.getByRole("button")).toHaveCount(0);
    const during = (await editor.boundingBox())!;
    for (const coordinate of ["x", "y", "width", "height"] as const) expect(Math.abs(during[coordinate] - before[coordinate]), `editor ${coordinate}`).toBeLessThanOrEqual(1);
    await releaseSave(page);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
});

test("consecutive theme edits save only changed fields and remain editable", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light" });
    await enterHex(page, "#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await enterHex(page, "#006A60");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await selectThemeOption(page, "theme-mode", "dark");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(3);
    await selectThemeOption(page, "theme-scheme", "neutral");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(4);
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.update))).toEqual([
        { appearance: { sourceColor: "#E040A0" } }, { appearance: { sourceColor: "#006A60" } },
        { appearance: { mode: "dark" } }, { appearance: { scheme: "neutral" } }
    ]);
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#006A60");
    await expect(themeEditor(page).getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
});

test("Restore defaults confirms before persisting only the theme card", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openTheme(page, { "theme-mode": "dark", "theme-source": "#E040A0", "theme-scheme": "expressive" });
    await page.getByRole("region", { name: "Theme", exact: true }).getByRole("button", { name: "Restore defaults", exact: true }).click();
    const confirmation = page.locator("md-dialog[open]");
    await expect(confirmation).toContainText("Restore the default settings for Theme?");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await confirmation.getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(confirmation).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#6CB6FF");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "light");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([
        { expectedRevision: 7, update: { appearance: { mode: "system", sourceColor: null, scheme: "vibrant" } } }
    ]);
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    expect(await palette(page)).toMatchObject(defaultRoles.dark);
});

test("failed theme autosave retains editable values and uses the shared Retry", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "error-once" });
    await enterHex(page, "#E040A0");
    await expect(page.getByRole("alert")).toContainText("Settings save result is unknown");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
    await expect(page.locator("#theme-source").getByRole("textbox")).toBeEditable();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.expectedRevision))).toEqual([7, 7]);
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ appearance: { sourceColor: "#E040A0" } });
});

test("dismissing a theme failure preserves its intent and the settings header can retry corrected input", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "error-once" });
    await enterHex(page, "#E040A0");
    await expect(page.getByRole("alert")).toContainText("Settings save result is unknown");
    await page.locator(".reminder-floating").getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toBeHidden();
    await enterHex(page, "#006A60");
    await settleFrames(page);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.getByRole("button", { name: "Retry saving settings", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#006A60");
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ appearance: { sourceColor: "#006A60" } });
});

test("a revision conflict rereads settings and retries only local theme fields over external changes", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "conflict-once" });
    const reads = await page.evaluate(() => window.settingsReads);
    await enterHex(page, "#E040A0");
    await expect(page.locator(".reminder-floating").getByRole("alert")).toContainText("Settings could not be saved.");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsReads)).toBeGreaterThan(reads);
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.expectedRevision))).toEqual([7, 8]);
    expect(await page.evaluate(() => window.settingsRequests.at(-1)!.update)).toEqual({ appearance: { sourceColor: "#E040A0" } });
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "reduced");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    await showThemeEditor(page);
    await expect(page.locator("#theme-mode")).toHaveJSProperty("value", "dark");
    await expect(page.locator("#theme-scheme")).toHaveJSProperty("value", "neutral");
    await page.evaluate(() => {
        window.history.pushState(null, "", "/projects/00000000-0000-4000-8000-000000000101/packages");
        window.dispatchEvent(new PopStateEvent("popstate"));
    });
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Package filters", exact: true }).getByRole("checkbox", { name: "Show prerelease versions", exact: true })).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.every((request) => request.update.packages === undefined))).toBe(true);
});

test("theme autosave preserves ordinary settings that were already saved", async ({ page }) => {
    await openSettings(page, { "theme-mode": "light" });
    await selectThemeOption(page, "settings-motion", "full");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await showThemeEditor(page);
    await enterHex(page, "#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(3);
    await expect(page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true })).toBeChecked();
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.settingsRequests.at(-1)!.expectedRevision)).toBe(9);
    expect(await page.evaluate(() => window.settingsRequests.at(-1)!.update)).toEqual({ appearance: { sourceColor: "#E040A0" } });
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
});

test("changing settings sections retains controls and automatic theme writes", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light" });
    const original = await themeEditor(page).elementHandle();
    await enterHex(page, "#E040A0");
    await selectThemeOption(page, "theme-mode", "dark");
    await selectThemeOption(page, "theme-scheme", "neutral");
    const preview = await palette(page);
    for (const section of ["general", "unity", "notifications"] as const) {
        await selectSettingsSection(page, section);
        await expect(themeEditor(page)).toHaveCount(0);
        expect(await palette(page)).toEqual(preview);
        expect(await page.evaluate(() => window.settingsWrites.length)).toBeGreaterThan(0);
    }
    await showThemeEditor(page);
    expect(await themeEditor(page).evaluate((element, previous) => element === previous, original)).toBe(true);
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
    await expect(page.locator("#theme-mode")).toHaveJSProperty("value", "dark");
    await expect(page.locator("#theme-scheme")).toHaveJSProperty("value", "neutral");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(3);
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.update))).toEqual([
        { appearance: { sourceColor: "#E040A0" } }, { appearance: { mode: "dark" } }, { appearance: { scheme: "neutral" } }
    ]);
});

test("navigation keeps an in-flight theme autosave without a discard confirmation", async ({ page }) => {
    await openTheme(page, { "theme-mode": "light", "theme-save": "hold" });
    await enterHex(page, "#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await navigationItem(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toBeHidden();
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
    await releaseSave(page);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await navigationItem(page, "Settings").click();
    await showThemeEditor(page);
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
});

for (const acceptedBeforeOldRead of [false, true]) {
    test(`a delayed initial shell read cannot replace ${acceptedBeforeOldRead ? "an accepted theme" : "an in-flight automatic theme"}`, async ({ page }) => {
        const query = new URLSearchParams({ route: "/projects", "theme-mode": "light", "theme-delay-initial-read": "1", ...(acceptedBeforeOldRead ? {} : { "theme-save": "hold" }) });
        await page.goto(`/browser-harness.html?${query.toString()}`);
        await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
        await expect.poll(() => page.evaluate(() => window.heldInitialSettingsReads)).toBeGreaterThan(0);
        const held = await page.evaluate(() => window.heldInitialSettingsReads);
        await navigationItem(page, "Settings").click();
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
        await showThemeEditor(page);
        await enterHex(page, "#E040A0");
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
        await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
        if (acceptedBeforeOldRead) await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
        await page.evaluate(() => window.dispatchEvent(new Event("test-theme-initial-read-ready")));
        await expect.poll(() => page.evaluate(() => window.releasedInitialSettingsReads)).toBe(held);
        await settleFrames(page);
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
        expect(await palette(page)).toMatchObject(customRoles.light);
        expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
        if (!acceptedBeforeOldRead) { await releaseSave(page); await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1); }
    });
}

for (const language of [
    { locale: "zh-CN", source: "源色", picker: "选择主题颜色", mode: "颜色模式", scheme: "配色方案", theme: "主题", light: "浅色", dark: "深色", defaults: "恢复默认", save: "保存", cancel: "取消", vibrant: "活力" },
    { locale: "ja-JP", source: "ソースカラー", picker: "テーマカラーを選択", mode: "カラーモード", scheme: "配色スキーム", theme: "テーマ", light: "ライト", dark: "ダーク", defaults: "既定値に戻す", save: "保存", cancel: "キャンセル", vibrant: "ビブラント" }
]) {
    test(`theme controls expose translated labels and schemes in ${language.locale}`, async ({ page }) => {
        await openSettings(page, { "theme-mode": "light" });
        await selectThemeOption(page, "settings-locale", language.locale);
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        const editor = page.getByRole("form", { name: language.theme, exact: true });
        await expect(editor).toBeVisible();
        await expect(page.getByRole("dialog", { name: language.theme, exact: true })).toHaveCount(0);
        await expect(page.locator("#theme-source").getByRole("textbox")).toHaveAccessibleName(language.source);
        await expect(page.locator("#theme-picker")).toHaveAccessibleName(language.picker);
        await expect(page.getByRole("combobox", { name: language.scheme, exact: true })).toBeVisible();
        await expect(page.locator("#theme-mode")).toHaveJSProperty("ariaLabel", language.mode);
        await expect(editor.locator("md-slider")).toHaveCount(0);
        await expect(page.getByRole("combobox", { name: language.mode, exact: true })).toBeVisible();
        await expect(page.locator("#theme-mode md-select-option")).toHaveCount(3);
        await expect(page.locator("#theme-mode md-select-option").nth(1)).toHaveText(language.light);
        await expect(page.locator("#theme-mode md-select-option").nth(2)).toHaveText(language.dark);
        await expect(page.getByRole("region", { name: language.theme, exact: true }).getByRole("button", { name: language.defaults, exact: true })).toBeVisible();
        await expect(page.locator("#theme-scheme md-select-option")).toHaveCount(9);
        await expect(page.locator("#theme-scheme")).toContainText(language.vibrant);
        await expect(page.locator("html")).toHaveAttribute("data-source-color", "#6CB6FF");
        await expect(editor.getByRole("button", { name: language.save, exact: true })).toHaveCount(0);
        await expect(editor.getByRole("button", { name: language.cancel, exact: true })).toHaveCount(0);
        await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
        await expect(editor).toBeVisible();
        await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
        await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    });
}

async function openSettings(page: Page, settings: Record<string, string>) {
    const query = new URLSearchParams({ route: "/settings", ...settings });
    await page.goto(`/browser-harness.html?${query.toString()}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await selectSettingsSection(page, "appearance");
    await expect(page.locator("#settings-locale")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-source-color", (settings["theme-source"] ?? "#6CB6FF").toUpperCase());
}

async function openTheme(page: Page, settings: Record<string, string>) {
    await openSettings(page, settings);
    await showThemeEditor(page);
}

async function showThemeEditor(page: Page) {
    await selectSettingsSection(page, "appearance");
    await expect(themeEditor(page)).toBeVisible();
}

function themeEditor(page: Page) {
    return page.getByRole("form", { name: "Theme", exact: true });
}

async function enterHex(page: Page, value: string) {
    await page.locator("#theme-source").getByRole("textbox").fill(value);
}

async function selectThemeOption(page: Page, id: string, value: string) {
    if (id.startsWith("settings-")) await selectSettingsSection(page, "appearance");
    await page.locator(`#${id}`).evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

async function setNativeControl(page: Page, id: string, value: string) {
    await page.locator(`#${id}`).evaluate((element, next) => {
        if (element instanceof HTMLInputElement) {
            const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!;
            setter.call(element, next);
        } else {
            (element as HTMLElement & { value: number }).value = Number(next);
        }
        element.dispatchEvent(new Event("input", { bubbles: true }));
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

async function releaseSave(page: Page) {
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
}

function navigationItem(page: Page, label: "Projects" | "Settings") {
    return page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${label}$`) }) });
}

async function settleFrames(page: Page) {
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
}

async function palette(page: Page): Promise<Palette> {
    return page.locator("html").evaluate((element, names) => {
        const style = getComputedStyle(element);
        return Object.fromEntries(names.map((name) => [name, style.getPropertyValue(`--md-sys-color-${name}`).trim().toLowerCase()]));
    }, [...roles]);
}

function assertReadableRoles(colors: Palette) {
    for (const name of roles) expect(colors[name], `Material role ${name}`).toMatch(/^#[0-9a-f]{6}$/);
    const pairs = [
        ["on-primary", "primary"], ["on-primary-container", "primary-container"],
        ["on-secondary", "secondary"], ["on-secondary-container", "secondary-container"],
        ["on-tertiary", "tertiary"], ["on-tertiary-container", "tertiary-container"],
        ["on-error", "error"], ["on-error-container", "error-container"],
        ["on-background", "background"], ["on-surface", "surface"],
        ["on-surface-variant", "surface"], ["on-surface-variant", "surface-variant"], ["on-surface", "surface-container-lowest"],
        ["on-surface", "surface-container-low"], ["on-surface", "surface-container"],
        ["on-surface", "surface-container-high"], ["on-surface", "surface-container-highest"],
        ["inverse-on-surface", "inverse-surface"]
    ];
    for (const family of ["primary", "secondary", "tertiary"]) {
        pairs.push([`on-${family}-fixed`, `${family}-fixed`]);
        pairs.push([`on-${family}-fixed-variant`, `${family}-fixed`]);
        pairs.push([`on-${family}-fixed`, `${family}-fixed-dim`]);
        pairs.push([`on-${family}-fixed-variant`, `${family}-fixed-dim`]);
    }
    for (const [foreground, background] of pairs) {
        const ratio = contrast(colors[foreground!]!, colors[background!]!);
        expect(ratio, `${foreground} on ${background} normal text contrast`).toBeGreaterThanOrEqual(4.5);
    }
}

function contrast(first: string, second: string): number {
    const luminance = (hex: string) => {
        const linear = [1, 3, 5].map((start) => {
            const srgb = parseInt(hex.slice(start, start + 2), 16) / 255;
            return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * linear[0]! + 0.7152 * linear[1]! + 0.0722 * linear[2]!;
    };
    const values = [luminance(first), luminance(second)].sort((left, right) => right - left);
    return (values[0]! + 0.05) / (values[1]! + 0.05);
}
