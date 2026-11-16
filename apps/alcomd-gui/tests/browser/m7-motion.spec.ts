import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";

for (const preference of ["system", "full", "reduced"] as const) {
    for (const system of ["reduce", "no-preference"] as const) {
        test(`${preference} animation preference with system ${system} reaches Material and scrollbars`, async ({ page }) => {
            await page.emulateMedia({ reducedMotion: system });
            await openSettings(page, `&motion=${preference}`);
            const reduced = preference === "reduced" || (preference === "system" && system === "reduce");
            await expect(page.locator("html")).toHaveAttribute("data-motion", preference);
            await expect(page.locator("html")).toHaveAttribute("data-reduced-motion", String(reduced));
            await expect(page.locator("#settings-motion")).toHaveJSProperty("value", preference);
            await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", reduced);
            // Use real existing elements in light and shadow DOM, without injected test CSS.
            const scrollbar = page.locator(".main-content .os-scrollbar").first();
            // Scrollbars initialize after a rendering opportunity, independently of the controls above.
            await expect(scrollbar).toHaveCSS("transition-property", "opacity, visibility, top, right, bottom, left");
            const duration = expect.poll(async () => maxDuration(await scrollbar.evaluate((element) => getComputedStyle(element).transitionDuration)));
            if (reduced) await duration.toBeLessThan(0.001);
            else await duration.toBeGreaterThan(0.01);
            const checkbox = page.locator("md-checkbox").first();
            expect(await checkbox.evaluate((element) => {
                const sheet = element.shadowRoot!.querySelector<HTMLStyleElement>("style[data-alcomd-motion]")!;
                return sheet.media;
            })).toBe(reduced ? "all" : "not all");
            const theme = page.getByRole("form", { name: "Theme", exact: true });
            await theme.getByRole("textbox", { name: "Source color", exact: true }).fill("#315DA8");
            await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
            await page.getByRole("region", { name: "Theme", exact: true }).getByRole("button", { name: "Restore defaults", exact: true }).click();
            await expect(page.locator("md-dialog[open]")).toHaveJSProperty("quick", reduced);
            await expect(page.getByRole("dialog", { name: "Restore defaults", exact: true })).toBeVisible();
            const menu = page.locator("#settings-motion").locator("md-menu");
            await expect(menu).toHaveJSProperty("quick", reduced);
            await page.keyboard.press("Escape");
            await expect(page.getByRole("dialog", { name: "Restore defaults", exact: true })).toBeHidden();
            await expect(theme.getByRole("textbox", { name: "Source color", exact: true })).toHaveValue("#315DA8");
            expect(await page.evaluate(() => window.settingsRequests)).toHaveLength(1);
        });
    }
}

test("system follows live OS changes while explicit preferences stay fixed", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await openSettings(page);
    await emulateSystemMotion(page, "reduce");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", true);
    await emulateSystemMotion(page, "no-preference");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", false);
    for (const [value, quick] of [["full", false], ["reduced", true]] as const) {
        await selectOption(page, value);
        await expect(page.locator("html")).toHaveAttribute("data-motion", value);
        for (const system of ["reduce", "no-preference"] as const) {
            await emulateSystemMotion(page, system);
            await expect(page.locator("#settings-motion")).toHaveJSProperty("quick", quick);
        }
    }
    await selectOption(page, "system");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update.appearance?.motion))).toEqual(["full", "reduced", "system"]);
});

test("a failed animation write retains the latest control intent without claiming an accepted preference", async ({ page }) => {
    await openSettings(page, "&theme-save=error");
    await selectOption(page, "reduced");
    await expect(page.getByRole("button", { name: "Retry saving settings", exact: true })).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "reduced");
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
});

test("Chinese animation choices use the requested labels", async ({ page }) => {
    await openSettings(page);
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "system");
    await expect(page.locator("#settings-motion md-select-option")).toHaveText(["System", "Enable animations", "Disable animations"]);
    await page.locator("#settings-locale").evaluate((element) => {
        (element as HTMLElement & { select(value: string): void }).select("zh-CN");
        element.dispatchEvent(new Event("change", { bubbles: true }));
    });
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.getByRole("combobox", { name: "界面动画", exact: true })).toBeVisible();
    await page.getByRole("combobox", { name: "界面动画", exact: true }).click();
    await expect(page.locator("#settings-motion md-select-option")).toHaveText(["跟随系统", "启用动画", "关闭动画"]);
    await page.keyboard.press("Escape");
});

async function openSettings(page: Page, suffix = "") {
    await page.goto(`/browser-harness.html?route=/settings${suffix}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await selectSettingsSection(page, "appearance");
    await expect(page.locator("#settings-motion")).toBeVisible();
}

async function selectOption(page: Page, value: string) {
    const select = page.locator("#settings-motion");
    const lifecycle = await select.evaluateHandle((element) => ({
        opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
        closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
    }));
    try {
        await select.getByRole("combobox", { name: "GUI animation", exact: true }).click();
        await lifecycle.evaluate((events) => events.opened);
        const label = { full: "Enable animations", reduced: "Disable animations", system: "System" }[value]!;
        await select.getByRole("option", { name: label, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
    await expect(select).toHaveJSProperty("value", value);
}

function maxDuration(value: string) {
    return Math.max(...value.split(",").map((duration) => duration.trim().endsWith("ms") ? parseFloat(duration) / 1000 : parseFloat(duration)));
}

async function emulateSystemMotion(page: Page, reducedMotion: "reduce" | "no-preference") {
    const expected = reducedMotion === "reduce";
    const input = await page.evaluateHandle(() => {
        const media = window.matchMedia("(prefers-reduced-motion: reduce)");
        let delivered = media.matches;
        const changed = (event: MediaQueryListEvent) => { delivered = event.matches; };
        media.addEventListener("change", changed);
        return { get matches() { return media.matches; }, get delivered() { return delivered; }, dispose() { media.removeEventListener("change", changed); } };
    });
    try {
        await page.emulateMedia({ reducedMotion });
        await expect.poll(() => input.evaluate((state) => state.matches)).toBe(expected);
        await expect.poll(() => input.evaluate((state) => state.delivered)).toBe(expected);
    } catch (error) {
        console.log("MOTION_INPUT_STATE", JSON.stringify(await input.evaluate((state) => ({ matches: state.matches, delivered: state.delivered }))));
        throw error;
    } finally {
        await input.evaluate((state) => state.dispose());
        await input.dispose();
    }
}

test.afterEach(async ({ page }, info) => {
    if (info.status === info.expectedStatus || page.isClosed()) return;
    const evidence = await page.evaluate(() => {
        const control = document.querySelector("#settings-motion") as (HTMLElement & { quick?: boolean; value?: string }) | null;
        return { matches: window.matchMedia("(prefers-reduced-motion: reduce)").matches,
            visibility: document.visibilityState, focused: document.hasFocus(),
            preference: document.documentElement.dataset.motion, reduced: document.documentElement.dataset.reducedMotion,
            quick: control?.quick, value: control?.value };
    }).catch(() => null);
    console.log("MOTION_FAILURE_STATE", JSON.stringify(evidence));
});
