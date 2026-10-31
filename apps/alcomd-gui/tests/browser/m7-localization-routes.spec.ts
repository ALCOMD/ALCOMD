import { expect, test, type Page } from "@playwright/test";

type RouteExpectation = {
    path: string;
    title: string;
    role: "button" | "columnheader" | "searchbox" | "heading";
    control: string;
};

const languages = [
    {
        locale: "zh-CN", name: "简体中文", settings: "设置", otherName: "日本語", otherLocale: "ja-JP",
        routes: [
            { path: "/repositories", title: "资源", role: "button", control: "添加存储库" },
            { path: "/user-packages", title: "资源", role: "button", control: "添加文件夹" },
            { path: "/templates", title: "资源", role: "button", control: "创建模板" },
            { path: "/unity", title: "设置", role: "button", control: "管理安装" },
            { path: "/extensions", title: "扩展", role: "button", control: "安装扩展" },
            { path: "/activity", title: "日志", role: "searchbox", control: "搜索活动" },
            { path: "/diagnostics", title: "日志", role: "button", control: "运行状态检查" },
            { path: "/operations", title: "任务", role: "columnheader", control: "状态" },
            { path: "/about", title: "关于", role: "heading", control: "许可证" }
        ]
    },
    {
        locale: "ja-JP", name: "日本語", settings: "設定", otherName: "简体中文", otherLocale: "zh-CN",
        routes: [
            { path: "/repositories", title: "パッケージ&テンプレート", role: "button", control: "リポジトリを追加" },
            { path: "/user-packages", title: "パッケージ&テンプレート", role: "button", control: "フォルダーを登録" },
            { path: "/templates", title: "パッケージ&テンプレート", role: "button", control: "テンプレートを作成" },
            { path: "/unity", title: "設定", role: "button", control: "インストールを管理" },
            { path: "/extensions", title: "拡張機能", role: "button", control: "拡張機能をインストール" },
            { path: "/activity", title: "ログ", role: "searchbox", control: "アクティビティを検索" },
            { path: "/diagnostics", title: "ログ", role: "button", control: "状態を検査" },
            { path: "/operations", title: "タスク", role: "columnheader", control: "状態" },
            { path: "/about", title: "このアプリについて", role: "heading", control: "ライセンス" }
        ]
    }
] satisfies Array<{
    locale: string;
    name: string;
    settings: string;
    otherName: string;
    otherLocale: string;
    routes: RouteExpectation[];
}>;

const englishRoutes: RouteExpectation[] = [
    { path: "/repositories", title: "Resources", role: "button", control: "Add repository" },
    { path: "/user-packages", title: "Resources", role: "button", control: "Enroll folder" },
    { path: "/templates", title: "Resources", role: "button", control: "Create template" },
    { path: "/unity", title: "Settings", role: "button", control: "Manage installations" },
    { path: "/extensions", title: "Extensions", role: "button", control: "Install extension" },
    { path: "/activity", title: "Logs", role: "searchbox", control: "Search activity" },
    { path: "/diagnostics", title: "Logs", role: "button", control: "Run state check" },
    { path: "/operations", title: "Tasks", role: "columnheader", control: "Status" },
    { path: "/about", title: "About", role: "heading", control: "Licenses" }
];

test("Shared Select refreshes translated selected text without changing value, focus or settings", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fsettings&theme-save=hold");
    const motion = page.locator("#settings-motion");
    await expect(motion).toHaveJSProperty("value", "system");
    await expect(motion.getByRole("combobox")).toContainText("System");
    const observed = await motion.evaluateHandle((element) => {
        const events: string[] = [];
        element.addEventListener("input", () => events.push("input"));
        element.addEventListener("change", () => events.push("change"));
        return { element, events };
    });
    for (const [index, language] of [
        { name: "简体中文", locale: "zh-CN", selected: "跟随系统", options: ["跟随系统", "启用动画", "关闭动画"] },
        { name: "English", locale: "en-US", selected: "System", options: ["System", "Enable animations", "Disable animations"] }
    ].entries()) {
        const locale = page.locator("#settings-locale");
        const lifecycle = await locale.evaluateHandle((element) => ({
            opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
            closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
        }));
        await locale.getByRole("combobox").click();
        await lifecycle.evaluate((events) => events.opened);
        await page.getByRole("option", { name: language.name, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
        await lifecycle.dispose();
        await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(index + 1);
        await motion.getByRole("combobox").focus();
        await expect(motion.getByRole("combobox")).toBeFocused();
        await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        await expect(motion.locator("md-select-option")).toHaveText(language.options);
        await expect(motion.getByRole("combobox")).toContainText(language.selected);
        await expect(motion.getByRole("combobox")).toBeFocused();
        await expect(motion).toHaveJSProperty("value", "system");
        await expect(motion).toHaveJSProperty("disabled", false);
        expect(await motion.evaluate((element, state) => element === state.element, observed)).toBe(true);
        expect(await observed.evaluate((state) => state.events)).toEqual([]);
        expect(await page.evaluate(() => window.settingsWrites)).toHaveLength(index + 1);
    }
    await observed.dispose();
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ locale: "zh-CN" }, { locale: "en-US" }]);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([{ expectedRevision: 7, revision: 8 }, { expectedRevision: 8, revision: 9 }]);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

for (const language of languages) {
    test(`${language.locale} covers the existing utility routes and switches back to English without translating external data`, async ({ page }) => {
        test.setTimeout(60_000);
        await page.goto("/browser-harness.html?route=%2Fsettings&state=package-user-source");
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
        await saveLanguage(page, language.name, language.locale);
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        await expect(page.getByRole("heading", { level: 1, name: language.settings, exact: true })).toBeVisible();

        for (const route of language.routes) {
            await visitFixtureRoute(page, route.path);
            await expect(page.getByRole("heading", { level: 1, name: route.title, exact: true })).toBeVisible();
            await expect(page.getByRole(route.role, { name: route.control, exact: true })).toBeVisible();
            await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
            await assertExternalFixtureText(page, route.path);
        }

        // Exercise both saved non-English preferences before restoring English. These
        // are the only settings mutations; visits must not create additional writes.
        await visitFixtureRoute(page, "/settings");
        await saveLanguage(page, language.otherName, language.otherLocale);
        await expect(page.locator("html")).toHaveAttribute("lang", language.otherLocale);
        await saveLanguage(page, "English", "en-US");
        await expect(page.locator("html")).toHaveAttribute("lang", "en-US");
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();

        for (const route of englishRoutes) {
            await visitFixtureRoute(page, route.path);
            await expect(page.getByRole("heading", { level: 1, name: route.title, exact: true })).toBeVisible();
            await expect(page.getByRole(route.role, { name: route.control, exact: true })).toBeVisible();
            await assertExternalFixtureText(page, route.path);
        }
        expect(await page.evaluate(() => window.settingsWrites)).toEqual([
            { expectedRevision: 7, revision: 8 },
            { expectedRevision: 8, revision: 9 },
            { expectedRevision: 9, revision: 10 }
        ]);
        expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    });
}

async function saveLanguage(page: Page, name: string, locale: string) {
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
    await expect(page.locator("html")).toHaveAttribute("lang", locale);
}

/** Keep the same browser-fixture App/client; a reload would reset its in-memory settings. */
async function visitFixtureRoute(page: Page, path: string) {
    await page.evaluate((nextPath) => {
        window.history.pushState(null, "", nextPath);
        window.dispatchEvent(new PopStateEvent("popstate"));
    }, path);
    await expect(page).toHaveURL(new RegExp(`${path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`));
}

async function assertExternalFixtureText(page: Page, path: string) {
    if (path === "/repositories") {
        const table = page.getByRole("table");
        await expect(table.getByText("Example packages", { exact: true })).toBeVisible();
        await expect(table).toContainText("https://packages.example.invalid/index.json");
    } else if (path === "/user-packages") {
        const table = page.getByRole("table");
        await expect(table.getByText("Local avatar tools", { exact: true })).toBeVisible();
        await expect(table).toContainText("com.example.avatar");
        await expect(table).toContainText("1.2.3");
    } else if (path === "/templates") {
        await expect(page.getByRole("table")).toContainText("Avatar starter");
        await expect(page.getByRole("table")).toContainText("com.cqmhv.template.avatar");
    } else if (path === "/unity") {
        await expect(page.getByRole("table")).toContainText("2022.3.22f1");
        await expect(page.getByRole("table")).toContainText("x86_64");
    } else if (path === "/extensions") {
        await expect(page.locator(".extension-card h4").filter({ hasText: /^com\.cqmhv\.discord$/ })).toBeVisible();
    } else if (path === "/diagnostics") {
        await expect(page.getByRole("table")).toContainText("package_archive_invalid");
        const locale = await page.locator("html").getAttribute("lang");
        const summary = locale === "zh-CN" ? "操作失败。请求支持时请提供诊断 ID。"
            : locale === "ja-JP" ? "処理に失敗しました。サポートに問い合わせる際は診断 ID をお知らせください。"
            : "The operation failed. Use the diagnostic ID when requesting support.";
        await expect(page.getByRole("table")).toContainText(summary);
    } else if (path === "/about") {
        await expect(page.locator(".detail-grid dd").filter({ hasText: /^ALCOMD$/ })).toBeVisible();
        await expect(page.getByText("THIRD_PARTY_NOTICES.md", { exact: true })).toBeVisible();
    }
}
