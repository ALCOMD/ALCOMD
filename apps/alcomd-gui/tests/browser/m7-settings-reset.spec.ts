import { expect, test, type Locator, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const card = (page: Page, name: string) => page.getByRole("region", { name, exact: true });
const reset = (region: Locator) => region.locator(".settings-section-heading").getByRole("button", { name: "Restore defaults", exact: true });
const dialog = (page: Page) => page.locator("md-dialog[open]");

test("six cards have title-row resets disabled at defaults and the two Unity exceptions retain their actions", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&backup-compression=level5");
    for (const [category, names] of [["general", ["Projects", "Backups"]], ["unity", ["Default Unity launch arguments"]], ["appearance", ["Appearance", "Window", "Theme"]]] as const) {
        await selectSettingsSection(page, category);
        for (const name of names) {
            const region = card(page, name);
            const button = reset(region);
            await expect(button).toBeVisible();
            const headingBounds = (await region.getByRole("heading", { name, exact: true }).boundingBox())!;
            const buttonBounds = (await button.boundingBox())!;
            expect(Math.abs((headingBounds.y + headingBounds.height / 2) - (buttonBounds.y + buttonBounds.height / 2))).toBeLessThanOrEqual(2);
            if (name === "Appearance") {
                // The harness starts with an explicit English locale rather than system.
                await expect(button).toBeEnabled();
                await button.click();
                await expect(dialog(page)).toContainText(`Restore the default settings for ${name}?`);
                await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
                await expect(button).toBeFocused();
                await select(page.locator("#settings-locale"), "system");
            }
            await expect(button).toBeDisabled();
        }
    }
    await selectSettingsSection(page, "unity");
    for (const name of ["Unity Hub Path", "Unity installations"]) await expect(reset(card(page, name))).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ locale: "system" }]);
});

test("a reset can be cancelled and a confirmed backup reset leaves project preferences alone", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&project-directory=D%3A%5CProjects");
    await select(page.locator("#settings-backup-compression"), "level7");
    await card(page, "Backups").getByRole("checkbox", { name: "Keep Library cache by default", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await reset(card(page, "Backups")).click();
    await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(2);
    await expect(page.locator("#settings-backup-compression")).toHaveJSProperty("value", "level7");
    await reset(card(page, "Backups")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).dblclick();
    await expect(dialog(page)).toHaveCount(0);
    await expect(page.locator("#settings-backup-compression")).toHaveJSProperty("value", "level5");
    await expect(page.getByRole("textbox", { name: "Default project location", exact: true })).toHaveValue("D:\\Projects");
    expect(await page.evaluate(() => window.settingsRequests.at(-1)?.update)).toEqual({ backups: { compressionMode: "level5", includeLibrary: false } });
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(3);
    await expect(reset(card(page, "Backups"))).toBeDisabled();
    await expect(card(page, "Backups").getByRole("heading")).toBeFocused();
});

test("default directory cards allow discarding a draft without writing the default again", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&backup-compression=level5");
    for (const [name, label, defaultPath] of [["Projects", "Default project location", "C:\\Fixture"], ["Backups", "Default backup location", "C:\\Fixture\\Backups"]]) {
        const region = card(page, name!);
        const path = region.getByRole("textbox", { name: label, exact: true });
        await expect(reset(region)).toBeDisabled();
        await path.fill("D:\\Unfinished");
        await expect(reset(region)).toBeEnabled();
        await reset(region).click();
        await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(reset(region)).toBeFocused();
        await expect(path).toHaveValue("D:\\Unfinished");
        await path.focus();
        await page.keyboard.press("Escape");
        await expect(reset(region)).toBeDisabled();
        await path.fill("D:\\Unfinished");
        await reset(region).click();
        await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
        await expect(path).toHaveValue(defaultPath!);
        await expect(reset(region)).toBeDisabled();
        await expect(region.getByRole("heading")).toBeFocused();
    }
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

test("a saved fast backup preference stays unchanged until restoring the new level5 default", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1");
    const compression = page.locator("#settings-backup-compression");
    await expect(compression).toHaveJSProperty("value", "fast");
    await expect(reset(card(page, "Backups"))).toBeEnabled();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await reset(card(page, "Backups")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(compression).toHaveJSProperty("value", "level5");
    await expect(reset(card(page, "Backups"))).toBeDisabled();
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.update))).toEqual([
        { backups: { compressionMode: "level5" } }
    ]);
});

test("theme autosave enables reset and a confirmed restore disables it again", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1");
    await selectSettingsSection(page, "appearance");
    const region = card(page, "Theme");
    const source = page.locator("#theme-source").getByRole("textbox");
    await expect(reset(region)).toBeDisabled();
    await source.fill("#112233");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(reset(region)).toBeEnabled();
    await reset(region).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(source).toHaveValue("#6CB6FF");
    await expect(reset(region)).toBeDisabled();
    await expect(region.getByRole("heading")).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests.map(request => request.update))).toEqual([
        { appearance: { sourceColor: "#112233" } }, { appearance: { sourceColor: null } }
    ]);
});

test("opening and cancelling a project reset does not submit its unfinished path draft", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&project-directory=D%3A%5CProjects");
    const path = page.getByRole("textbox", { name: "Default project location", exact: true });
    await path.fill("D:\\Unfinished");
    await reset(card(page, "Projects")).click();
    await expect(dialog(page)).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(path).toHaveValue("D:\\Unfinished");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await expect(reset(card(page, "Projects"))).toBeFocused();
    await path.focus();
    await page.keyboard.press("Shift+Tab");
    await expect(reset(card(page, "Projects"))).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(dialog(page)).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(reset(card(page, "Projects"))).toBeFocused();
    await expect(path).toHaveValue("D:\\Unfinished");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await reset(card(page, "Projects")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(path).toHaveValue("C:\\Fixture");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ paths: { projectDirectory: null } }]);
});

test("theme reset confirms without a separate save workflow and stays editable", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&theme-source=%23E040A0&theme-mode=dark");
    await selectSettingsSection(page, "appearance");
    const source = page.locator("#theme-source").getByRole("textbox");
    await source.fill("#006A60");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await reset(card(page, "Theme")).click();
    await dialog(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(source).toHaveValue("#006A60");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await reset(card(page, "Theme")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(source).toHaveValue("#6CB6FF");
    await expect(page.locator("#theme-mode")).toHaveJSProperty("value", "system");
    await expect(reset(card(page, "Theme"))).toBeDisabled();
    await expect(card(page, "Theme").getByRole("heading")).toBeFocused();
    await source.fill("#112233");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(3);
});

test("Appearance and Window reset their own values and preserve the saved theme", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h3=1&theme-source=%23E040A0&theme-mode=dark");
    await selectSettingsSection(page, "appearance");
    await select(page.locator("#settings-motion"), "full");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    const remember = card(page, "Window").getByRole("checkbox", { name: "Remember window size and maximized state", exact: true });
    await remember.check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await reset(card(page, "Appearance")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(page.locator("#settings-locale")).toHaveJSProperty("value", "system");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "system");
    await expect(remember).toBeChecked();
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
    await reset(card(page, "Window")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(dialog(page)).toHaveCount(0);
    await expect(remember).not.toBeChecked();
    await expect(reset(card(page, "Appearance"))).toBeDisabled();
    await expect(reset(card(page, "Window"))).toBeDisabled();
    expect(await page.evaluate(() => window.settingsRequests.slice(-2).map((request) => request.update))).toEqual([
        { locale: "system", appearance: { motion: "system" } }, { window: { rememberState: false } }
    ]);
});

async function select(control: Locator, value: string) {
    await control.evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

test("a lost reset response stays open and retry confirms the committed values without another write", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&theme-source=%23E040A0&theme-mode=dark&theme-save=commit-error-once");
    await selectSettingsSection(page, "appearance");
    await reset(card(page, "Theme")).click();
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("Restore result is unknown");
    await expect(dialog(page)).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await dialog(page).getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(reset(card(page, "Theme"))).toBeDisabled();
    await expect(card(page, "Theme").getByRole("heading")).toBeFocused();
    await expect(dialog(page)).toHaveCount(0);
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#6CB6FF");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
});
