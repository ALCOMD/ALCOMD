import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";

test("ordinary settings save immediately as narrow updates without a save bar", async ({ page }) => {
    await openSettings(page);
    await expect(page.locator(".settings-save-bar")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Save settings", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Discard changes", exact: true })).toHaveCount(0);
    await expect(page.getByText("All changes saved", { exact: true })).toHaveCount(0);
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { window: { rememberState: true } } }]);
    await selectMotion(page, "full");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.settingsRequests.at(-1))).toEqual({ expectedRevision: 8, update: { appearance: { motion: "full" } } });
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toHaveCount(0);
    await navigation(page, "Settings").click();
    await selectSettingsSection(page, "appearance");
    await expect(page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true })).toBeChecked();
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
    await selectSettingsSection(page, "appearance");
    await page.screenshot({ path: test.info().outputPath("settings-without-save-bar.png"), animations: "disabled" });
});

test("Settings keeps one desktop density without a compact mode control or density updates", async ({ page }) => {
    await openSettings(page);
    await expect(page.getByRole("checkbox", { name: "Compact GUI", exact: true })).toHaveCount(0);
    await expect(page.locator("#settings-density")).toHaveCount(0);
    await expect(page.locator("html")).not.toHaveAttribute("data-density");
    await selectMotion(page, "full");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.locator("html")).not.toHaveAttribute("data-density");
    await navigation(page, "Settings").click();
    await expect(page.getByRole("checkbox", { name: "Compact GUI", exact: true })).toHaveCount(0);
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { appearance: { motion: "full" } } }]);
    expect(await page.evaluate(() => window.settingsRequests.every((request) => !Object.hasOwn(request.update.appearance ?? {}, "density")))).toBe(true);
});

test("rapid changes survive navigation while writes stay serialized", async ({ page }) => {
    await openSettings(page, "hold");
    await selectMotion(page, "full");
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).check();
    await selectMotion(page, "reduced");
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await release(page);
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests[1])).toEqual({ expectedRevision: 8, update: { appearance: { motion: "reduced" }, window: { rememberState: true } } });
    await release(page);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await navigation(page, "Settings").click();
    await selectSettingsSection(page, "appearance");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "reduced");
    await expect(page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true })).toBeChecked();
});

test("failed autosave is visible after navigation and explicit Retry saves retained intent", async ({ page }) => {
    await openSettings(page, "error-once");
    await selectMotion(page, "full");
    await expect(page.getByRole("alert")).toContainText("Settings save result is unknown");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toBeVisible();
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ appearance: { motion: "full" } }, { appearance: { motion: "full" } }]);
    await navigation(page, "Settings").click();
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
});

test("unknown commit outcome reconciles without a duplicate save", async ({ page }) => {
    await openSettings(page, "commit-error-once");
    await selectMotion(page, "full");
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
});

test("conflict Retry preserves external theme fields and changes only the requested setting", async ({ page }) => {
    await openSettings(page, "conflict-once");
    await selectMotion(page, "full");
    await expect(page.getByRole("alert")).toBeVisible();
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    await expect(page.locator("html")).toHaveAttribute("data-theme-scheme", "neutral");
    await expect(page.locator("html")).toHaveCSS("color-scheme", "dark");
    expect(await page.evaluate(() => window.settingsRequests.at(-1))).toEqual({ expectedRevision: 8, update: { appearance: { motion: "full" } } });
});

test("theme shares automatic saving with ordinary settings and has no separate Save or Cancel", async ({ page }) => {
    await openSettings(page);
    await selectMotion(page, "full");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    await expect(theme).toBeVisible();
    await expect(theme.getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests.at(-1)!.update)).toEqual({ appearance: { sourceColor: "#E040A0" } });
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
});

async function openSettings(page: Page, outcome?: string) {
    await page.goto(`/browser-harness.html?route=/settings${outcome === undefined ? "" : `&theme-save=${outcome}`}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await selectSettingsSection(page, "appearance");
    await expect(page.locator("#settings-motion")).toBeVisible();
}
async function selectMotion(page: Page, value: string) {
    await selectSettingsSection(page, "appearance");
    await page.locator("#settings-motion").evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}
function navigation(page: Page, name: "Projects" | "Settings") {
    return page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });
}
async function release(page: Page) {
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
}
