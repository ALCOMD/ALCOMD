import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";

async function startPackageTask(page: Page) {
    await page.goto(`/browser-harness.html?route=/projects/${PROJECT_ID}&hold-core-operation=1`);
    await page.getByRole("button", { name: /^Update All \(\d+\)$/ }).click();
    const review = page.locator('md-dialog[open][data-aria-label="Apply package changes?"]');
    await expect(review).toBeVisible();
    await review.getByRole("button", { name: "Apply changes", exact: true }).click();
    const progress = page.locator('md-dialog[open][data-aria-label="Package changes"]');
    await expect(progress).toBeVisible();
    await expect(progress.getByRole("status")).toContainText("running");
    return progress;
}

for (const state of ["succeeded", "failed", "cancelled"] as const) {
    test(`package confirmation hands off to progress and retains ${state} until dismissed`, async ({ page }) => {
        const progress = await startPackageTask(page);
        await expect(progress.getByRole("button", { name: "Minimize", exact: true })).toBeVisible();
        await expect(progress.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
        await page.evaluate(({ id, state }) => window.completeGuiTask(id, state), { id: OPERATION_ID, state });
        await expect(progress.getByRole("status")).toContainText(state);
        await expect(progress.getByRole("status")).not.toContainText("extracting");
        if (state === "succeeded") await expect(progress.getByText("Task completed.", { exact: true })).toBeVisible();
        if (state === "failed") await expect(progress.getByRole("alert")).toContainText("fixture-task-diagnostic");
        await expect(progress.getByRole("button", { name: "Minimize", exact: true })).toHaveCount(0);
        await progress.getByRole("button", { name: "Close", exact: true }).click();
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await expect(page).toHaveURL(new RegExp(`/projects/${PROJECT_ID}$`));
        expect(await page.evaluate(() => window.packageRequests.filter(({ method }) => method === "apply"))).toHaveLength(1);
    });
}

for (const dismiss of ["Minimize", "Escape", "View operation"]) {
    test(`${dismiss} keeps the task running and completion does not reopen the window`, async ({ page }) => {
        const progress = await startPackageTask(page);
        if (dismiss === "Escape") await page.keyboard.press("Escape");
        else await progress.getByRole("button", { name: dismiss, exact: true }).click();
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await page.getByRole("navigation", { name: "Utilities", exact: true }).locator("md-list-item").filter({ hasText: /^Tasks/ }).click();
        await expect(page).toHaveURL(/\/operations$/);
        await expect(page.getByRole("row").filter({ hasText: "Change packages" })).toContainText("running");
        await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), OPERATION_ID);
        await expect(page.getByRole("row").filter({ hasText: "Change packages" })).toContainText("succeeded");
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await expect(page).toHaveURL(/\/operations$/);
    });
}

test("a progress read failure retries the accepted task without creating it twice", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${PROJECT_ID}&hold-core-operation=1`);
    await page.evaluate(() => { window.taskReadError = "daemon_unavailable"; });
    await page.getByRole("button", { name: /^Update All \(\d+\)$/ }).click();
    await page.locator('md-dialog[open][data-aria-label="Apply package changes?"]').getByRole("button", { name: "Apply changes", exact: true }).click();
    const progress = page.locator('md-dialog[open][data-aria-label="Package changes"]');
    await expect(progress.getByRole("status")).toContainText("temporarily unavailable");
    await page.evaluate(() => { window.taskReadError = undefined; });
    await progress.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(progress.getByRole("status")).toContainText("running");
    expect(await page.evaluate(() => window.packageRequests.filter(({ method }) => method === "apply"))).toHaveLength(1);
});

test("task progress retains the existing cooperative cancellation flow", async ({ page }) => {
    const progress = await startPackageTask(page);
    await progress.getByRole("button", { name: "Cancel operation", exact: true }).click();
    await page.locator('md-dialog[open][data-aria-label="Request cancellation?"]').getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(progress.getByRole("status")).toContainText("cancelling");
    await page.evaluate((id) => window.completeGuiTask(id, "cancelled"), OPERATION_ID);
    await expect(progress.getByRole("status")).toContainText("cancelled");
    await expect(progress.getByRole("button", { name: "Close", exact: true })).toBeVisible();
});

test("historical and externally accepted tasks never open a foreground window", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/operations&hold-core-operation=1");
    const id = await page.evaluate(async () => (await window.fixtureCore.stateCheck()).operationId);
    await expect(page.getByRole("row").filter({ hasText: "Check state" })).toContainText("running");
    await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), id);
    await expect(page.getByRole("row").filter({ hasText: "Check state" })).toContainText("succeeded");
    await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("minimized project creation completes without navigating away or reopening", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&hold-core-operation=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Create project name", { exact: true }).fill("Background project");
    await dialog.getByRole("button", { name: "Create project", exact: true }).click();
    await dialog.getByRole("button", { name: "Minimize", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    const operationId = await page.evaluate(async () => (await window.fixtureCore.operationsList()).operations[0]!.operationId);
    await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), operationId);
    await page.getByRole("navigation", { name: "Utilities", exact: true }).locator("md-list-item").filter({ hasText: /^Tasks/ }).click();
    await expect(page.getByRole("row").filter({ hasText: "Create project" })).toContainText("succeeded");
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(page).toHaveURL(/\/operations$/);
});

for (const [width, theme] of [[1280, "light"], [640, "dark"]] as const) {
    test(`Chinese task progress and completion fit ${width}px in ${theme} mode`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height: 800 });
        await page.goto(`/browser-harness.html?route=/projects/${PROJECT_ID}&hold-core-operation=1&locale=zh-CN&theme-mode=${theme}`);
        await page.getByRole("button", { name: /^更新全部（\d+）$/ }).click();
        await page.locator("md-dialog[open]").getByRole("button", { name: "应用更改", exact: true }).click();
        const progress = page.locator('md-dialog[open][data-aria-label="软件包变更"]');
        await expect(progress.getByRole("button", { name: "最小化", exact: true })).toBeVisible();
        await expect(progress.getByRole("status")).not.toContainText("Reading progress");
        await page.screenshot({ path: testInfo.outputPath(`task-progress-${width}-${theme}.png`), animations: "disabled" });
        await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), OPERATION_ID);
        await expect(progress.getByText("任务已完成。", { exact: true })).toBeVisible();
        await expect(progress.getByRole("status")).not.toContainText("正在解压");
        const dialogBox = await progress.getByRole("dialog").boundingBox();
        for (const action of await progress.getByRole("button").all()) {
            const bounds = await action.boundingBox();
            expect(bounds!.x).toBeGreaterThanOrEqual(dialogBox!.x);
            expect(bounds!.x + bounds!.width).toBeLessThanOrEqual(dialogBox!.x + dialogBox!.width);
        }
        await page.screenshot({ path: testInfo.outputPath(`task-complete-${width}-${theme}.png`), animations: "disabled" });
    });
}
