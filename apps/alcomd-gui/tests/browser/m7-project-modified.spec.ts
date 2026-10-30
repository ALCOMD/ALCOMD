import { expect, test, type Page } from "@playwright/test";

const descending = ["Favorite", "Newest", "Equal A", "Equal B", "Unknown history"];
const ascending = ["Favorite", "Equal A", "Equal B", "Newest", "Unknown history"];

test("Projects last-modified order uses authoritative timestamps, stable ties and unknown-last in both directions", async ({ page }) => {
    await openProjects(page);
    const header = page.getByRole("columnheader", { name: "Last modified", exact: true });
    await expect(header).toHaveAttribute("aria-sort", "descending");
    await expect(listNames(page)).toHaveText(descending);
    const unknown = page.locator(".projects-list tbody tr").filter({ hasText: "Unknown history" });
    await expect(unknown.locator("td").nth(4)).toHaveText("Unknown Date");
    await expect(page.locator(".projects-list tbody tr").filter({ hasText: "Newest" }).locator("td").nth(4)).toHaveText("1 day ago");

    await header.getByRole("button", { name: "Last modified", exact: true }).click();
    await expect(header).toHaveAttribute("aria-sort", "ascending");
    await expect(listNames(page)).toHaveText(ascending);
    await header.getByRole("button", { name: "Last modified", exact: true }).click();
    await expect(listNames(page)).toHaveText(descending);

    const before = await page.evaluate(() => ({ count: window.projectSnapshots.length, snapshot: window.projectSnapshots.at(-1)! }));
    await page.getByRole("button", { name: "Refresh projects", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.projectSnapshots.length)).toBeGreaterThan(before.count);
    const after = await page.evaluate(() => window.projectSnapshots.at(-1)!);
    expect([...before.snapshot, ...after].every((item) => !Object.hasOwn(item, "observedAtMs"))).toBe(true);
    expect(after.map((item) => [item.projectId, item.lastModifiedAtMs])).toEqual(before.snapshot.map((item) => [item.projectId, item.lastModifiedAtMs]));
    await expect(listNames(page)).toHaveText(descending);
    await expect(unknown.locator("td").nth(4)).toHaveText("Unknown Date");
});

test("Project grid and list share last-modified sorting and never substitute check or registration dates", async ({ page }) => {
    await openProjects(page);
    await page.getByRole("button", { name: "List view", exact: true }).click();
    await expect(page.locator(".projects-sort")).toHaveJSProperty("value", "modified");
    await expect(page.getByRole("combobox", { name: "Sort by", exact: true })).toContainText("Last modified");
    await expect(page.locator(".project-card h2")).toHaveText(descending);
    const unknown = page.getByRole("article", { name: "Unknown history", exact: true });
    await expect(unknown.locator(".project-card-dates")).toContainText("Last modified Unknown Date");
    await expect(page.getByRole("article", { name: "Newest", exact: true }).locator(".project-card-dates")).toContainText("Last modified 1 day ago");
    await expect(page.getByText(/Last observed/)).toHaveCount(0);
    await page.getByRole("button", { name: "Sort descending", exact: true }).click();
    await expect(page.locator(".project-card h2")).toHaveText(ascending);
    await page.getByRole("button", { name: "Grid view", exact: true }).click();
    await expect(page.getByRole("columnheader", { name: "Last modified", exact: true })).toHaveAttribute("aria-sort", "ascending");
    await expect(listNames(page)).toHaveText(ascending);
});

for (const language of [
    { name: "简体中文", locale: "zh-CN", projects: "项目", modified: "最后修改", unknown: "未知日期", list: "列表视图" },
    { name: "日本語", locale: "ja-JP", projects: "プロジェクト", modified: "最終更新日", unknown: "不明", list: "リスト表示" }
] as const) {
    test(`${language.locale} uses v3 last-modified terminology and an explicit unknown date`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=%2Fsettings&state=ready&projectModified=1");
        await page.locator("#settings-locale").getByRole("combobox").click();
        await page.getByRole("option", { name: language.name, exact: true }).click();
        await expect(page.locator("html")).toHaveAttribute("lang", language.locale);
        await page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${language.projects}$`) }) }).click();
        await expect(page.getByRole("columnheader", { name: language.modified, exact: true })).toHaveAttribute("aria-sort", "descending");
        await expect(page.locator(".projects-list tbody tr").filter({ hasText: "Unknown history" }).locator("td").nth(4)).toHaveText(language.unknown);
        await page.getByRole("button", { name: language.list, exact: true }).click();
        await expect(page.getByRole("article", { name: "Unknown history", exact: true }).locator(".project-card-dates")).toContainText(`${language.modified}：${language.unknown}`);
    });
}

for (const view of ["list", "grid"] as const) {
    for (const failed of [false, true]) {
        test(`${view} launch ${failed ? "failure" : "success"} rereads the Core last-modified value without a retry`, async ({ page }) => {
            await openProjects(page, failed ? "unityLaunchError=1" : "");
            if (view === "grid") await page.getByRole("button", { name: "List view", exact: true }).click();
            const target = view === "grid" ? page.getByRole("article", { name: "Equal B", exact: true }) : page.locator(".projects-list tbody tr").filter({ hasText: "Equal B" });
            const date = view === "grid" ? target.locator(".project-card-dates") : target.locator("td").nth(4);
            await expect(date).toContainText("2 days ago");
            await target.getByRole("button", { name: "Open Unity", exact: true }).click();
            await expect(date).toContainText("12 hours ago");
            if (failed) await expect(page.getByRole("status").filter({ hasText: "launch_failed" })).toBeVisible();
            else await expect(page.getByRole("status").filter({ hasText: "Unity launch opening." })).toBeVisible();
            const names = view === "grid" ? page.locator(".project-card h2") : listNames(page);
            await expect(names).toHaveText(["Favorite", "Equal B", "Newest", "Equal A", "Unknown history"]);
            expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toHaveLength(1);
            const snapshot = await page.evaluate(() => window.projectSnapshots.at(-1)!);
            expect(snapshot.find((item) => item.projectId === "00000000-0000-4000-8000-000000000102")?.lastModifiedAtMs).toBe(1_700_216_000_000);
        });
    }

    test(`${view} launch-options failure neither retries launch nor rereads a modification`, async ({ page }) => {
        await openProjects(page, "unityOptionsError=1");
        if (view === "grid") await page.getByRole("button", { name: "List view", exact: true }).click();
        const count = await page.evaluate(() => window.projectSnapshots.length);
        const target = view === "grid" ? page.getByRole("article", { name: "Equal B", exact: true }) : page.locator(".projects-list tbody tr").filter({ hasText: "Equal B" });
        await target.getByRole("button", { name: "Open Unity", exact: true }).click();
        await expect(page.getByRole("status").filter({ hasText: "revision_conflict" })).toBeVisible();
        const date = view === "grid" ? target.locator(".project-card-dates") : target.locator("td").nth(4);
        await expect(date).toContainText("2 days ago");
        expect(await page.evaluate(() => window.projectSnapshots.length)).toBe(count);
        expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toHaveLength(0);
    });
}

async function openProjects(page: Page, extraQuery = "") {
    await page.addInitScript(() => { Date.now = () => 1_700_259_200_000; });
    await page.goto(`/browser-harness.html?route=%2Fprojects&state=ready&projectModified=1&${extraQuery}`);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
}

function listNames(page: Page) { return page.locator(".projects-list tbody tr td:first-child strong"); }
