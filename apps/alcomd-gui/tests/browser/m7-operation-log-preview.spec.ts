import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("single log view shows business steps and actual details without paging controls", async ({ page }) => {
    await page.goto("/browser-harness.html?operation-log-preview=1");
    await expect(page.getByRole("heading", { name: "日志", exact: true })).toBeVisible();
    await expect(page.locator("tbody tr")).toHaveCount(4);
    await expect(page.getByRole("tab")).toHaveCount(0);
    await expect(page.getByRole("button")).toHaveCount(1);
    await page.getByRole("button", { name: "详情", exact: true }).click();
    await expect(page.locator("pre")).toContainText("C:\\ALCOMD-Test\\项目A\\Packages\\vpm-manifest.json");
    await expect(page.locator("pre")).toContainText("access denied (os error 5)");
    await page.screenshot({ path: "../../output/m7-operation-log-preview-20261106.png" });
});

test("long paths and causes stay complete and wrap inside the desktop list", async ({ page }) => {
    await page.goto("/browser-harness.html?operation-log-preview=1");
    const longPath = "C:\\" + "中文项目目录\\".repeat(65) + "manifest.json";
    const reason = "Unable to read " + longPath + "\ncaused by: " + "long reason ".repeat(150);
    await page.evaluate(({ path, details }) => window.logPreview.replace([{
        id: "long-path", actor: "CLI", occurredAtMs: 1793937600000, operation: "添加项目", step: "检查项目", outcome: "failed",
        target: { name: "项目A", path }, details
    }]), { path: longPath, details: reason });
    await page.getByRole("button", { name: "详情", exact: true }).click();
    await expect(page.locator("pre")).toHaveText(reason);
    await expect(page.locator("small")).toHaveText(longPath);
    expect(await page.locator(".desktop-list-viewport").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
    expect(await page.locator("pre").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});

test("read failure leaves loaded records alone and only replaces an unavailable list", async ({ page }) => {
    await page.goto("/browser-harness.html?operation-log-preview=1");
    await page.evaluate(() => window.logPreview.fail("无法读取日志；已显示的内容可能不是最新。"));
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(page.locator("tbody tr")).toHaveCount(4);
    await page.evaluate(() => { window.logPreview.replace([]); window.logPreview.fail("无法读取日志"); });
    await expect(page.getByRole("alert")).toContainText("无法读取日志");
    await expect(page.getByText("暂无日志", { exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "重试", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.logPreview.retries)).toBe(1);
});
