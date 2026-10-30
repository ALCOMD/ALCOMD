import { expect, test, type Page } from "@playwright/test";

test("ordinary settings save immediately as narrow updates without a save bar", async ({ page }) => {
    await openSettings(page);
    await expect(page.locator(".settings-save-bar")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Save settings", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Discard changes", exact: true })).toHaveCount(0);
    await expect(page.getByText("All changes saved", { exact: true })).toHaveCount(0);
    await page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { packages: { showPrerelease: true } } }]);
    await selectMotion(page, "full");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.settingsRequests.at(-1))).toEqual({ expectedRevision: 8, update: { appearance: { motion: "full" } } });
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toHaveCount(0);
    await navigation(page, "Settings").click();
    await expect(page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true })).toBeChecked();
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
    await page.getByRole("region", { name: "System information", exact: true }).scrollIntoViewIfNeeded();
    await page.screenshot({ path: "../../target/m7-settings-autosave-20261030/settings-without-save-bar.png", animations: "disabled" });
});

test("rapid changes survive navigation while writes stay serialized", async ({ page }) => {
    await openSettings(page, "hold");
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true }).check();
    await page.getByRole("checkbox", { name: "Hide local User Packages", exact: true }).check();
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await release(page);
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests[1])).toEqual({ expectedRevision: 8, update: { packages: { showPrerelease: true, hideLocalUserPackages: true } } });
    await release(page);
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await navigation(page, "Settings").click();
    for (const name of ["Compact GUI", "Show prerelease package versions", "Hide local User Packages"]) await expect(page.getByRole("checkbox", { name, exact: true })).toBeChecked();
});

test("failed autosave is visible after navigation and explicit Retry saves retained intent", async ({ page }) => {
    await openSettings(page, "error-once");
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).check();
    await expect(page.getByRole("alert")).toContainText("Settings could not be saved.");
    await expect(page.locator("html")).toHaveAttribute("data-density", "comfortable");
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toBeVisible();
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ appearance: { density: "compact" } }, { appearance: { density: "compact" } }]);
    await navigation(page, "Settings").click();
    await expect(page.getByRole("checkbox", { name: "Compact GUI", exact: true })).toBeChecked();
});

test("unknown commit outcome reconciles without a duplicate save", async ({ page }) => {
    await openSettings(page, "commit-error-once");
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).check();
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-density", "comfortable");
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.getByRole("alert").getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
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

test("theme retains its own preview, Cancel and Save while ordinary settings persist independently", async ({ page }) => {
    await openSettings(page);
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeHidden();
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#6CB6FF");
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeHidden();
    expect(await page.evaluate(() => window.settingsRequests.at(-1)!.update)).toEqual({ appearance: { mode: "system", scheme: "vibrant", sourceColor: "#E040A0" } });
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
});

async function openSettings(page: Page, outcome?: string) {
    await page.goto(`/browser-harness.html?route=/settings${outcome === undefined ? "" : `&theme-save=${outcome}`}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await expect(page.locator("#settings-motion")).toBeVisible();
}
async function selectMotion(page: Page, value: string) {
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
