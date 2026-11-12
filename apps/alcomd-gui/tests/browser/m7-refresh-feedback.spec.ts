import { expect, test } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

test("instant refresh paints a real circle even though its result is already available", async ({ page }) => {
    await page.goto("/browser-harness.html?refresh-feedback=1");
    await expect(page.getByTestId("immediate-refresh")).toBeVisible();
    const firstPaint = await page.getByTestId("immediate-refresh").evaluate(async (element) => {
        const before = element.getBoundingClientRect();
        (element as HTMLElement).click();
        return new Promise<{ busy: string | null; circle: boolean; completed: string | null; width: number; beforeWidth: number }>((resolve) => {
            requestAnimationFrame(() => resolve({
                busy: element.getAttribute("aria-busy"),
                circle: element.querySelector("md-circular-progress") !== null,
                completed: document.querySelector('[data-testid="refresh-completed"]')?.textContent ?? null,
                width: element.getBoundingClientRect().width,
                beforeWidth: before.width
            }));
        });
    });
    expect(firstPaint).toMatchObject({ busy: "true", circle: true, completed: "1" });
    expect(firstPaint.width).toBe(firstPaint.beforeWidth);
    await expect(page.getByTestId("immediate-refresh")).toHaveJSProperty("ariaBusy", "false");
    await expect(page.getByTestId("immediate-refresh").locator('md-icon[data-icon-name="refresh"]')).toBeVisible();
});

test("text refresh retains its width and accessible name while showing completion feedback", async ({ page }) => {
    await page.goto("/browser-harness.html?refresh-feedback=1");
    await expect(page.getByTestId("text-refresh")).toBeVisible();
    const evidence = await page.getByTestId("text-refresh").evaluate(async (element) => {
        const width = element.getBoundingClientRect().width;
        (element as HTMLElement).click();
        return new Promise<{ busy: string | null; width: number; originalWidth: number; circle: boolean; completed: string | null }>((resolve) => requestAnimationFrame(() => resolve({
            busy: element.getAttribute("aria-busy"), width: element.getBoundingClientRect().width, originalWidth: width,
            circle: element.querySelector("md-circular-progress") !== null,
            completed: document.querySelector('[data-testid="refresh-completed"]')?.textContent ?? null
        })));
    });
    expect(evidence).toMatchObject({ busy: "true", circle: true, completed: "1" });
    expect(evidence.width).toBe(evidence.originalWidth);
    await expect(page.getByRole("button", { name: "Discover and refresh", exact: true })).toBeVisible();
    await expect(page.getByTestId("text-refresh").locator("button")).toBeEnabled();
});

test("slow refresh stays busy and repeated activation cannot start a second request", async ({ page }, testInfo) => {
    await page.goto("/browser-harness.html?refresh-feedback=1");
    const button = page.getByTestId("held-refresh");
    await button.click();
    await page.waitForTimeout(360);
    await expect(button.locator("md-circular-progress")).toBeVisible();
    await button.evaluate((element) => {
        element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await expect(page.getByTestId("refresh-calls")).toHaveText("1");
    await expect(button.locator("button")).toBeDisabled();
    await page.screenshot({ path: testInfo.outputPath("refresh-circle.png") });
    await page.getByRole("button", { name: "Complete refresh", exact: true }).click();
    await expect(button).toHaveJSProperty("ariaBusy", "false");
    await expect(page.getByTestId("refresh-completed")).toHaveText("1");
});

test("controlled refresh remains busy after the minimum feedback window", async ({ page }) => {
    await page.goto("/browser-harness.html?refresh-feedback=1");
    const button = page.getByTestId("controlled-refresh");
    await button.click();
    await page.waitForTimeout(400);
    await expect(button.locator("md-circular-progress")).toBeVisible();
    await expect(button.locator("button")).toBeDisabled();
    await expect(page.getByTestId("refresh-completed")).toHaveText("1");
    await expect(button).toHaveJSProperty("ariaBusy", "false");
});

test("refresh failures appear immediately and the button becomes usable again", async ({ page }) => {
    await page.goto("/browser-harness.html?refresh-feedback=1");
    const evidence = await page.getByTestId("failed-refresh").evaluate(async (element) => {
        (element as HTMLElement).click();
        return new Promise<{ circle: boolean; error: string | null }>((resolve) => {
            const inspect = () => {
                const error = document.querySelector('[role="alert"]')?.textContent;
                if (error === undefined) requestAnimationFrame(inspect);
                else resolve({ circle: element.querySelector("md-circular-progress") !== null, error });
            };
            requestAnimationFrame(inspect);
        });
    });
    expect(evidence).toEqual({ circle: true, error: "Refresh failed" });
    await expect(page.getByTestId("failed-refresh").locator("button")).toBeEnabled();
});

test("disabled refresh does nothing and an unmounted request cannot keep a new control busy", async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto("/browser-harness.html?refresh-feedback=1");
    await page.getByTestId("disabled-refresh").evaluate((element) => element.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await expect(page.getByTestId("refresh-calls")).toHaveText("0");
    await page.getByTestId("held-refresh").click();
    await page.getByRole("button", { name: "Toggle refresh control", exact: true }).click();
    await expect(page.getByTestId("held-refresh")).toHaveCount(0);
    await page.getByRole("button", { name: "Toggle refresh control", exact: true }).click();
    await page.getByRole("button", { name: "Complete refresh", exact: true }).click();
    await expect(page.getByTestId("held-refresh")).toHaveJSProperty("ariaBusy", "false");
    expect(errors).toEqual([]);
});

test("keyboard refresh still gives feedback with reduced motion", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/browser-harness.html?refresh-feedback=1");
    const button = page.getByTestId("held-refresh");
    await button.focus();
    await page.keyboard.press("Enter");
    await expect(button.locator("md-circular-progress")).toBeVisible();
    await expect(page.getByTestId("refresh-calls")).toHaveText("1");
});

test("a real Unity discovery refresh uses the circular feedback", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&unity-discovery-hold=1");
    await selectSettingsSection(page, "unity");
    const button = page.getByRole("region", { name: "Unity installations", exact: true }).locator("md-filled-button").filter({ hasText: "Discover and refresh" });
    await expect(button).toBeVisible();
    await button.click();
    await expect(button.locator("md-circular-progress")).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new Event("test-unity-discovery-ready")));
    await expect(button.locator("button")).toBeEnabled();
});

test("extension-owned refresh actions receive feedback without interpreting their action IDs", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/extensions/com.vrclearn.mcp-management/ui");
    const button = page.getByRole("region", { name: "MCP Management" }).locator("md-filled-tonal-button").filter({ hasText: "Refresh" });
    await expect(button).toBeVisible();
    const evidence = await button.evaluate(async (element) => {
        (element as HTMLElement).click();
        return new Promise<boolean>((resolve) => requestAnimationFrame(() => resolve(element.querySelector("md-circular-progress") !== null)));
    });
    expect(evidence).toBe(true);
    await expect(button.locator("button")).toBeEnabled();
});

test("permission refresh keeps circular feedback when the inline error action is replaced", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/extensions/com.vrclearn.discord&permission-refresh-error=1");
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const host = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Extension permissions", exact: true }) });
    const retry = host.locator('[role="alert"] md-text-button').filter({ hasText: "Refresh" });
    await expect(retry.locator("button")).toBeEnabled();
    await page.evaluate(() => window.dispatchEvent(new Event("test-permission-read-ready")));
    const circle = await retry.evaluate(async (element) => {
        const dialog = element.closest("md-dialog")!;
        (element as HTMLElement).click();
        return new Promise<boolean>((resolve) => requestAnimationFrame(() => resolve(dialog.querySelector(".dialog-actions md-circular-progress") !== null)));
    });
    expect(circle).toBe(true);
    await expect(host.locator('[role="alert"]')).toHaveCount(0);
    await expect(host.locator(".dialog-actions md-text-button").filter({ hasText: "Refresh" }).locator("button")).toBeEnabled();
});

for (const colorScheme of ["light", "dark"] as const) {
    test(`busy refresh circles remain legible in ${colorScheme} mode`, async ({ page }, testInfo) => {
        await page.emulateMedia({ colorScheme });
        await page.goto("/browser-harness.html?refresh-feedback=1");
        for (const id of ["held-refresh", "held-text-refresh"]) {
            const button = page.getByTestId(id);
            await button.click();
            const progress = button.locator("md-circular-progress");
            await expect(progress).toBeVisible();
            await page.waitForTimeout(360);
            const opacity = await progress.evaluate((element) => {
                let opacity = 1;
                let node: Element | null = element;
                while (node !== null) {
                    opacity *= Number(getComputedStyle(node).opacity);
                    node = node.assignedSlot ?? node.parentElement ?? (node.getRootNode() as ShadowRoot).host ?? null;
                }
                return opacity;
            });
            expect(opacity).toBeGreaterThan(0.9);
            await page.screenshot({ path: testInfo.outputPath(`${id}-${colorScheme}.png`) });
            await page.getByRole("button", { name: "Complete refresh", exact: true }).click();
            await expect(button.locator("button")).toBeEnabled();
        }
    });
}
