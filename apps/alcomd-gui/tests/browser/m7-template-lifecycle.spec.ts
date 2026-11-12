import { expect, test, type Locator, type Page } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { expectRequestError } from "./request-error-assertions";

const avatarId = "com.vrclearn.alcomd.templates.vrchat.avatars.2022";
const userId = "com.example.fixture";

test("manual creation submits one exact editor and the package list as a tracked operation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const dialog = await create(page, "com.example.avatar-tools", "Avatar starter");
    const packages = dialog.getByRole("group", { name: "Packages", exact: true });
    await packages.getByRole("button", { name: "Add package", exact: true }).click();
    await packages.getByLabel("Package ID", { exact: true }).fill("com.example.avatar");
    await packages.getByLabel("Version", { exact: true }).fill("^1.2.3");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await closeCompletedTemplate(dialog);
    await expect(row(page, "com.example.avatar-tools")).toContainText("Avatar starter");
    await expect(row(page, "com.example.avatar-tools").getByRole("cell", { name: "User template", exact: true })).toBeVisible();
    await expect(row(page, avatarId)).toContainText("VRChat Avatars 2022");
    await expect(row(page, avatarId).getByRole("cell", { name: "Built-in", exact: true })).toBeVisible();
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "manual", id: "com.example.avatar-tools", name: "Avatar starter", unityVersion: "2022.3.22f1",
            baseTemplate: { id: "com.vrclearn.alcomd.templates.blank", revision: 1 },
            dependencies: [{ packageId: "com.example.avatar", versionRange: "^1.2.3" }] },
        idempotencyKey: expect.any(String)
    });
});

test("project source sends the selected revision and both explicit content choices", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await openSource(page, "Create template from project");
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Template ID", { exact: true }).fill("com.example.from-project");
    await dialog.getByLabel("Display name", { exact: true }).fill("From my project");
    await dialog.getByRole("combobox", { name: "Source project", exact: true }).click();
    await dialog.getByRole("option", { name: /private-project/ }).click();
    const includeAssets = dialog.getByRole("checkbox", { name: "Include project assets" });
    const lockVersions = dialog.getByRole("checkbox", { name: "Lock package versions" });
    await expect(includeAssets).not.toBeChecked();
    await expect(lockVersions).not.toBeChecked();
    await includeAssets.check();
    await lockVersions.check();
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await closeCompletedTemplate(dialog);
    await expect(row(page, "com.example.from-project")).toContainText("From my project");
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "project", id: "com.example.from-project", name: "From my project",
            projectId: "00000000-0000-4000-8000-000000000101", expectedProjectRevision: 2, includeAssets: true, lockPackageVersions: true },
        idempotencyKey: expect.any(String)
    });
});

test("file import waits for a selected archive and an explicit confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await openSource(page, "Import template");
    const dialog = page.locator("md-dialog[open]");
    const submit = dialog.getByRole("button", { name: "Confirm", exact: true });
    await expect(submit).toBeDisabled();
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(dialog.getByLabel("Template file", { exact: true })).toHaveValue("C:\\Fixture\\input.alcomdtemplate");
    expect(await request(page, "projectTemplates.save")).toBeUndefined();
    await submit.click();
    await closeCompletedTemplate(dialog);
    await expect(row(page, "com.example.imported")).toContainText("Imported template");
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "file", sourceFile: "C:\\Fixture\\input.alcomdtemplate" }, idempotencyKey: expect.any(String)
    });
});

test("template file picker keeps one editable path for typing and targeted drops", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/browser-harness.html?route=/templates");
    await openSource(page, "Import template");
    const dialog = page.locator("md-dialog[open]");
    const input = dialog.getByLabel("Template file", { exact: true });
    const target = dialog.getByRole("button", { name: "Drop a file here or click to select", exact: true });
    await target.click({ position: { x: 12, y: 12 } });
    await expect(input).toHaveValue("C:\\Fixture\\input.alcomdtemplate");
    await input.fill("C:\\Fixture\\typed.alcomdtemplate");
    const box = await target.boundingBox();
    if (box === null) throw new Error("Missing file drop zone");
    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;
    const dropped = "C:\\Fixture\\dropped.alcomdtemplate";
    await page.evaluate(({ x, y, dropped }) => {
        window.dispatchEvent(new CustomEvent("fixture-file-drag", { detail: { type: "drop", x: 0, y: 0, paths: [dropped] } }));
        window.dispatchEvent(new CustomEvent("fixture-file-drag", { detail: { type: "hover", x, y } }));
    }, { x, y, dropped });
    await expect(input).toHaveValue("C:\\Fixture\\typed.alcomdtemplate");
    await expect(dialog.getByText("Release to select the file", { exact: true })).toBeVisible();
    await page.evaluate(({ x, y, dropped }) => window.dispatchEvent(new CustomEvent("fixture-file-drag", {
        detail: { type: "drop", x, y, paths: [dropped, "C:\\Fixture\\second.alcomdtemplate"] }
    })), { x, y, dropped });
    await expect(dialog.getByRole("alert")).toHaveText("Select one file at a time.");
    await expect(input).toHaveValue("C:\\Fixture\\typed.alcomdtemplate");
    await page.evaluate(({ x, y, dropped }) => window.dispatchEvent(new CustomEvent("fixture-file-drag", {
        detail: { type: "drop", x, y, paths: [dropped] }
    })), { x, y, dropped });
    await expect(input).toHaveValue(dropped);
    expect(await request(page, "projectTemplates.save")).toBeUndefined();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    expect(await request(page, "projectTemplates.save")).toMatchObject({ source: { kind: "file", sourceFile: dropped } });
});

test("user template replacement binds the selected revision and supports removal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1&template-file-replace=1");
    await selectActionMenuItem(row(page, userId).getByRole("button", { name: "More actions for Existing fixture", exact: true }), page.getByRole("menuitem", { name: "Replace template", exact: true }));
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog).toContainText(userId);
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await dialog.getByRole("button", { name: "Replace template", exact: true }).click();
    await closeCompletedTemplate(dialog);
    await expect(row(page, userId)).toContainText("Imported replacement");
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "file", sourceFile: "C:\\Fixture\\input.alcomdtemplate" }, expectedRevision: 8, idempotencyKey: expect.any(String)
    });
    await selectActionMenuItem(row(page, userId).getByRole("button", { name: "More actions for Imported replacement", exact: true }), page.getByRole("menuitem", { name: "Remove template", exact: true }));
    await dialog.getByRole("button", { name: "Remove template", exact: true }).click();
    await expect(row(page, userId)).toHaveCount(0);
});

test("built-in export submits the chosen new file path and shows task completion", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await selectActionMenuItem(row(page, avatarId).getByRole("button", { name: /More actions/ }), page.getByRole("menuitem", { name: "Export template", exact: true }));
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("File name", { exact: true }).fill("avatars.alcomdtemplate");
    await dialog.getByRole("button", { name: "Choose directory", exact: true }).click();
    await expect(dialog.getByLabel("Destination file", { exact: true })).toHaveValue("C:\\Fixture\\avatars.alcomdtemplate");
    await dialog.getByRole("button", { name: "Export template", exact: true }).click();
    await expect(dialog.getByRole("status")).toContainText("succeeded");
    expect(await request(page, "projectTemplates.export")).toEqual({
        id: avatarId, revision: 2, destinationFile: "C:\\Fixture\\avatars.alcomdtemplate", idempotencyKey: expect.any(String)
    });
});

test("saving a different template under an existing ID reports a task conflict", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    const dialog = await create(page, avatarId, "Unrelated template");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "template_conflict");
    await dialog.getByRole("button", { name: "Back", exact: true }).click();
    await expect(dialog.getByLabel("Display name", { exact: true })).toHaveValue("Unrelated template");
    await expect(row(page, avatarId)).toContainText("VRChat Avatars 2022");
});

test("a failed template operation shows the package reported by Core", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-package-error=1");
    const dialog = await create(page, "com.example.package-error", "Package error fixture");
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "template_dependency_unsatisfied");
    await expect(dialog.getByText("com.example.missing", { exact: true })).toBeVisible();
    await expect(row(page, "com.example.package-error")).toHaveCount(0);
});

test("retrying a lost import response sends the same file and idempotency key", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-operation-unknown=1");
    await openSource(page, "Import template");
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
    await expect(dialog.getByRole("button", { name: "Select", exact: true })).toBeDisabled();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await closeCompletedTemplate(dialog);
    await expect(row(page, "com.example.imported")).toHaveCount(1);
    const writes = await page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projectTemplates.save"));
    expect(writes).toHaveLength(2);
    expect(writes[1]).toEqual(writes[0]);
});

function row(page: Page, id: string) {
    return page.getByRole("row").filter({ has: page.getByRole("cell", { name: id, exact: true }) });
}

async function create(page: Page, id: string, name: string) {
    await page.getByRole("button", { name: "Create template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Template ID", { exact: true }).fill(id);
    await dialog.getByLabel("Display name", { exact: true }).fill(name);
    return dialog;
}

async function openSource(page: Page, name: string) {
    await selectActionMenuItem(page.getByRole("button", { name: "More template creation options", exact: true }), page.getByRole("menuitem", { name, exact: true }));
}

function request(page: Page, method: string) {
    return page.evaluate((name) => window.templateRequests.find((item) => item.method === name)?.params, method);
}

test("manual templates default to Blank and inherit a chosen base's fixed Editor", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await expect(row(page, "com.vrclearn.alcomd.templates.blank")).toContainText("Blank");
    const dialog = await create(page, "com.example.derived", "Derived");
    const base = dialog.getByRole("combobox", { name: "Base template", exact: true });
    await expect(base).toContainText("Blank");
    await base.click();
    await dialog.getByRole("option", { name: "VRChat Avatars 2019 · 2019.4.31f1", exact: true }).click();
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveValue("2019.4.31f1");
    await expect(dialog.getByRole("textbox", { name: "Unity version", exact: true })).toHaveJSProperty("readOnly", true);
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await closeCompletedTemplate(dialog);
    expect(await request(page, "projectTemplates.save")).toMatchObject({ source: {
        kind: "manual", id: "com.example.derived", unityVersion: "2019.4.31f1",
        baseTemplate: { id: "com.vrclearn.alcomd.templates.vrchat.avatars.2019", revision: 3 }
    } });
});

test("manual base pagination preserves the draft on failure and accepts user templates from later pages", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-pages=1&template-page-error=1");
    const dialog = await create(page, "com.example.from-page", "Kept draft");
    await dialog.getByRole("button", { name: "Load more templates", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("Your loaded items are still available.");
    await expect(dialog.getByLabel("Display name", { exact: true })).toHaveValue("Kept draft");
    await dialog.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await dialog.getByRole("combobox", { name: "Base template", exact: true }).click();
    await dialog.getByRole("option", { name: /^Catalog template 101/ }).click();
    await expect(dialog.getByRole("button", { name: "Save template", exact: true })).toBeEnabled();
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    expect(await request(page, "projectTemplates.save")).toMatchObject({ source: { baseTemplate: { id: "com.example.catalog-101", revision: 111 } } });
});

test("a changed base revision preserves the manual draft and requires explicit reselection", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1");
    const dialog = await create(page, "com.example.kept-draft", "Kept manual draft");
    const base = dialog.getByRole("combobox", { name: "Base template", exact: true });
    const choose = async () => { await base.click(); await dialog.getByRole("option", { name: /Existing fixture/ }).click(); };
    await choose();
    await page.evaluate(() => window.dispatchEvent(new Event("test-template-advance")));
    await expect(dialog.getByRole("alert")).toContainText("The selected template changed. Select it again.");
    await expect(dialog.getByLabel("Display name", { exact: true })).toHaveValue("Kept manual draft");
    await expect(dialog.getByRole("button", { name: "Save template", exact: true })).toBeDisabled();
    await choose();
    await dialog.getByRole("button", { name: "Save template", exact: true }).click();
    await closeCompletedTemplate(dialog);
    expect(await request(page, "projectTemplates.save")).toMatchObject({ source: { baseTemplate: { id: "com.example.fixture", revision: 9 } } });
});

for (const [width, theme] of [[1280, "light"], [640, "dark"]] as const) {
    test(`manual base selection remains usable in Chinese at ${width}px in ${theme} mode`, async ({ page }, testInfo) => {
        await page.setViewportSize({ width, height: 900 });
        await page.emulateMedia({ reducedMotion: "reduce" });
        await page.goto(`/browser-harness.html?route=/templates&locale=zh-CN&theme-mode=${theme}`);
        await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
        await page.getByRole("button", { name: "创建模板", exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        const base = dialog.getByRole("combobox", { name: "基础模板", exact: true });
        await expect(base).toContainText("空白");
        await expect(dialog.getByText("基础模板", { exact: true })).toBeVisible();
        await dialog.getByLabel("模板 ID", { exact: true }).fill("com.example.chinese");
        await dialog.getByLabel("显示名称", { exact: true }).fill("我的基础模板");
        await base.click();
        await dialog.getByRole("option", { name: "VRChat Avatars 2022 · 2022.3.22f1", exact: true }).click();
        await expect(dialog.getByRole("button", { name: "保存模板", exact: true })).toBeEnabled();
        await page.screenshot({ path: testInfo.outputPath(`template-base-${width}-${theme}.png`), animations: "disabled" });
    });
}

async function closeCompletedTemplate(dialog: Locator) {
    await expect(dialog.getByRole("status")).toContainText("succeeded");
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
}
