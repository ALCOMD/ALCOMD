import { expect, test, type Page } from "@playwright/test";
import { createTranslator } from "../../src/i18n";
import { selectSettingsSection } from "./settings-sections";

async function openSettings(page: Page) {
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto("/browser-harness.html?route=/settings&h5-settings=1");
    await expect(page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true })).toBeVisible();
}
const changes = (page: Page) => page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.update"));

for (const locale of ["en-US", "zh-CN", "ja-JP"] as const) {
    test(`window preference defaults off and saves narrow toggles across groups in ${locale}`, async ({ page }) => {
        await openSettings(page);
        if (locale !== "en-US") {
            await selectSettingsSection(page, "appearance");
            await page.locator("#settings-locale").evaluate((element, value) => {
                (element as HTMLElement & { select(value: string): void }).select(value);
                element.dispatchEvent(new Event("change", { bubbles: true }));
            }, locale);
            await expect(page.locator("html")).toHaveAttribute("lang", locale);
            await selectSettingsSection(page, "general");
        }
        const t = createTranslator(locale);
        const region = page.getByRole("region", { name: t("Window"), exact: true });
        const checkbox = page.getByRole("checkbox", { name: t("Remember window size and maximized state"), exact: true });
        await expect(region).toBeVisible();
        await expect(checkbox).not.toBeChecked();
        const original = await checkbox.elementHandle();
        await checkbox.check();
        await expect.poll(() => page.evaluate(() => window.h5Settings.commits.filter((request) => request.update.window !== undefined).length)).toBe(1);
        await selectSettingsSection(page, "unity");
        await expect(region).toHaveCount(0);
        await selectSettingsSection(page, "general");
        await expect(checkbox).toBeChecked();
        expect(await checkbox.evaluate((element, first) => element === first, original)).toBe(true);
        await checkbox.uncheck();
        await expect.poll(() => page.evaluate(() => window.h5Settings.commits.filter((request) => request.update.window !== undefined).length)).toBe(2);
        const revision = locale === "en-US" ? 7 : 8;
        expect((await changes(page)).filter((request) => (request.params.update as { window?: unknown }).window !== undefined).map((request) => request.params)).toEqual([
            { expectedRevision: revision, update: { window: { rememberState: true } } },
            { expectedRevision: revision + 1, update: { window: { rememberState: false } } }
        ]);
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await expect(page.getByRole("button", { name: "Save settings", exact: true })).toHaveCount(0);
    });
}

test("failed window preference keeps the intent and uses explicit Retry after checking Core", async ({ page }) => {
    await openSettings(page);
    await page.evaluate(() => window.h5Settings.failNext("settings.update", "permission_denied"));
    const checkbox = page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true });
    await checkbox.check();
    const feedback = page.locator(".settings-save-error").getByRole("alert");
    await expect(feedback).toContainText("You do not have permission for this action.");
    expect(await page.evaluate(() => window.h5Settings.commits)).toEqual([]);
    await selectSettingsSection(page, "unity");
    await selectSettingsSection(page, "general");
    await expect(checkbox).toBeChecked();
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    expect((await changes(page)).map((request) => request.params)).toEqual([
        { expectedRevision: 7, update: { window: { rememberState: true } } },
        { expectedRevision: 7, update: { window: { rememberState: true } } }
    ]);
    expect(await page.evaluate(() => window.h5Settings.requests.slice(-2).map((request) => request.method))).toEqual(["settings.get", "settings.update"]);
});

test("an unknown committed window setting is reconciled without another write", async ({ page }) => {
    await openSettings(page);
    await page.evaluate(() => window.h5Settings.failNext("settings.update", "daemon_unavailable", true));
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).check();
    const feedback = page.locator(".settings-save-error").getByRole("alert");
    await expect(feedback).toContainText("Settings save result is unknown");
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    const reads = await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length)).toBe(reads + 1);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
});
