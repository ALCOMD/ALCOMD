import { expect, test } from "@playwright/test";

for (const [theme, width] of [["light", 1280], ["dark", 1280], ["light", 640], ["dark", 640]] as const) {
    test(`Projects loading does not paint a square first record in ${theme} at ${width}px`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height: 800 });
        await page.goto(`/browser-harness.html?route=/repositories&projectGrid=1&locale=zh-CN&theme-mode=${theme}&holdProjectsList=1`);
        await expect(page.locator("html")).toHaveAttribute("data-appearance", theme);
        await page.locator("#primary-navigation md-list-item").filter({ hasText: "项目" }).click();
        const table = page.locator(".projects-list");
        const placeholder = table.locator(".desktop-list-empty");
        await expect(placeholder).toBeVisible();
        await expect(placeholder).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
        await expect(placeholder.locator("..")).toHaveCSS("height", "4px");
        const header = table.locator("thead");
        const before = (await header.boundingBox())!;
        await expect(table.locator("tbody tr[data-row-key]")).toHaveCount(0);
        await expect(placeholder.getByRole("progressbar")).toBeVisible();
        await page.screenshot({ path: testInfo.outputPath(`project-loading-${theme}-${width}.png`), animations: "disabled" });
        await page.evaluate(() => {
            const samples: string[] = [];
            Object.assign(window, { projectCornerFrames: samples });
            let remaining = 12;
            function record() {
                const cell = document.querySelector(".projects-list tbody tr[data-row-key] td");
                if (cell) { samples.push(getComputedStyle(cell).borderTopLeftRadius); remaining--; }
                if (remaining > 0) requestAnimationFrame(record);
            }
            requestAnimationFrame(record);
            window.resumeProjectsList?.();
        });
        await expect(table.locator("tbody tr[data-row-key]")).toHaveCount(7);
        await expect(placeholder).toHaveCount(0);
        await expect.poll(() => page.evaluate(() => (window as unknown as { projectCornerFrames: string[] }).projectCornerFrames.length)).toBe(12);
        expect(await page.evaluate(() => [...new Set((window as unknown as { projectCornerFrames: string[] }).projectCornerFrames)])).toEqual(["8px"]);
        const after = (await header.boundingBox())!;
        expect(after.y).toBe(before.y);
        expect(after.height).toBe(before.height);
        await expect(table.locator("tbody tr[data-row-key]").first().locator("td").last()).toHaveCSS("border-top-right-radius", "8px");
        await page.screenshot({ path: testInfo.outputPath(`project-records-${theme}-${width}.png`), animations: "disabled" });
        // Returning to a prepared page retains the same row shape without the placeholder block.
        await page.locator("#primary-navigation md-list-item").filter({ hasText: "资源" }).click();
        await page.locator("#primary-navigation md-list-item").filter({ hasText: "项目" }).click();
        await expect(table.locator("tbody tr[data-row-key]")).toHaveCount(7);
        await expect(table.locator("tbody tr[data-row-key] td").first()).toHaveCSS("border-top-left-radius", "8px");
    });
}
