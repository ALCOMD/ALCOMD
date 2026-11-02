import { expect, test, type Page } from "@playwright/test";
import { H5_HIDDEN_ID, H5_HUB_PATH, H5_REPOSITORY_CURSOR, H5_SETTINGS_DIAGNOSTIC, H5_SETTINGS_SECRET, H5_VISIBLE_ID } from "./h5-settings-client";

const hub = (page: Page) => page.getByRole("region", { name: "Unity Hub Path", exact: true });
const packages = (page: Page) => page.getByRole("region", { name: "Packages", exact: true });
const changes = (page: Page) => page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "settings.update"));

async function openSettings(page: Page, extra = "") {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto(`/browser-harness.html?route=/settings&h5-settings=1&${extra}`);
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await expect(page.locator("#settings-motion")).toBeVisible();
}
async function fail(page: Page, method: string, code: string, afterCommit = false) {
    await page.evaluate(({ method, code, afterCommit }) => window.h5Settings.failNext(method, code, afterCommit), { method, code, afterCommit });
}
function navigation(page: Page, name: string) {
    return page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });
}

async function selectMotion(page: Page, value: "full") {
    await page.locator("#settings-motion").evaluate((element, next) => {
        (element as HTMLElement & { select(value: string): void }).select(next);
        element.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

test("H5 autosave retains a definitive cause and safe details across navigation before explicit Retry", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "permission_denied");
    await selectMotion(page, "full");
    const feedback = page.locator(".settings-save-error").getByRole("alert");
    await expect(feedback).toContainText("Settings could not be saved.");
    await expect(feedback).toContainText("You do not have permission for this action.");
    await expect(feedback.locator("code")).toHaveCount(0);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
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

test("H5 Theme unknown reply preserves preview and reconciles to one write with layout-free success feedback", async ({ page }) => {
    await openSettings(page);
    await page.getByRole("button", { name: "Theme", exact: true }).click();
    await page.locator("#theme-source").getByRole("textbox").fill("#E040A0");
    await fail(page, "settings.update", "internal_error", true);
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Save", exact: true }).click();
    const feedback = dialog.getByRole("alert");
    await expect(feedback).toContainText("Theme save result is unknown");
    await expect(feedback).toContainText("Retry checks the current settings before writing again.");
    await expect(feedback).not.toContainText("Check Tasks");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#E040A0");
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
    await dialog.getByRole("button", { name: "Retry theme save", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeHidden();
    expect(await changes(page)).toHaveLength(1);
    await expect(page.locator(".reminder-floating")).toContainText("Theme saved");
    await expect(page.getByRole("region", { name: "Appearance", exact: true })).not.toContainText("Theme saved");
});

test("H5 Hub unknown reply preserves original picker input and reconciles before any further write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.update", "internal_error", true);
    await hub(page).getByRole("button", { name: "Select", exact: true }).click();
    const feedback = hub(page).getByRole("alert");
    await expect(feedback).toContainText("Unity Hub save result is unknown");
    await expect(feedback).toContainText("Retry checks the current path before writing again.");
    await expect(feedback).not.toContainText("Check Tasks");
    expect((await changes(page))[0]?.params).toEqual({ expectedRevision: 7, update: { unity: { hubPath: H5_HUB_PATH } } });
    await feedback.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(feedback.locator("code")).toHaveText(["internal_error", H5_SETTINGS_DIAGNOSTIC]);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(hub(page).getByRole("alert")).toHaveCount(0);
    await expect(hub(page).locator("md-outlined-text-field")).toHaveJSProperty("value", H5_HUB_PATH);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "unity.installations.refresh").length)).toBe(1);
});

test("H5 confirmed Hub save remains accepted when Editor discovery reports a separate failure", async ({ page }) => {
    await openSettings(page);
    await fail(page, "unity.installations.refresh", "internal_error");
    await hub(page).getByRole("button", { name: "Select", exact: true }).click();
    const feedback = hub(page).getByRole("alert");
    await expect(feedback).toContainText("Unity Hub path was saved, but Unity installations could not be refreshed.");
    await expect(feedback).not.toContainText("Check Tasks");
    await feedback.getByRole("button", { name: "Show details", exact: true }).click();
    await expect(feedback.locator("code")).toHaveText(["internal_error", H5_SETTINGS_DIAGNOSTIC]);
    await expect(hub(page).locator("md-outlined-text-field")).toHaveJSProperty("value", H5_HUB_PATH);
    await expect(feedback.getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
    expect(await page.evaluate(() => window.h5Settings.commits.length)).toBe(1);
    await page.getByRole("button", { name: "Refresh Unity installations", exact: true }).click();
    await expect(page.getByRole("region", { name: "Unity installations", exact: true }).getByRole("alert")).toHaveCount(0);
    expect(await changes(page)).toHaveLength(1);
});

test("H5 Auto detect read failure does not claim an unknown save or submit a settings write", async ({ page }) => {
    await openSettings(page);
    await fail(page, "settings.get", "daemon_unavailable");
    await hub(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    const feedback = hub(page).getByRole("alert");
    await expect(feedback).toContainText("Unity Hub location could not be detected. Retry the operation.");
    await expect(feedback).not.toContainText("save result is unknown");
    await expect(feedback).not.toContainText("Check Tasks");
    expect(await changes(page)).toEqual([]);
    await feedback.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(hub(page).getByRole("alert")).toHaveCount(0);
    expect(await changes(page)).toEqual([]);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "unity.installations.refresh"))).toEqual([]);
});

test("H5 repository options preserve loaded rows and retry the same failed cursor with a readable cause", async ({ page }) => {
    await openSettings(page);
    await fail(page, "repositories.list", "permission_denied");
    await packages(page).getByRole("button", { name: "Load more repository options", exact: true }).click();
    const feedback = packages(page).getByRole("alert");
    await expect(feedback).toContainText("Repository visibility options could not be loaded.");
    await expect(feedback).toContainText("You do not have permission for this action.");
    await expect(packages(page).getByRole("checkbox", { name: "Hide H5 hidden source", exact: true })).toBeVisible();
    await expect(packages(page).getByRole("checkbox", { name: "Hide H5 visible source", exact: true })).toHaveCount(0);
    await feedback.getByRole("button", { name: "Retry repository options", exact: true }).click();
    await expect(feedback).toHaveCount(0);
    await expect(packages(page).getByRole("checkbox", { name: "Hide H5 visible source", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "repositories.list" && "cursor" in request.params).map((request) => request.params))).toEqual([{ cursor: H5_REPOSITORY_CURSOR }, { cursor: H5_REPOSITORY_CURSOR }]);
    await expect(page.locator("body")).not.toContainText(H5_SETTINGS_SECRET);
});

test("H5 repository visibility enforces the complete 256-ID bound while allowing existing entries to be unhidden", async ({ page }) => {
    await openSettings(page, "h5-hidden-limit=1");
    await packages(page).getByRole("button", { name: "Load more repository options", exact: true }).click();
    const hidden = packages(page).getByRole("checkbox", { name: "Hide H5 hidden source", exact: true });
    const visible = packages(page).getByRole("checkbox", { name: "Hide H5 visible source", exact: true });
    await expect(hidden).toBeChecked();
    await expect(hidden).toBeEnabled();
    await expect(visible).not.toBeChecked();
    await expect(visible).toBeDisabled();
    await expect(packages(page).getByRole("alert")).toContainText("Up to 256 repositories can be hidden.");
    await hidden.uncheck();
    await expect.poll(() => changes(page).then((requests) => requests.length)).toBe(1);
    await expect(visible).toBeEnabled();
    await visible.check();
    await expect.poll(() => changes(page).then((requests) => requests.length)).toBe(2);
    await expect.poll(() => page.evaluate(() => window.h5Settings.commits.length)).toBe(2);
    const lists = await page.evaluate(() => window.h5Settings.commits.map((commit) => commit.update.packages!.hiddenRepositoryIds!));
    expect(lists.map((ids) => ids.length)).toEqual([255, 256]);
    expect(lists[0]).not.toContain(H5_HIDDEN_ID);
    expect(lists[1]).toContain(H5_VISIBLE_ID);
    expect(lists[1]).toEqual([...lists[1]!].sort());
    await expect(hidden).toBeDisabled();
    await expect(visible).toBeEnabled();
    await expect(visible).toBeChecked();
});
