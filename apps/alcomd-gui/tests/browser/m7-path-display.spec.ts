import { selectSettingsSection } from "./settings-sections";
import { expect, test } from "@playwright/test";
import { displayChildPath, displayPath } from "../../src/display-path";

test.use({ viewport: { width: 1440, height: 900 } });

const editorPath = "C:\\Program Files\\Unity\\Hub\\Editor\\2022.3.22f1\\Editor\\Unity.exe";
const networkEditor = "\\\\builds\\Editors\\Unity.exe";

test("path display formats drive and UNC labels while preserving other namespaces and characters", () => {
    const examples: [string, string][] = [
        ["\\\\?\\C:\\", "C:\\"],
        [`\\\\?\\${editorPath}`, editorPath],
        ["\\\\?\\UNC\\nas\\Share\\项目 文件", "\\\\nas\\Share\\项目 文件"],
        ["\\\\?\\unc\\nas\\Share", "\\\\nas\\Share"],
        [editorPath, editorPath],
        [networkEditor, networkEditor],
        ["/home/user/项目 文件", "/home/user/项目 文件"],
        ["\\\\?\\Volume{26a21bda-a627-11d7-9931-806e6f6e6963}\\", "\\\\?\\Volume{26a21bda-a627-11d7-9931-806e6f6e6963}\\"],
        ["\\\\.\\pipe\\alcomd", "\\\\.\\pipe\\alcomd"],
        ["\\\\?\\GLOBALROOT\\Device\\HarddiskVolume1", "\\\\?\\GLOBALROOT\\Device\\HarddiskVolume1"],
        ["\\\\?\\C:relative", "\\\\?\\C:relative"],
        ["\\\\?\\UNC\\nas", "\\\\?\\UNC\\nas"],
        ["https://example.invalid/path?value=1", "https://example.invalid/path?value=1"]
    ];
    for (const [original, expected] of examples) expect(displayPath(original)).toBe(expected);
    for (const original of [
        "\\\\?\\C:\\项目 文件\\folder. ",
        "\\\\?\\C:\\folder.\\Unity.exe",
        "\\\\?\\C:\\folder\\..\\Unity.exe",
        "\\\\?\\C:\\folder//Unity.exe",
        "\\\\?\\C:\\folder\\\\Unity.exe",
        "\\\\?\\C:\\folder\\NUL.txt",
        "\\\\?\\C:\\folder\\COM¹.txt",
        "\\\\?\\C:\\folder\\name:stream",
        "\\\\?\\UNC\\nas\\\\share",
        "\\\\?\\C:\\" + "😀".repeat(127) + "\\xx"
    ]) expect(displayPath(original)).toBe(original);
    expect(displayChildPath("\\\\?\\C:\\", "Project")).toBe("C:\\Project");
    expect(displayChildPath("/home/user/Projects", "Project")).toBe("/home/user/Projects/Project");
});

test("settings show supplied paths without rewriting them or sending edits on focus", async ({ page }) => {
    const hub = "\\\\?\\C:\\Tools\\Unity Hub.exe";
    await page.goto("/browser-harness.html?route=/settings&windows-paths=1&hub-path=" + encodeURIComponent(hub));
    const directory = page.locator("#settings-projectDirectory").getByRole("textbox");
    await expect(directory).toHaveValue("C:\\Projects");
    await directory.fill("C:\\Projects2");
    await directory.fill("C:\\Projects");
    await directory.press("Enter");
    await selectSettingsSection(page, "unity");
    const hubField = page.locator("#settings-unity-hub-path").getByRole("textbox");
    await expect(hubField).toHaveValue("C:\\Tools\\Unity Hub.exe");
    await hubField.focus();
    await page.getByRole("button", { name: "Open Unity Hub", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.unityRequests.find((request) => request.method === "unityHubOpen")?.params))
        .toMatchObject({ expectedPath: hub });
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
});

test("Unity installation rows and full-path tooltips show drive and network labels", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings&state=unity-multiple&windows-paths=1");
    await selectSettingsSection(page, "unity");
    const table = page.getByRole("table", { name: "Unity installations", exact: true });
    await expect(table.getByTitle(editorPath, { exact: true })).toHaveText(editorPath);
    await expect(table.getByTitle(networkEditor, { exact: true })).toHaveText(networkEditor);
    await expect(table).not.toContainText("\\\\?\\");
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Discover and refresh", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.unityRequests.filter((request) => request.method === "unityInstallationsRefresh").length)).toBe(1);
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(table.getByTitle(editorPath, { exact: true })).toBeVisible();
    await expect(table.getByTitle(networkEditor, { exact: true })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test("Unity launch choices display friendly paths and submit only the selected installation identity", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=unity-multiple&windows-paths=1");
    await page.getByRole("button", { name: "Open Unity", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("combobox", { name: "Unity installation for this launch", exact: true }).click();
    await page.getByRole("option", { name: `Unity 2022.3.22f1 · x86_64 · ${networkEditor}`, exact: true }).click();
    await expect(dialog).not.toContainText("\\\\?\\");
    await dialog.getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toEqual([
        { method: "unity.launch", params: { projectId: "00000000-0000-4000-8000-000000000101", installationId: "00000000-0000-4000-8000-000000000111", expectedProjectRevision: 2 } }
    ]);
});

test("project paths and copy review use display labels while the plan request keeps the raw parent", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&windows-paths=1");
    await expect(page.getByTitle("C:\\Projects\\Sample", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "More actions for Sample", exact: true }).click();
    await page.getByRole("menuitem", { name: "Copy Project", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("textbox", { name: "Copy project location", exact: true })).toHaveValue("C:\\Projects");
    await dialog.getByRole("button", { name: "Review copy", exact: true }).click();
    await expect(dialog).toContainText("C:\\Projects\\Sample Copy");
    await expect(dialog).not.toContainText("\\\\?\\");
    expect(await page.evaluate(() => window.h3Requests.filter((request) => request.method === "projects.planCopy"))).toEqual([
        { method: "projects.planCopy", params: { sourceProjectId: "00000000-0000-4000-8000-000000000101", expectedRevision: 2, targetParentPath: "\\\\?\\C:\\Projects", targetLeaf: "Sample Copy" } }
    ]);
    await dialog.getByRole("button", { name: "Back", exact: true }).click();
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
});

test("create shows the default project directory without changing its request", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&windows-paths=1");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const parent = page.getByRole("textbox", { name: "Create target parent", exact: true });
    await expect(parent).toHaveValue("C:\\Projects");
    await page.getByRole("textbox", { name: "Create project name", exact: true }).fill("Path Fixture");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Create project", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.templateRequests.find((request) => request.method === "projects.create")?.params))
        .toMatchObject({ targetParent: "\\\\?\\C:\\Projects" });
});

test("user package source folders use the same UNC display rule", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&windows-paths=1");
    await page.getByRole("button", { name: "Details", exact: true }).click();
    await expect(page.locator("dd").filter({ hasText: "\\\\nas\\Packages\\Avatar" })).toBeVisible();
    await expect(page.locator("dd").filter({ hasText: "\\\\?\\" })).toHaveCount(0);
});

test("restore review displays the destination without modifying its plan request", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/backups/00000000-0000-4000-8000-000000000104&windows-paths=1");
    await page.getByRole("button", { name: "Restore backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const parent = dialog.getByRole("textbox", { name: "Restore target parent", exact: true });
    await parent.fill("\\\\?\\C:\\Restores");
    await expect(parent).toHaveValue("\\\\?\\C:\\Restores");
    await dialog.getByRole("textbox", { name: "Restore project name", exact: true }).fill("Restored");
    await expect(parent).toHaveValue("C:\\Restores");
    await dialog.getByRole("button", { name: "Review restore", exact: true }).click();
    await expect(dialog).toContainText("C:\\Restores\\Restored");
    await expect(dialog).not.toContainText("\\\\?\\");
    expect(await page.evaluate(() => window.h3Requests.filter((request) => request.method === "backups.planRestore"))).toEqual([
        { method: "backups.planRestore", params: { backupId: "00000000-0000-4000-8000-000000000104", targetParent: "\\\\?\\C:\\Restores", targetLeaf: "Restored" } }
    ]);
});
