import { expect, test } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";

test("a missing project is grey in the list and becomes normal after a fresh Core read without changing row height", async ({ page }, testInfo) => {
    await page.goto("/browser-harness.html?route=/projects&h3=1&windows-paths=1&project-directory-state=missing");
    const row = page.locator(".projects-list tbody tr").first();
    await expect(row).toHaveClass(/project-row--missing/);
    await expect(row.locator("td").first()).toHaveCSS("opacity", "0.5");
    await expect(row.getByText("Project folder is missing.", { exact: true })).toHaveCount(1);
    await expect(row.getByRole("button", { name: "More actions for Sample", exact: true })).toBeEnabled();
    const height = (await row.boundingBox())!.height;
    await page.screenshot({ path: testInfo.outputPath("missing-project-list.png"), animations: "disabled" });
    await page.evaluate(() => { window.omitProjectDirectoryState = true; });
    await row.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(row.getByRole("button", { name: "Remove from favorites", exact: true })).toBeVisible();
    await expect(row).toHaveClass(/project-row--missing/);
    await page.evaluate(() => { window.omitProjectDirectoryState = false; window.projectDirectoryState = "available"; });
    await page.getByRole("searchbox", { name: "Search projects", exact: true }).fill("Sample");
    await expect(row).not.toHaveClass(/project-row--missing/, { timeout: 10_000 });
    await expect(row.getByText("Project folder is missing.", { exact: true })).toHaveCount(0);
    expect((await row.boundingBox())!.height).toBe(height);
});

test("cards grey only a confirmed missing directory and retain unavailable or unobserved status", async ({ page }, testInfo) => {
    await page.goto("/browser-harness.html?route=/projects&h3=1&windows-paths=1&project-directory-state=missing");
    await page.getByRole("button", { name: "List view", exact: true }).click();
    const card = page.locator(".project-card").first();
    await expect(card).toHaveClass(/project-card--missing/);
    await expect(card.locator(".project-card-identity")).toHaveCSS("opacity", "0.5");
    await expect(card.getByRole("button", { name: "More actions for Sample", exact: true })).toBeEnabled();
    await page.screenshot({ path: testInfo.outputPath("missing-project-card.png"), animations: "disabled" });
    await page.goto("/browser-harness.html?route=/projects&h3=1&windows-paths=1&project-directory-state=unavailable");
    await expect(card).not.toHaveClass(/project-card--missing/);
    await expect(card.getByText("The project folder is unavailable.", { exact: true })).toHaveCount(1);
    await page.goto("/browser-harness.html?route=/projects&h3=1&windows-paths=1");
    await expect(card).not.toHaveClass(/project-card--missing/);
    await expect(card.getByText("Project folder is missing.", { exact: true })).toHaveCount(0);
    await expect(card.getByText("The project folder is unavailable.", { exact: true })).toHaveCount(0);
});

for (const scenario of [
    { locale: "en-US", more: "More actions for Sample", copy: "Copy Project", review: "Review copy", message: "The source project folder does not exist. Check the source project's path before copying." },
    { locale: "zh-CN", more: "Sample 的更多操作", copy: "复制项目", review: "检查复制内容", message: "源项目文件夹不存在，请检查源项目路径后再复制。" },
    { locale: "ja-JP", more: "Sample のその他の操作", copy: "プロジェクトをコピー", review: "コピー内容を確認", message: "コピー元のプロジェクトフォルダーが存在しません。コピー元のパスを確認してください。" }
]) {
    test(`${scenario.locale} copy reports a missing source before its action buttons`, async ({ page }, testInfo) => {
        await page.goto(`/browser-harness.html?route=/projects&h3=1&windows-paths=1&project-directory-state=missing&copy-source-missing=1&theme-mode=dark&locale=${scenario.locale}`);
        await selectActionMenuItem(page.getByRole("button", { name: scenario.more, exact: true }), page.getByRole("menuitem", { name: scenario.copy, exact: true }));
        const dialog = page.locator("md-dialog[open]");
        await dialog.getByRole("button", { name: scenario.review, exact: true }).click();
        const alert = dialog.getByRole("alert");
        await expect(alert).toContainText(scenario.message);
        await expect(alert).toContainText("project_copy_source_missing");
        const alertBounds = (await alert.boundingBox())!;
        const actionBounds = (await dialog.locator(".dialog-actions").boundingBox())!;
        expect(alertBounds.y + alertBounds.height).toBeLessThanOrEqual(actionBounds.y);
        await expect(dialog.getByRole("button", { name: scenario.review, exact: true })).toBeEnabled();
        await page.screenshot({ path: testInfo.outputPath(`missing-copy-${scenario.locale}.png`), animations: "disabled" });
    });
}

for (const scenario of [
    { locale: "en-US", more: "More actions for Sample", remove: "Remove Project", delete: "Delete Project Directory…", cancel: "Cancel", message: "The project folder does not exist. Check the project's path before deleting." },
    { locale: "zh-CN", more: "Sample 的更多操作", remove: "删除项目", delete: "删除项目目录…", cancel: "取消", message: "项目文件夹不存在，请检查项目路径后再删除。" },
    { locale: "ja-JP", more: "Sample のその他の操作", remove: "プロジェクトを削除", delete: "プロジェクトフォルダーを削除…", cancel: "キャンセル", message: "プロジェクトフォルダーが存在しません。削除するプロジェクトのパスを確認してください。" }
]) {
    test(`${scenario.locale} deletion identifies a missing directory before its action buttons`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects&h3=1&windows-paths=1&project-directory-state=missing&delete-source-missing=1&locale=${scenario.locale}`);
        await selectActionMenuItem(page.getByRole("button", { name: scenario.more, exact: true }), page.getByRole("menuitem", { name: scenario.remove, exact: true }));
        const dialog = page.locator("md-dialog[open]");
        await dialog.getByRole("button", { name: scenario.delete, exact: true }).click();
        const alert = dialog.getByRole("alert");
        await expect(alert).toContainText(scenario.message);
        await expect(alert).toContainText("project_delete_source_missing");
        const positions = await dialog.evaluate((element) => {
            const feedback = element.querySelector<HTMLElement>("[role=alert]")!.getBoundingClientRect();
            const actions = element.querySelector<HTMLElement>(".dialog-actions")!.getBoundingClientRect();
            return { feedbackBottom: feedback.bottom, actionsTop: actions.top };
        });
        expect(positions.feedbackBottom).toBeLessThanOrEqual(positions.actionsTop);
        await expect(dialog.getByRole("button", { name: scenario.cancel, exact: true })).toBeEnabled();
        await expect(dialog.getByRole("textbox")).toHaveCount(0);
        await expect(page.locator(".projects-list tbody tr")).toHaveCount(1);
    });
}
