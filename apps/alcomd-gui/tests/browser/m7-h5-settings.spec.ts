import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";
import { H5_HIDDEN_ID, H5_HUB_PATH, H5_REPOSITORY_CURSOR, H5_SETTINGS_DIAGNOSTIC, H5_SETTINGS_SECRET, H5_VISIBLE_ID } from "./h5-settings-client";

const hub = (page: Page) => page.getByRole("region", { name: "Unity Hub Path", exact: true });
const filters = (page: Page) => page.getByRole("dialog", { name: "Package filters", exact: true });
const changes = (page: Page) => page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.update"));

async function openSettings(page: Page, extra = "") {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto(`/browser-harness.html?route=/settings&h5-settings=1&${extra}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await selectSettingsSection(page, "appearance");
    await expect(page.locator("#settings-motion")).toBeVisible();
}
async function fail(page: Page, method: string, code: string, afterCommit = false) {
    await page.evaluate(({ method, code, afterCommit }) => window.h5Settings.failNext(method, code, afterCommit), { method, code, afterCommit });
}
function navigation(page: Page, name: string) {
    return page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });
}

async function openProject(page: Page) {
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Manage", exact: true }).click();
}

async function selectMotion(page: Page, value: "full") {
    await selectSettingsSection(page, "appearance");
    await page.locator("#settings-motion").evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

test("H5 autosave retains a definitive cause and safe details across navigation before explicit Retry", async ({ page }) => {
    await openSettings(page);
    await page.setViewportSize({ width: 1280, height: 800 });
    await fail(page, "settings.update", "permission_denied");
    await selectMotion(page, "full");
    const feedback = page.locator(".settings-save-error").getByRole("alert");
    await expect(feedback).toContainText("Settings could not be saved.");
    await expect(feedback).toContainText("You do not have permission for this action.");
    await expect(feedback.locator("code")).toHaveCount(0);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
    const outlet = page.locator(".settings-save-error");
    await expect(outlet).toHaveCount(1);
    const errorBounds = (await outlet.boundingBox())!;
    expect(errorBounds.x).toBeGreaterThanOrEqual(0);
    expect(errorBounds.x + errorBounds.width).toBeLessThanOrEqual(1280);
    expect(await outlet.evaluate((node) => getComputedStyle(node).position)).not.toBe("fixed");
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(feedback).toBeVisible();
    await selectSettingsSection(page, "unity");
    await selectSettingsSection(page, "appearance");
    await expect(page.locator(".utility-workspace-toolbar")).toBeVisible();
    await navigation(page, "Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await expect(feedback).toBeVisible();
    await feedback.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(feedback.locator("code")).toHaveText(["permission_denied", H5_SETTINGS_DIAGNOSTIC]);
    expect(await changes(page)).toHaveLength(1);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await changes(page)).toHaveLength(2);
    expect(await page.evaluate(() => window.h5Settings.requests.slice(-2).map((request) => request.method))).toEqual(["settings.get", "settings.update"]);
    await navigation(page, "Settings").click();
    await expect(page.locator("#settings-motion")).toHaveJSProperty("value", "full");
});

test("closing a save notification retains the failure and an explicit Settings retry reconciles the unknown write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "daemon_unavailable", true);
    await selectMotion(page, "full");
    const outlet = page.locator(".settings-save-error");
    await expect(outlet).toContainText("Settings save result is unknown");
    await expect(outlet.getByRole("alert")).toBeVisible();
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    await expect(theme.getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    const reads = await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length);
    await page.getByRole("button", { name: "Retry saving settings", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    await expect(page.getByRole("button", { name: "Retry saving settings", exact: true })).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length)).toBe(reads + 1);
});

test("H5 unknown autosave reports uncertainty and checks Core before avoiding a duplicate committed write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "daemon_unavailable", true);
    await selectMotion(page, "full");
    const feedback = page.locator(".settings-save-error").getByRole("alert");
    await expect(feedback).toContainText("Settings save result is unknown");
    await expect(feedback).toContainText("Retry checks the current settings before writing again.");
    await expect(feedback).not.toContainText("Check Tasks");
    await expect(page.locator("html")).toHaveAttribute("data-motion", "system");
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    const reads = await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await expect(page.locator("html")).toHaveAttribute("data-motion", "full");
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.get").length)).toBe(reads + 1);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
});

test("H5 theme autosave uses shared unknown feedback and reconciles to one write with readable inline feedback", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "internal_error", true);
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    const rowsBefore = await theme.locator(".settings-row").evaluateAll((rows) => rows.map((row) => ({ width: row.getBoundingClientRect().width, height: row.getBoundingClientRect().height })));
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("Settings save result is unknown");
    expect(await theme.locator(".settings-row").evaluateAll((rows) => rows.map((row) => ({ width: row.getBoundingClientRect().width, height: row.getBoundingClientRect().height })))).toEqual(rowsBefore);
    await expect(theme.getByRole("alert")).toHaveCount(0);
    const failureBounds = (await feedback.boundingBox())!;
    expect(failureBounds.x).toBeGreaterThanOrEqual(0);
    expect(failureBounds.x + failureBounds.width).toBeLessThanOrEqual(1400);
    expect(await feedback.evaluate((node) => getComputedStyle(node).position)).not.toBe("fixed");
    await expect(feedback).toContainText("Retry checks the current settings before writing again.");
    await expect(feedback).not.toContainText("Check Tasks");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await expect(theme.getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    await expect(page.locator("#theme-source").getByRole("textbox")).toHaveValue("#E040A0");
});

test("dismissing an unknown theme result still permits shared settings Retry", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "internal_error", true);
    const theme = page.getByRole("form", { name: "Theme", exact: true });
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    const outlet = page.locator(".settings-save-error");
    await expect(outlet.getByRole("alert")).toContainText("Settings save result is unknown");
    await expect(outlet.getByRole("alert")).toBeVisible();
    await page.getByRole("button", { name: "Retry saving settings", exact: true }).click();
    await expect(page.getByRole("button", { name: "Retry saving settings", exact: true })).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
    await expect(theme.getByRole("button", { name: /^(Save|Cancel)$/ })).toHaveCount(0);
});

test("H5 Hub unknown reply preserves original picker input and reconciles before any further write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "internal_error", true);
    await selectSettingsSection(page, "unity");
    await hub(page).getByRole("button", { name: "Select", exact: true }).click();
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("Unity Hub save result is unknown");
    await expect(feedback).toContainText("Retry checks the current path before writing again.");
    await expect(feedback).not.toContainText("Check Tasks");
    expect((await changes(page))[0]?.params).toEqual({ expectedRevision: 7, update: { unity: { hubPath: H5_HUB_PATH } } });
    await feedback.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(feedback.locator("code")).toHaveText(["internal_error", H5_SETTINGS_DIAGNOSTIC]);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(hub(page).locator("md-outlined-text-field")).toHaveJSProperty("value", H5_HUB_PATH);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "unity.installations.refresh").length)).toBe(1);
});

test("H5 confirmed Hub save remains accepted when Editor discovery reports a separate failure", async ({ page }) => {
    await openSettings(page);
    await fail(page, "unity.installations.refresh", "internal_error");
    await selectSettingsSection(page, "unity");
    await hub(page).getByRole("button", { name: "Select", exact: true }).click();
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("Unity Hub path was saved, but Unity installations could not be refreshed.");
    await expect(feedback).not.toContainText("Check Tasks");
    await feedback.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(feedback.locator("code")).toHaveText(["internal_error", H5_SETTINGS_DIAGNOSTIC]);
    await expect(hub(page).locator("md-outlined-text-field")).toHaveJSProperty("value", H5_HUB_PATH);
    await expect(feedback.getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    await expect(page.getByRole("button", { name: "Refresh Unity installations", exact: true })).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Unity installations", exact: true }).getByRole("alert")).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
});

test("H5 Auto detect read failure does not claim an unknown save or submit a settings write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.get", "daemon_unavailable");
    await selectSettingsSection(page, "unity");
    await hub(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("Unity Hub location could not be detected. Retry the operation.");
    await expect(feedback).not.toContainText("save result is unknown");
    await expect(feedback).not.toContainText("Check Tasks");
    expect(await changes(page)).toEqual([]);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await changes(page)).toEqual([]);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "unity.installations.refresh"))).toEqual([]);
});

test("Project package sources report a safe read failure and Retry loads every repository page", async ({ page }) => {
    await openSettings(page);
    await fail(page, "repositories.list", "permission_denied");
    await openProject(page);
    const feedback = page.getByRole("alert");
    await expect(feedback).toContainText("You do not have permission for this action.");
    expect(await changes(page)).toEqual([]);
    await expect(page.getByRole("button", { name: "Filter packages", exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Reconnect and retry", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    await expect(filters(page).getByRole("checkbox", { name: "H5 hidden source", exact: true })).toBeVisible();
    await expect(filters(page).getByRole("checkbox", { name: "H5 visible source", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "repositories.list").slice(-2).map((request) => request.params))).toEqual([{}, { cursor: H5_REPOSITORY_CURSOR }]);
    expect(await changes(page)).toEqual([]);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
});

test("Project package filters enforce the complete 256-ID bound while allowing existing entries to be unhidden", async ({ page }) => {
    await openSettings(page, "h5-hidden-limit=1");
    await openProject(page);
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    const hidden = filters(page).getByRole("checkbox", { name: "H5 hidden source", exact: true });
    const visible = filters(page).getByRole("checkbox", { name: "H5 visible source", exact: true });
    await expect(hidden).not.toBeChecked();
    await expect(hidden).toBeEnabled();
    await expect(visible).toBeChecked();
    await expect(visible).toBeDisabled();
    await expect(filters(page).getByRole("alert")).toContainText("Up to 256 repositories can be hidden.");
    await hidden.check();
    await expect.poll(() => changes(page).then((requests) => requests.length)).toBe(1);
    await expect(visible).toBeEnabled();
    await visible.uncheck();
    await expect.poll(() => changes(page).then((requests) => requests.length)).toBe(2);
    await expect.poll(() => page.evaluate(() => window.h5Settings.commits.length)).toBe(2);
    const lists = await page.evaluate(() => window.h5Settings.commits.map((commit) => commit.update.packages!.hiddenRepositoryIds!));
    expect(lists.map((ids) => ids.length)).toEqual([255, 256]);
    expect(lists[0]).not.toContain(H5_HIDDEN_ID);
    expect(lists[1]).toContain(H5_VISIBLE_ID);
    expect(lists[1]).toEqual([...lists[1]!].sort());
    await expect(hidden).toBeDisabled();
    await expect(visible).toBeEnabled();
    await expect(visible).not.toBeChecked();
});
