import { expect, test, type Page } from "@playwright/test";

const locales = [
    {
        locale: "zh-CN", selfName: "简体中文", projects: "项目", settings: "设置", resources: "资源",
        create: "新建项目", openUnity: "打开 Unity", manage: "管理", cancel: "取消",
        language: "界面语言(Language)", projectName: "新建项目的名称", search: "搜索软件包",
        latest: "最新", updateAll: /^更新全部/, review: "应用软件包更改？"
    },
    {
        locale: "ja-JP", selfName: "日本語", projects: "プロジェクト", settings: "設定", resources: "パッケージ&テンプレート",
        create: "プロジェクトを作成", openUnity: "Unityを開く", manage: "管理", cancel: "キャンセル",
        language: "言語設定(Language)", projectName: "作成するプロジェクト名", search: "パッケージを検索",
        latest: "最新", updateAll: /^すべて更新/, review: "パッケージの変更を適用しますか？"
    }
] as const;

for (const language of locales) {
    test(`saved ${language.locale} localizes the shell, pages and reviews while preserving external text`, async ({ page }, testInfo) => {
        await page.goto("/browser-harness.html?route=%2Fsettings&state=package-multiple&versions=1&projectGrid=1");
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
        await chooseLanguage(page, language.selfName);
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        await expect(page.getByRole("heading", { level: 1, name: language.settings, exact: true })).toBeVisible();
        await expect(navigationItem(page, language.projects)).toBeVisible();
        await expect(navigationItem(page, language.resources)).toBeVisible();
        await expect(page.getByRole("combobox", { name: language.language, exact: true })).toBeVisible();
        expect(await page.evaluate(() => window.settingsWrites)).toEqual([{ expectedRevision: 7, revision: 8 }]);
        await page.screenshot({ path: testInfo.outputPath(`settings-${language.locale}.png`), animations: "disabled" });

        await navigationItem(page, language.resources).click();
        await expect(page.getByRole("heading", { level: 1, name: language.resources, exact: true })).toBeVisible();
        await expect(page.getByRole("table").getByText("Example packages", { exact: true })).toBeVisible();
        await navigationItem(page, language.projects).click();
        await expect(page.getByRole("heading", { level: 1, name: language.projects, exact: true })).toBeVisible();
        const project = page.locator(".projects-list tbody tr").filter({ hasText: "Avatar Studio" });
        await expect(project).toContainText("Avatar Studio");
        await expect(project).toContainText("C:\\Projects\\Workspace with a long directory name\\Avatar Studio");
        await expect(project.getByRole("button", { name: language.openUnity, exact: true })).toBeVisible();

        await page.getByRole("button", { name: language.create, exact: true }).click();
        await expect(page.getByRole("dialog", { name: language.create, exact: true })).toBeVisible();
        const creation = page.locator("md-dialog[open]");
        await expect(creation.getByRole("textbox", { name: language.projectName, exact: true })).toBeVisible();
        await creation.getByRole("button", { name: language.cancel, exact: true }).click();
        await expect(page.locator("md-dialog[open]")).toHaveCount(0);

        await project.getByRole("button", { name: language.manage, exact: true }).click();
        await expect(page.getByRole("searchbox", { name: language.search, exact: true })).toBeVisible();
        await expect(page.getByRole("columnheader", { name: language.latest, exact: true })).toBeVisible();
        const packageRow = page.getByRole("row").filter({ hasText: "Avatar tools" });
        await expect(packageRow).toContainText("Avatar tools");
        await expect(packageRow).toContainText("Example packages");
        await expect(packageRow).toContainText("1.10.0");
        await page.getByRole("button", { name: language.updateAll }).click();
        await expect(page.getByRole("dialog", { name: language.review, exact: true })).toBeVisible();
        const review = page.locator("md-dialog[open]");
        await expect(review).toBeVisible();
        await expect(review).toContainText("com.example.avatar");
        expect(await page.evaluate(() => window.packageRequests.some((request) => request.method === "apply"))).toBe(false);
        await page.screenshot({ path: testInfo.outputPath(`package-review-${language.locale}.png`), animations: "disabled" });
        await review.getByRole("button", { name: language.cancel, exact: true }).click();
        await expect(page.locator("md-dialog[open]")).toHaveCount(0);
        expect(await page.evaluate(() => window.packageRequests.some((request) => request.method === "apply"))).toBe(false);

        await navigationItem(page, language.settings).click();
        await expect(page.getByRole("heading", { level: 1, name: language.settings, exact: true })).toBeVisible();
        await expect(page.locator("#settings-locale")).toHaveJSProperty("value", language.locale);
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        // The harness creates a new in-memory client on reload. This proves saved settings
        // survive route changes, not real daemon persistence across browser/process reloads.
    });
}

test("changing language saves immediately and ordinary navigation needs no discard confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fsettings");
    await chooseLanguage(page, "简体中文");
    await expect(page.locator("#settings-locale")).toHaveJSProperty("value", "zh-CN");
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await navigationItem(page, "项目").click();
    await expect(page.getByRole("heading", { level: 1, name: "项目", exact: true })).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await navigationItem(page, "设置").click();
    await expect(page.locator("#settings-locale")).toHaveJSProperty("value", "zh-CN");
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([{ expectedRevision: 7, revision: 8 }]);
});

test("English remains the source-language baseline for shell and project creation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&projectGrid=1");
    await expect(page.locator("html")).toHaveAttribute("lang", "en-US");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    for (const name of ["Projects", "Resources", "Settings", "Logs"]) await expect(navigationItem(page, name)).toBeVisible();
    const first = page.locator(".projects-list tbody tr").filter({ hasText: "Avatar Studio" });
    await expect(first.getByRole("button", { name: "Open Unity", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Create project", exact: true })).toBeVisible();
    const creation = page.locator("md-dialog[open]");
    await expect(creation.getByRole("textbox", { name: "Create project name", exact: true })).toBeVisible();
    await creation.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
});

function navigationItem(page: Page, label: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${label.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) })
    });
}

async function chooseLanguage(page: Page, name: string) {
    const select = page.locator("#settings-locale");
    const lifecycle = await select.evaluateHandle((element) => ({
        opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
        closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
    }));
    try {
        await select.getByRole("combobox").click();
        await lifecycle.evaluate((events) => events.opened);
        await page.getByRole("option", { name, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
}
