import { expect, test, type Page } from "@playwright/test";
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
    await expect(dialog).toHaveCount(0);
    await expect(row(page, "com.example.avatar-tools")).toContainText("Avatar starter");
    await expect(row(page, "com.example.avatar-tools").getByRole("cell", { name: "User template", exact: true })).toBeVisible();
    await expect(row(page, avatarId)).toContainText("VRChat Avatars (2022)");
    await expect(row(page, avatarId).getByRole("cell", { name: "Built-in", exact: true })).toBeVisible();
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "manual", id: "com.example.avatar-tools", name: "Avatar starter", unityVersion: "2022.3.22f1",
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
    await expect(dialog).toHaveCount(0);
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
    const submit = dialog.getByRole("button", { name: "Import template", exact: true });
    await expect(submit).toBeDisabled();
    await dialog.getByRole("button", { name: "Choose file", exact: true }).click();
    await expect(dialog.getByLabel("Template file", { exact: true })).toHaveValue("C:\\Fixture\\input.alcomdtemplate");
    expect(await request(page, "projectTemplates.save")).toBeUndefined();
    await submit.click();
    await expect(dialog).toHaveCount(0);
    await expect(row(page, "com.example.imported")).toContainText("Imported template");
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "file", sourceFile: "C:\\Fixture\\input.alcomdtemplate" }, idempotencyKey: expect.any(String)
    });
});

test("user template replacement binds the selected revision and supports removal", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1&template-file-replace=1");
    await row(page, userId).getByRole("button", { name: "More actions for Existing fixture", exact: true }).click();
    await page.getByRole("menuitem", { name: "Replace template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog).toContainText(userId);
    await dialog.getByRole("button", { name: "Choose file", exact: true }).click();
    await dialog.getByRole("button", { name: "Replace template", exact: true }).click();
    await expect(row(page, userId)).toContainText("Imported replacement");
    expect(await request(page, "projectTemplates.save")).toEqual({
        source: { kind: "file", sourceFile: "C:\\Fixture\\input.alcomdtemplate" }, expectedRevision: 8, idempotencyKey: expect.any(String)
    });
    await row(page, userId).getByRole("button", { name: "More actions for Imported replacement", exact: true }).click();
    await page.getByRole("menuitem", { name: "Remove template", exact: true }).click();
    await dialog.getByRole("button", { name: "Remove template", exact: true }).click();
    await expect(row(page, userId)).toHaveCount(0);
});

test("built-in export submits the chosen new file path and shows task completion", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates");
    await row(page, avatarId).getByRole("button", { name: /More actions/ }).click();
    await page.getByRole("menuitem", { name: "Export template", exact: true }).click();
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
    await expect(row(page, avatarId)).toContainText("VRChat Avatars (2022)");
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
    await dialog.getByRole("button", { name: "Choose file", exact: true }).click();
    await dialog.getByRole("button", { name: "Import template", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
    await expect(dialog.getByRole("button", { name: "Choose file", exact: true })).toBeDisabled();
    await dialog.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(dialog).toHaveCount(0);
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
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name, exact: true }).click();
}

function request(page: Page, method: string) {
    return page.evaluate((name) => window.templateRequests.find((item) => item.method === name)?.params, method);
}
