import { expect, test, type Page } from "@playwright/test";
import type { DesktopLink } from "../../src/DesktopLinkDialog";

function activate(page: Page, link: DesktopLink) {
    return page.evaluate((detail) => window.dispatchEvent(new CustomEvent("fixture-desktop-link", { detail })), link);
}

test("a cold repository link prefills the address and waits for review and registration", async ({ page }) => {
    const url = "https://repository.test/index.json?b=2&a=1&a=3";
    await page.goto("/browser-harness.html?route=/projects&resource-lifecycle=1&desktop-link-fixture=repository");
    const dialog = page.locator("md-dialog[open]");
    const requests = () => page.evaluate(() => window.resourceRequests.filter((request) =>
        request.method === "repositories.inspect" || request.method === "repositories.register"));
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    await expect(dialog.getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue(url);
    await expect(dialog.getByRole("textbox", { name: "Header name", exact: true })).toHaveCount(0);
    expect(await requests()).toEqual([]);

    await dialog.getByRole("button", { name: "Review repository", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Add repository", exact: true })).toBeEnabled();
    expect(await requests()).toEqual([{ method: "repositories.inspect", params: { source: { kind: "remote", url } } }]);
    await dialog.getByRole("button", { name: "Add repository", exact: true }).click();
    await expect(dialog).toBeHidden();
    expect((await requests()).map(({ method }) => method)).toEqual(["repositories.inspect", "repositories.register"]);
});

test("an extension link waits for an open form and follows online install consent", async ({ page }) => {
    const url = "https://example.invalid/extensions.json?b=2&a=1&a=3";
    await page.goto("/browser-harness.html?route=/extensions&h4-management=1&h4-terms=1&desktop-link-fixture=empty");
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const input = dialog.getByLabel("Extension source URL", { exact: true });
    await input.fill("https://example.invalid/manual.json");
    await activate(page, { kind: "extension", url });
    await activate(page, { kind: "repository", url: "https://repository.test/later.json" });
    await expect(dialog).toHaveCount(1);
    await expect(input).toHaveValue("https://example.invalid/manual.json");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();

    await expect(input).toHaveValue(url);
    const requests = () => page.evaluate(() => window.h4Management.requests.filter(({ method }) =>
        ["extensions.inspectSource", "extensions.planInstallFromSource", "extensions.applyInstall"].includes(method)));
    expect(await requests()).toEqual([]);
    await dialog.getByRole("button", { name: "Continue", exact: true }).click();
    await expect(dialog.getByRole("heading", { name: "Online example", exact: true })).toBeVisible();
    expect(await requests()).toEqual([{ method: "extensions.inspectSource", params: { url } }]);
    await dialog.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(dialog.getByRole("heading", { name: "Extension terms", exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Agree and continue", exact: true }).click();
    await expect(dialog.getByRole("heading", { name: "Required permissions", exact: true })).toBeVisible();
    expect((await requests()).map(({ method }) => method)).toEqual(["extensions.inspectSource", "extensions.planInstallFromSource"]);
    await dialog.getByRole("button", { name: "Agree and install", exact: true }).click();

    const minimize = dialog.getByRole("button", { name: "Minimize", exact: true });
    await expect(minimize).toBeVisible();
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toHaveCount(0);
    await minimize.click();
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    await expect(dialog.getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue("https://repository.test/later.json");
    expect((await requests()).map(({ method }) => method)).toEqual([
        "extensions.inspectSource", "extensions.planInstallFromSource", "extensions.applyInstall"
    ]);
});

test("a link waits until an unsaved extension view is saved", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/extensions/com.vrclearn.discord/ui&h4-portable=1&desktop-link-fixture=empty");
    const presence = page.getByLabel("Enable presence", { exact: true });
    await presence.uncheck();
    await activate(page, { kind: "repository", url: "https://repository.test/deferred.json" });
    const dialog = page.getByRole("dialog", { name: "Add repository", exact: true });
    await expect(dialog).toHaveCount(0);
    await expect(presence).not.toBeChecked();
    await page.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue("https://repository.test/deferred.json");
});

test("ALCOM links and files open their native confirmation flows in order", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&resource-lifecycle=1&desktop-link-fixture=empty");
    const url = "https://repository.test/alcom.json";
    const link = new URL("alcom://vpm/addRepo");
    link.searchParams.set("url", url);
    link.searchParams.append("headers[]", "Authorization:fixture");
    const repositoryList = "C:/fixture/Project sources.alcomrepositories";
    const template = "C:/fixture/Unity project.alcomtemplate";
    await activate(page, { kind: "vccRepository", link: link.href });
    await activate(page, { kind: "repositoryList", path: repositoryList });
    await activate(page, { kind: "templateFile", path: template });
    const dialog = page.locator("md-dialog[open]");
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    const add = dialog;
    await expect(add.getByRole("textbox", { name: "Repository URL", exact: true })).toHaveValue(url);
    await expect(add.getByRole("textbox", { name: "Header name", exact: true })).toHaveValue("Authorization");
    expect(await page.evaluate(() => window.resourceRequests.filter(({ method }) => method === "repositories.inspect"))).toEqual([]);
    await add.getByRole("button", { name: "Cancel", exact: true }).click();

    await expect(page.getByRole("dialog", { name: "Import list", exact: true })).toBeVisible();
    const list = dialog;
    await expect(list.getByRole("textbox", { name: "Repository list file", exact: true })).toBeVisible();
    const listRequests = () => page.evaluate(() => window.resourceRequests.filter(({ method }) => method.endsWith("ListFile")));
    expect(await listRequests()).toEqual([]);
    await list.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(list.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    expect(await listRequests()).toEqual([{ method: "repositories.inspectListFile", params: { path: repositoryList } }]);
    await list.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(list.getByRole("button", { name: "Close", exact: true })).toBeVisible();
    expect((await listRequests()).map(({ method }) => method)).toEqual(["repositories.inspectListFile", "repositories.importListFile"]);
    await list.getByRole("button", { name: "Close", exact: true }).click();

    await expect(page.getByRole("dialog", { name: "Import template", exact: true })).toBeVisible();
    const importTemplate = dialog;
    await expect(importTemplate.getByRole("textbox", { name: "Template file", exact: true })).toBeVisible();
    const saves = () => page.evaluate(() => window.templateRequests.filter(({ method }) => method === "projectTemplates.save"));
    expect(await saves()).toEqual([]);
    await importTemplate.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect.poll(saves).toMatchObject([{ method: "projectTemplates.save", params: { source: { kind: "file", sourceFile: template } } }]);
});
