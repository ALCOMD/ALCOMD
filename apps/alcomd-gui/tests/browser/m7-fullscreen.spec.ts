import { expect, test, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const windowCard = (page: Page) => page.getByRole("region", { name: "Window", exact: true });
const fullscreen = (page: Page) => windowCard(page).getByRole("checkbox", { name: "Full screen mode (F11)", exact: true });
const remember = (page: Page) => windowCard(page).getByRole("checkbox", { name: "Remember window size and maximized state", exact: true });
const reset = (page: Page) => windowCard(page).getByRole("button", { name: "Restore defaults", exact: true });

async function open(page: Page, query = "") {
    await page.goto(`/browser-harness.html?route=/settings${query}`);
    await selectSettingsSection(page, "appearance");
    await expect(fullscreen(page)).toBeVisible();
}

test("fullscreen defaults off and persists as its own field across navigation", async ({ page }) => {
    await open(page);
    await expect(fullscreen(page)).not.toBeChecked();
    await expect(reset(page)).toBeDisabled();
    await fullscreen(page).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(remember(page)).not.toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ window: { fullscreen: true } });
    await page.evaluate(() => { window.history.pushState(null, "", "/projects"); window.dispatchEvent(new PopStateEvent("popstate")); });
    await expect(page.getByRole("heading", { name: "Projects", exact: true, level: 1 })).toBeVisible();
    await page.evaluate(() => { window.history.pushState(null, "", "/settings"); window.dispatchEvent(new PopStateEvent("popstate")); });
    await selectSettingsSection(page, "appearance");
    await expect(fullscreen(page)).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
});

test("F11 changes the persisted preference and ignores repeats and modifiers", async ({ page }) => {
    await open(page, "&window-fullscreen=1");
    await expect(fullscreen(page)).toBeChecked();
    await page.keyboard.press("Control+F11");
    await page.evaluate(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "F11", repeat: true, bubbles: true })));
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await page.keyboard.press("F11");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(fullscreen(page)).not.toBeChecked();
    await page.keyboard.press("F11");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await expect(fullscreen(page)).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.update))).toEqual([{ window: { fullscreen: false } }, { window: { fullscreen: true } }]);
});

test("window reset confirms and resets both fullscreen and geometry memory", async ({ page }) => {
    await open(page, "&window-fullscreen=1&theme-source=%23E040A0");
    await remember(page).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await reset(page).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(reset(page)).toBeFocused();
    await expect(fullscreen(page)).toBeChecked();
    await expect(remember(page)).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await reset(page).click();
    await dialog.getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(reset(page)).toBeDisabled();
    await expect(windowCard(page).getByRole("heading")).toBeFocused();
    await expect(fullscreen(page)).not.toBeChecked();
    await expect(remember(page)).not.toBeChecked();
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ window: { rememberState: false, fullscreen: false } });
});

test("an unknown fullscreen save reconciles without another committed write", async ({ page }) => {
    await open(page, "&theme-save=commit-error-once");
    await fullscreen(page).check();
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("Settings save result is unknown");
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await expect(fullscreen(page)).toBeChecked();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
});

for (const [locale, label] of [["en-US", "Full screen mode (F11)"], ["zh-CN", "全屏模式（F11）"], ["ja-JP", "全画面モード（F11）"]]) {
    test(`fullscreen has a translated checkbox in ${locale}`, async ({ page }) => {
        await open(page);
        await page.locator("#settings-locale").evaluate((element, next) => {
            (element as HTMLElement & { select(value: string): void }).select(next!);
            element.dispatchEvent(new Event("change", { bubbles: true }));
        }, locale);
        await expect(page.getByRole("checkbox", { name: label, exact: true })).toBeVisible();
    });
}
