import { expect, test, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";
const defaultsSection = (page: Page) => page.getByRole("region", { name: "Default Unity launch arguments", exact: true });
const defaultsDialog = (page: Page) => page.getByRole("dialog", { name: "Default Unity launch arguments", exact: true });
// Material slots the editor from light DOM into its native shadow-root dialog.
const defaultsHost = (page: Page) => page.locator("md-dialog").filter({ has: page.locator('[slot="headline"]').filter({ hasText: /^Default Unity launch arguments$/ }) });
const projectDialog = (page: Page) => page.getByRole("dialog", { name: "Unity launch arguments", exact: true });
const literalArguments = ["-logFile", "C:\\Logs with spaces\\Editor.log", "", "  keep leading and trailing spaces  ", 'literal "quotes" and \\backslashes'];
const writes = (page: Page) => page.evaluate(() => window.settingsRequests);

async function openSettings(page: Page, query: Record<string, string> = {}) {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto(`/browser-harness.html?${new URLSearchParams({ route: "/settings", ...query })}`);
    await selectSettingsSection(page, "unity");
    await expect(defaultsSection(page)).toBeVisible();
}
async function editDefaults(page: Page) {
    await defaultsSection(page).getByRole("button", { name: "Edit", exact: true }).click();
    await expect(defaultsDialog(page)).toBeVisible();
}
async function chooseSource(page: Page, name: "Use global defaults" | "Custom arguments") {
    await page.getByRole("combobox", { name: "Launch arguments", exact: true }).click();
    await page.getByRole("option", { name, exact: true }).click();
}
async function openProject(page: Page, query: Record<string, string> = {}) {
    await page.setViewportSize({ width: 1400, height: 900 });
    await page.goto(`/browser-harness.html?${new URLSearchParams({ route: `/projects/${PROJECT_ID}/unity`, ...query })}`);
    await page.getByRole("button", { name: "Edit launch arguments", exact: true }).click();
    await expect(projectDialog(page)).toBeVisible();
}

test("default Unity arguments preserve argv values and Cancel discards edits", async ({ page }) => {
    await openSettings(page, { "default-arguments": JSON.stringify(literalArguments) });
    await editDefaults(page);
    for (let index = 0; index < literalArguments.length; index += 1) {
        await expect(page.getByLabel(`Argument ${index + 1}`, { exact: true })).toHaveValue(literalArguments[index]!);
    }
    await page.getByLabel("Argument 2", { exact: true }).fill("discard this");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    expect(await writes(page)).toEqual([]);
    await editDefaults(page);
    await expect(page.getByLabel("Argument 2", { exact: true })).toHaveValue(literalArguments[1]!);
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    expect((await writes(page)).map((request) => request.update)).toEqual([{ unity: { defaultLaunchArguments: [...literalArguments, ""] } }]);
});

test("null defaults and an explicit empty array remain distinct and can be restored", async ({ page }) => {
    await openSettings(page);
    await editDefaults(page);
    await expect(page.getByRole("button", { name: "Restore defaults", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByRole("button", { name: "Remove argument 1", exact: true }).click();
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    await editDefaults(page);
    await page.getByRole("button", { name: "Restore defaults", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    expect((await writes(page)).map((request) => request.update)).toEqual([{ unity: { defaultLaunchArguments: [] } }, { unity: { defaultLaunchArguments: null } }]);
});

test("a lost default-argument save response is reconciled without another write", async ({ page }) => {
    await openSettings(page, { "arguments-save": "commit-error-once" });
    await editDefaults(page);
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByLabel("Argument 1", { exact: true }).fill("-batchmode");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    await expect(page.getByRole("status").filter({ hasText: "Default Unity launch arguments saved." })).toBeVisible();
    expect(await writes(page)).toHaveLength(1);
});

test("unknown defaults remain editable and explicit Retry reads before another write", async ({ page }) => {
    await openSettings(page, { "arguments-save": "commit-read-error-once" });
    await editDefaults(page);
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByLabel("Argument 1", { exact: true }).fill("-batchmode");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeVisible();
    await expect(defaultsHost(page).getByRole("button", { name: "Retry", exact: true })).toBeVisible();
    await expect(defaultsHost(page).getByText("Launch argument save result is unknown", { exact: true })).toBeVisible();
    await expect(page.getByRole("status").filter({ hasText: "Default Unity launch arguments saved." })).toBeHidden();
    const readsBefore = await page.evaluate(() => window.settingsReads);
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    expect(await page.evaluate(() => window.settingsReads)).toBeGreaterThan(readsBefore);
    expect(await writes(page)).toHaveLength(1);
});

test("failed defaults keep the draft and permission denial is visible", async ({ page }) => {
    await openSettings(page, { "arguments-save": "denied" });
    await editDefaults(page);
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByLabel("Argument 1", { exact: true }).fill("-batchmode");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeVisible();
    await expect(defaultsHost(page).getByText("Launch arguments could not be saved.", { exact: true })).toBeVisible();
    await expect(defaultsHost(page).getByRole("alert")).toContainText("You do not have permission for this action.");
    await expect(page.getByLabel("Argument 1", { exact: true })).toHaveValue("-batchmode");
    await expect(page.getByRole("status").filter({ hasText: "Default Unity launch arguments saved." })).toBeHidden();
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
    await openSettings(page, { noUnityManage: "1" });
    await expect(defaultsSection(page).getByRole("button", { name: "Edit", exact: true })).toBeDisabled();
});

test("a competing default edit is shown without overwriting it", async ({ page }) => {
    await openSettings(page, { "arguments-save": "conflict-once" });
    await editDefaults(page);
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByLabel("Argument 1", { exact: true }).fill("-nographics");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(defaultsDialog(page)).toBeVisible();
    await expect(defaultsHost(page).getByText("These arguments changed elsewhere. Close and reopen this editor to load the current values.", { exact: true })).toBeVisible();
    expect(await writes(page)).toHaveLength(1);
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(defaultsDialog(page)).toBeHidden();
    await editDefaults(page);
    await expect(page.getByLabel("Argument 1", { exact: true })).toHaveValue("-batchmode");
});

test("project inheritance uses Core defaults and explicit empty custom arguments use set", async ({ page }) => {
    await openProject(page, { "default-arguments": JSON.stringify(literalArguments), "launch-inherit": "1" });
    await expect(page.getByRole("combobox", { name: "Launch arguments", exact: true })).toContainText("Use global defaults");
    await expect(page.getByLabel("Argument 2", { exact: true })).toHaveValue(literalArguments[1]!);
    await chooseSource(page, "Custom arguments");
    for (let index = literalArguments.length; index > 0; index -= 1) await page.getByRole("button", { name: `Remove argument ${index}`, exact: true }).click();
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(projectDialog(page)).toBeHidden();
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.projectLaunchConfig.set"))).toEqual([{ method: "unity.projectLaunchConfig.set", params: { projectId: PROJECT_ID, arguments: [], expectedRevision: 0 } }]);
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Edit launch arguments", exact: true }).click();
    await expect(page.getByRole("combobox", { name: "Launch arguments", exact: true })).toContainText("Custom arguments");
    await expect(page.getByText("No additional arguments.", { exact: true })).toBeVisible();
    await chooseSource(page, "Use global defaults");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(projectDialog(page)).toBeHidden();
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.projectLaunchConfig.clear"))).toEqual([{ method: "unity.projectLaunchConfig.clear", params: { projectId: PROJECT_ID, expectedRevision: 1 } }]);
});

test("project argv fields preserve empty values, spaces and literal quotes", async ({ page }) => {
    await openProject(page, { "project-arguments": JSON.stringify(literalArguments) });
    await page.getByRole("button", { name: "Add argument", exact: true }).click();
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(projectDialog(page)).toBeHidden();
    expect(await page.evaluate(() => window.unityRequests.find((request) => request.method === "unity.projectLaunchConfig.set")?.params)).toEqual({ projectId: PROJECT_ID, arguments: [...literalArguments, ""], expectedRevision: 2 });
});

test("long argv values fit the desktop dialog without horizontal overflow", async ({ page }, testInfo) => {
    await openSettings(page, { "default-arguments": JSON.stringify(["-logFile", `C:\\${"Long directory with spaces\\".repeat(20)}Editor.log`]) });
    await editDefaults(page);
    await expect(page.getByLabel("Argument 2", { exact: true })).toHaveValue(`C:\\${"Long directory with spaces\\".repeat(20)}Editor.log`);
    const bounds = await defaultsDialog(page).evaluate((element) => ({ scroll: element.scrollWidth, width: element.clientWidth }));
    expect(bounds.scroll).toBeLessThanOrEqual(bounds.width + 1);
    await page.screenshot({ path: testInfo.outputPath("unity-arguments-long-path.png"), animations: "disabled" });
});
