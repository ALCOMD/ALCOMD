import { expect, test, type Page } from "@playwright/test";

declare global {
    interface Window { routeAnimations: Animation[] }
}

test.use({ viewport: { width: 1440, height: 900 } });
test.beforeEach(async ({ page }) => { await page.emulateMedia({ reducedMotion: "no-preference" }); });

async function open(page: Page) {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready");
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.locator(".route-content").evaluate((element) => {
        window.routeAnimations = [];
        const animate = element.animate.bind(element);
        element.animate = (frames, options) => {
            const animation = animate(frames, options);
            // Freeze the real animation so rapid navigation and cancellation are deterministic.
            animation.pause();
            window.routeAnimations.push(animation);
            return animation;
        };
    });
}

function navigation(page: Page, name: string) {
    return page.locator("#primary-navigation .navigation-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) })
    });
}

test("pointer route transitions keep one live page and cancel superseded motion", async ({ page }) => {
    await open(page);
    const content = await page.locator(".route-content").elementHandle();
    await navigation(page, "Resources").click();
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeFocused();
    await page.getByRole("button", { name: "User Packages", exact: true }).click();
    await expect(page.getByRole("heading", { name: "No User Packages enrolled", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    await expect(page.getByRole("table", { name: "Templates", exact: true })).toBeVisible();
    await navigation(page, "Projects").click();
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    expect(await content!.evaluate((element) => element === document.querySelector(".route-content"))).toBe(true);
    expect(await page.evaluate(() => window.routeAnimations.map((animation) => animation.playState))).toEqual(["idle", "idle", "idle", "paused"]);
    expect(await page.evaluate(() => {
        const effect = window.routeAnimations.at(-1)!.effect as KeyframeEffect;
        return { duration: effect.getTiming().duration, opacity: effect.getKeyframes().map((frame) => frame.opacity), transforms: effect.getKeyframes().some((frame) => "transform" in frame) };
    })).toEqual({ duration: 180, opacity: ["0.65", "1"], transforms: false });
    await expect(page.locator("#main-content")).toHaveCSS("opacity", "1");
    await expect(page.locator(".route-content > .projects-page")).toHaveCount(1);
    // Clicking the current page must not restart motion or recreate its content.
    await navigation(page, "Projects").click();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(4);
    await page.evaluate(() => window.routeAnimations.at(-1)!.finish());
    await expect(page.locator(".route-content")).toHaveCSS("opacity", "1");
    await expect(page.getByRole("button", { name: "Register project", exact: true })).toBeEnabled();
});

test("keyboard navigation and reduced motion bypass or cancel page animation", async ({ page }) => {
    await open(page);
    await navigation(page, "Resources").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { name: "Resources", exact: true })).toBeFocused();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(0);
    await navigation(page, "Projects").click();
    await page.keyboard.press("Tab");
    expect(await page.evaluate(() => window.routeAnimations[0]!.playState)).toBe("idle");
    await navigation(page, "Resources").click();
    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect.poll(() => page.evaluate(() => window.routeAnimations.at(-1)!.playState)).toBe("idle");
    await navigation(page, "Projects").click();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(2);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await navigation(page, "Resources").click();
    await page.locator("html").evaluate((element) => { element.dataset.motion = "reduced"; });
    await expect.poll(() => page.evaluate(() => window.routeAnimations.at(-1)!.playState)).toBe("idle");
    await navigation(page, "Projects").click();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(3);
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeFocused();
});

test("cancelled dirty navigation preserves the draft without starting route motion", async ({ page }) => {
    await open(page);
    await navigation(page, "Settings").click();
    const compact = page.getByRole("checkbox", { name: "Compact GUI", exact: true });
    await compact.check();
    await navigation(page, "Projects").click();
    const discard = page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true });
    await expect(discard).toBeVisible();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(1);
    await page.locator("md-dialog").filter({ hasText: "Discard unsaved changes?" }).getByRole("button", { name: "Keep editing", exact: true }).click();
    await expect(compact).toBeChecked();
    expect(await page.evaluate(() => window.routeAnimations.length)).toBe(1);
});

test("filter entry is anchored, does not replay on input changes, and keeps Escape focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects%2F00000000-0000-4000-8000-000000000101%2Fpackages&state=package-multiple");
    const trigger = page.getByRole("button", { name: "Filter packages", exact: true });
    await expect(trigger).toBeVisible();
    await page.locator(".alcomd-filter-popover").evaluate((element) => {
        element.setAttribute("data-transition-runs", "0");
        element.addEventListener("transitionrun", () => {
            element.setAttribute("data-transition-runs", String(Number(element.getAttribute("data-transition-runs")) + 1));
        });
    });
    await trigger.click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await expect(panel).toBeVisible();
    await expect(panel).toHaveAttribute("data-transition-runs", "2");
    await expect.poll(() => panel.evaluate((element) => element.getAnimations().length)).toBe(0);
    const box = (await panel.boundingBox())!;
    const anchor = (await page.locator(".package-filter-trigger").boundingBox())!;
    const expectedTop = Math.max(8, Math.min(anchor.y + anchor.height + 4, 900 - box.height - 8));
    expect(Math.abs(box.y - expectedTop)).toBeLessThan(1);
    await page.keyboard.press("Tab");
    await panel.click({ position: { x: box.width / 2, y: 12 } });
    await expect(panel).toHaveAttribute("data-transition-runs", "2");
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("transition-duration", "0s");
    await expect(panel).toHaveCSS("transform", "none");
});
