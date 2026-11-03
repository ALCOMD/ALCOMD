import { expect, test } from "@playwright/test";
import { createTranslator } from "../../src/i18n";

for (const locale of ["en-US", "zh-CN", "ja-JP"] as const) {
    test(`Settings keeps Notifications blank and switches shared segments in ${locale}`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width: 1280, height: 800 });
        await page.goto("/browser-harness.html?route=/settings&state=ready");
        let navigation = page.locator(".resource-navigation");
        await expect(navigation.getByRole("button")).toHaveCount(4);
        if (locale !== "en-US") {
            await navigation.getByRole("button").nth(2).click();
            await page.locator("#settings-locale").evaluate((element, value) => {
                (element as HTMLElement & { select(value: string): void }).select(value);
                element.dispatchEvent(new Event("change", { bubbles: true }));
            }, locale);
            await expect(page.locator("html")).toHaveAttribute("lang", locale);
            await navigation.getByRole("button").first().click();
        }
        const t = createTranslator(locale);
        navigation = page.getByRole("navigation", { name: t("Settings sections"), exact: true });
        const labels = [t("General"), "Unity", t("Appearance"), t("Notifications")];
        await expect(navigation.getByRole("button")).toHaveText(labels);
        await expect(navigation.locator(".os-scrollbar")).toHaveCount(0);
        await expect(navigation).toHaveCSS("overflow", "visible");
        const writes = await page.evaluate(() => window.settingsRequests.length);
        const path = page.locator("#settings-projectDirectory");
        const original = await path.elementHandle();
        const regions = [[t("Save locations"), t("Packages")], [t("Unity Hub Path"), t("Unity installations"), t("Default Unity launch arguments")], [t("Appearance"), t("Window"), t("Theme")], []];
        for (let index = 0; index < labels.length; index++) {
            await navigation.getByRole("button").nth(index).click();
            await expect(navigation.getByRole("button", { pressed: true })).toHaveCount(1);
            await expect(navigation.getByRole("button").nth(index)).toHaveAttribute("aria-pressed", "true");
            expect(await page.locator(".settings-section:visible").evaluateAll(elements => elements.map(element => element.getAttribute("aria-label")))).toEqual(regions[index]);
            if (index === 3) await expect(page.locator(".settings-workspace")).toHaveText("", { useInnerText: true });
            for (const name of regions.flat()) {
                const region = page.getByRole("region", { name, exact: true });
                if (regions[index]!.includes(name)) await expect(region).toBeVisible();
                else await expect(region).toHaveCount(0);
            }
        }
        await navigation.getByRole("button").first().click();
        expect(await path.evaluate((element, first) => element === first, original)).toBe(true);
        expect(await page.evaluate(() => window.settingsRequests.length)).toBe(writes);
        await expect(page.locator(".settings-workspace").getByRole("button", { name: t("Technical Logs"), exact: true })).toHaveCount(0);
        const bounds = (await page.locator("#main-content").boundingBox())!;
        const group = (await navigation.boundingBox())!;
        expect(group.x + group.width).toBeLessThanOrEqual(bounds.x + bounds.width);
        await page.screenshot({ path: testInfo.outputPath(`settings-${locale}.png`), animations: "disabled" });
    });
}

test("changing settings groups keeps field instances and serializes edits made before leaving", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&h5-settings=1");
    const navigation = page.getByRole("navigation", { name: "Settings sections", exact: true });
    const path = page.getByRole("textbox", { name: "Default project location", exact: true });
    await expect(path).toHaveValue("C:\\Fixture");
    const original = await page.locator("#settings-projectDirectory").elementHandle();
    const value = "D:\\Projects after switching";
    await path.fill(value);
    await navigation.getByRole("button", { name: "Unity", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    await navigation.getByRole("button", { name: "General", exact: true }).click();
    await page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.h5Settings.commits.length)).toBe(2);
    expect(await page.evaluate(() => window.h5Settings.commits.map(item => Object.keys(item.update)))).toEqual([["paths"], ["packages"]]);
    await navigation.getByRole("button", { name: "General", exact: true }).click();
    await expect(path).toHaveValue(value);
    expect(await page.locator("#settings-projectDirectory").evaluate((element, first) => element === first, original)).toBe(true);
    await expect(page.getByRole("textbox", { name: "Default backup location", exact: true })).toHaveValue("C:\\Fixture\\Backups");
});

test("Settings groups remain keyboard reachable in an 800px desktop window", async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 800 });
    await page.goto("/browser-harness.html?route=/settings&state=ready");
    const navigation = page.getByRole("navigation", { name: "Settings sections", exact: true });
    await navigation.getByRole("button", { name: "General", exact: true }).focus();
    for (const name of ["Unity", "Appearance", "Notifications"]) {
        await page.keyboard.press("Tab");
        const button = navigation.getByRole("button", { name, exact: true });
        await expect(button).toBeFocused();
        await page.keyboard.press("Space");
        await expect(button).toHaveAttribute("aria-pressed", "true");
        const box = (await button.boundingBox())!;
        expect(box.x + box.width).toBeLessThanOrEqual(800);
    }
    await expect(page.locator(".settings-workspace")).toHaveText("", { useInnerText: true });
    await page.keyboard.press("Shift+Tab");
    await page.keyboard.press("Space");
    await expect(page.getByRole("form", { name: "Theme", exact: true })).toBeVisible();
});
