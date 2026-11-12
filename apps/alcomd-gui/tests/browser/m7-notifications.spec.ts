import { expect, test, type Page } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { selectSettingsSection } from "./settings-sections";

const projectId = "00000000-0000-4000-8000-000000000101";
const operationId = "00000000-0000-4000-8000-000000000105";
const notice = (page: Page) => page.getByRole("region", { name: "Notifications", exact: true });

async function startTask(page: Page) {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}&hold-core-operation=1`);
    await page.getByRole("button", { name: /^Update All \(\d+\)$/ }).click();
    await page.locator('md-dialog[open][data-aria-label="Apply package changes?"]').getByRole("button", { name: "Apply changes", exact: true }).click();
    const progress = page.locator('md-dialog[open][data-aria-label="Package changes"]');
    await expect(progress.getByRole("status")).toContainText("running");
    return progress;
}

test("reads and successful favorite changes stay quiet, failures persist without raw exception text", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects");
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("button", { name: "Remove from favorites", exact: true })).toBeVisible();
    await expect(notice(page)).toHaveCount(0);
    await page.goto("/browser-harness.html?route=/projects&state=favorite-error");
    await page.evaluate(() => { window.fixtureCore.projectSetFavorite = async () => { throw { code: "internal_error", message: "SECRET-not-for-display" }; }; });
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(notice(page)).not.toContainText("SECRET-not-for-display");
    await expect(notice(page)).toContainText("Could not complete: Change project favorite");
    await page.clock.install();
    await page.clock.fastForward(10_000);
    await expect(notice(page)).toBeVisible();
    await notice(page).getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(notice(page)).toHaveCount(0);
});

test("manual refresh uses one bottom-right outlet and pauses expiry while focused", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/settings");
    await selectSettingsSection(page, "unity");
    const before = await page.locator("main").boundingBox();
    await page.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect(notice(page)).toContainText("Completed: Discover and refresh");
    const bounds = (await notice(page).boundingBox())!;
    const viewport = page.viewportSize()!;
    expect(Math.abs(viewport.width - bounds.x - bounds.width - 24)).toBeLessThanOrEqual(1);
    expect(Math.abs(viewport.height - bounds.y - bounds.height - 24)).toBeLessThanOrEqual(1);
    expect(await page.locator("main").boundingBox()).toEqual(before);
    await notice(page).getByRole("button", { name: "Dismiss notification", exact: true }).focus();
    await page.clock.fastForward(6000);
    await expect(notice(page)).toBeVisible();
    await page.getByRole("heading", { level: 1 }).focus();
    await page.mouse.move(0, 0);
    await page.clock.runFor(4500);
    await expect(notice(page)).toHaveCount(0);
});

for (const state of ["succeeded", "failed", "cancelled"] as const) {
    test(`minimized task reports ${state} once and never reopens its progress window`, async ({ page }) => {
        const progress = await startTask(page);
        await progress.getByRole("button", { name: "Minimize", exact: true }).click();
        // Finish during the closing animation, before the closed event.
        await page.evaluate(({ id, state }) => window.completeGuiTask(id, state), { id: operationId, state });
        await expect(notice(page)).toContainText(state === "succeeded" ? "Completed: Package changes" : state === "failed" ? "Could not complete: Package changes" : "Cancelled: Package changes");
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await notice(page).getByRole("button", { name: "Dismiss notification", exact: true }).click();
        await page.evaluate(({ id, state }) => window.completeGuiTask(id, state), { id: operationId, state });
        await expect(notice(page)).toHaveCount(0);
    });
}

test("foreground task results stay in their progress window without a duplicate notification", async ({ page }) => {
    const progress = await startTask(page);
    await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), operationId);
    await expect(progress.getByText("Task completed.", { exact: true })).toBeVisible();
    await expect(notice(page)).toHaveCount(0);
    await progress.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(notice(page)).toHaveCount(0);
});

test("a background result waits for an unrelated modal and its task action is explicit", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&hold-core-operation=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await page.locator("md-dialog[open]").getByLabel("Create project name", { exact: true }).fill("Background result");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Create project", exact: true }).click();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Minimize", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    const id = await page.evaluate(async () => (await window.fixtureCore.operationsList()).operations[0]!.operationId);
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Create project", exact: true })).toBeVisible();
    await page.evaluate((id) => window.completeGuiTask(id, "succeeded"), id);
    await expect(notice(page)).toHaveCount(0);
    await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(notice(page)).toContainText("Completed: Create project");
    await notice(page).getByRole("button", { name: "View operation", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/operations/${id}$`));
});

test("partial refresh is one persistent summary instead of a notification for every repository", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=/projects/${projectId}`);
    await page.evaluate(() => {
        window.fixtureCore.repositoriesRefreshAll = async () => ({ items: [
            { repositoryId: "00000000-0000-4000-8000-000000000111", status: "refreshed" },
            { repositoryId: "00000000-0000-4000-8000-000000000112", status: "failed", code: "repository_unavailable" }
        ] });
    });
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(notice(page)).toContainText("1 succeeded · 1 failed or not completed");
    await expect(page.locator(".app-notifications:popover-open")).toHaveCount(1);
});


test("reconnect terminal snapshots and foreign client tasks never produce retrospective notifications", async ({ page }) => {
    const progress = await startTask(page);
    await progress.getByRole("button", { name: "Minimize", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await page.clock.install();
    await page.clock.pauseAt(await page.evaluate(() => Date.now()) + 1000);
    await page.evaluate((id) => {
        window.coreEvents.restart();
        window.completeGuiTask(id, "succeeded");
    }, operationId);
    await page.clock.runFor(1000);
    await expect.poll(() => page.evaluate(() => window.coreEvents.connections)).toBe(1);
    await expect(notice(page)).toHaveCount(0);
    // A task accepted outside the GUI's client belongs only in the task view.
    await page.evaluate(async () => {
        const result = await window.fixtureCore.backupCreate("00000000-0000-4000-8000-000000000101", 2);
        window.completeGuiTask(result.operationId, "succeeded");
    });
    await page.clock.runFor(1000);
    await expect(notice(page)).toHaveCount(0);
});

for (const failure of [false, true]) {
    test(`repository export ${failure ? "error" : "success"} remains in its dialog without a deferred duplicate`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories${failure ? "&list-export-error=permission_denied" : ""}`);
        await selectActionMenuItem(page.getByRole("button", { name: "Repository list actions", exact: true }), page.getByRole("menuitem", { name: "Export list", exact: true }));
        const host = page.locator("md-dialog[open]");
        await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
        await host.getByRole("button", { name: "Save new list file", exact: true }).click();
        if (failure) await expect(host.getByRole("alert")).toContainText("permission_denied");
        else await expect(host.getByText("Saved 231 repositories (30000 bytes).", { exact: true })).toBeVisible();
        await expect(notice(page)).toHaveCount(0);
        await host.getByRole("button", { name: "Close", exact: true }).click();
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await expect(notice(page)).toHaveCount(0);
    });
}

test("hover pauses success dismissal and dismiss restores the previous keyboard focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings");
    await selectSettingsSection(page, "unity");
    const refresh = page.getByRole("button", { name: "Discover and refresh", exact: true });
    await refresh.click();
    await expect(notice(page)).toBeVisible();
    await page.clock.install();
    await notice(page).hover();
    await page.clock.fastForward(6000);
    await expect(notice(page)).toBeVisible();
    const heading = page.getByRole("heading", { level: 1 });
    await heading.focus();
    await notice(page).getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(heading).toBeFocused();
});

test("a task notification survives navigation and fits a narrow dark viewport", async ({ page }) => {
    const progress = await startTask(page);
    await progress.getByRole("button", { name: "Minimize", exact: true }).click();
    await page.locator("#primary-navigation .navigation-item").filter({ hasText: "Settings" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await page.setViewportSize({ width: 320, height: 256 });
    await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
    await page.evaluate((id) => window.completeGuiTask(id, "failed"), operationId);
    await expect(notice(page)).toContainText("Could not complete: Package changes");
    const bounds = (await notice(page).boundingBox())!;
    expect(bounds.x).toBeGreaterThanOrEqual(0);
    expect(bounds.y).toBeGreaterThanOrEqual(0);
    expect(bounds.x + bounds.width).toBeLessThanOrEqual(320);
    expect(bounds.y + bounds.height).toBeLessThanOrEqual(256);
    expect(await notice(page).evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    await page.screenshot({ path: test.info().outputPath("notification-narrow-dark.png") });
});


for (const locale of ["zh-CN", "ja-JP"] as const) {
    test(`notification and dismissal labels are translated in ${locale}`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/settings&locale=${locale}`);
        await selectSettingsSection(page, "unity");
        const refreshName = locale === "zh-CN" ? "查找并刷新" : "検出して更新";
        await page.getByRole("button", { name: refreshName, exact: true }).click();
        const region = page.getByRole("region", { name: locale === "zh-CN" ? "应用内通知" : "通知", exact: true });
        await expect(region).toContainText(locale === "zh-CN" ? "已完成：" : "完了：");
        await expect(region).not.toContainText("Completed:");
        await region.getByRole("button", { name: locale === "zh-CN" ? "关闭通知" : "通知を閉じる", exact: true }).click();
        await expect(region).toHaveCount(0);
    });
}


test("ordinary setting saves stay quiet and final autosave failures clear after reconciliation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&theme-save=commit-error-once");
    await selectSettingsSection(page, "appearance");
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).check();
    await expect(notice(page)).toContainText("Could not complete: Save settings");
    await page.getByRole("button", { name: "Retry saving settings", exact: true }).click();
    await expect(page.getByRole("button", { name: "Retry saving settings", exact: true })).toHaveCount(0);
    await expect(notice(page)).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.getByRole("checkbox", { name: "Remember window size and maximized state", exact: true }).uncheck();
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(2);
    await expect(notice(page)).toHaveCount(0);
});

test("opening Unity reports a manual preflight failure and keeps chooser failures local", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&unityOptionsError=1");
    await page.locator(".project-row-actions").first().getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect(notice(page)).toContainText("Could not complete: Open Unity");
    await notice(page).getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await page.goto("/browser-harness.html?route=/projects&state=unity-multiple&unityLaunchError=1");
    await page.locator(".project-row-actions").first().getByRole("button", { name: "Open Unity", exact: true }).click();
    const chooser = page.locator("md-dialog[open]");
    await chooser.getByRole("combobox", { name: "Unity installation for this launch", exact: true }).click();
    await page.getByRole("option", { name: /<private-editor-two>/ }).click();
    await chooser.getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect(chooser.getByRole("alert")).toContainText("unity_launch_failed");
    await chooser.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(notice(page)).toHaveCount(0);
});


for (const minimized of [false, true]) {
    test(`template saving ${minimized ? "continues after minimizing with one result notice" : "keeps its result in the continuous dialog"}`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/templates");
        await page.getByRole("button", { name: "Create template", exact: true }).click();
        const host = page.locator("md-dialog[open]");
        await host.getByLabel("Template ID", { exact: true }).fill("com.example.notification");
        await host.getByLabel("Display name", { exact: true }).fill("Notification template");
        await page.clock.install();
        await page.clock.pauseAt(await page.evaluate(() => Date.now()) + 1000);
        await host.getByRole("button", { name: "Save template", exact: true }).click();
        await page.clock.runFor(1);
        await expect(host.getByRole("status")).toContainText("running");
        if (minimized) await host.getByRole("button", { name: "Minimize", exact: true }).click();
        await page.clock.runFor(1000);
        if (minimized) {
            await expect(page.getByRole("dialog")).toHaveCount(0);
            await expect(notice(page)).toContainText("Completed: Save template");
        } else {
            await expect(host.getByRole("status")).toContainText("succeeded");
            await host.getByRole("button", { name: "Close", exact: true }).click();
            await expect(page.getByRole("dialog")).toHaveCount(0);
            await expect(notice(page)).toHaveCount(0);
        }
    });
}

test("manual permission refresh waits for its reads and then reports completion after closing the modal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/extensions/com.vrclearn.discord");
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    const refresh = host.getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeEnabled();
    await refresh.click();
    await expect(refresh).toBeEnabled();
    await expect(notice(page)).toHaveCount(0);
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await expect(notice(page)).toContainText("Completed: Refresh");
});


test("a picker failure already shown in the form is not repeated after the form closes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await page.evaluate(() => { window.fixtureCore.selectProjectTemplateFile = async () => { throw { code: "file_selection_failed", message: "private picker exception" }; }; });
    await selectActionMenuItem(page.getByRole("button", { name: "More template creation options", exact: true }), page.getByRole("menuitem", { name: "Import template", exact: true }));
    const host = page.locator("md-dialog[open]");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    await expect(host.getByRole("alert")).toContainText("Could not select the file.");
    await host.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(notice(page)).toHaveCount(0);
});
