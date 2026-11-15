import { expect, test } from "@playwright/test";
import { H4_ALPHA } from "./h4-management-client";

const route = "/browser-harness.html?route=%2Fextensions%2Fcom.example.alpha&state=ready&h4-management=1&m8-mcp-permissions=1&windows-paths=1";

test("MCP management permission rows read every grant page and use the exact global scope", async ({ page }) => {
    await page.goto(route);
    await page.evaluate((id) => window.h4Management.setGrants(id, [
        { permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101" },
        { permission: "mcp.clients.read", resourceKind: "Global", resourceId: "*" }
    ]), H4_ALPHA);
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const read = dialog.getByRole("group", { name: "Read MCP clients", exact: true });
    const manage = dialog.getByRole("group", { name: "Manage MCP clients", exact: true });
    const projects = dialog.getByRole("group", { name: "Read projects", exact: true });
    await expect(read.getByText("Allowed", { exact: true })).toBeVisible();
    await expect(manage.getByText("Not allowed", { exact: true })).toBeVisible();
    await expect(projects.locator(".extension-grants > li")).toHaveCount(1);
    await expect(dialog.getByRole("combobox", { name: "Permission", exact: true })).toHaveCount(0);
    await expect(dialog.getByRole("region", { name: "Requested permissions", exact: true })).toHaveCount(0);
    await expect(dialog.getByText("Required to enable", { exact: true })).toHaveCount(0);
    await expect(manage).toContainText("Create MCP clients, change their grants, and permanently revoke them.");
    expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").length)).toBe(0);
    await manage.getByRole("button", { name: "Allow Manage MCP clients", exact: true }).click();
    await expect(manage.getByText("Allowed", { exact: true })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").at(-1)?.params)).toEqual({
        extensionId: H4_ALPHA, permission: "mcp.clients.manage", resourceKind: "Global", resourceId: "*", expectedGrantRevision: 4
    });
    await expect(read.getByText("Allowed", { exact: true })).toBeVisible();
    await expect(projects.locator(".extension-grants > li")).toHaveCount(1);
    const reads = await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.permissions.list"));
    expect(reads.some((r) => r.params.cursor === `${H4_ALPHA}:4:1`)).toBe(true);
    expect(reads.some((r) => r.params.cursor === `${H4_ALPHA}:5:2`)).toBe(true);
    await page.screenshot({ path: "target/m8-zero-grant/permissions-explicit-scopes.png", fullPage: true });
});

for (const colorScheme of ["light", "dark"] as const) {
    test(`narrow ${colorScheme} permission dialog keeps long project scopes, keyboard actions, errors and retry usable`, async ({ browser }) => {
        const context = await browser.newContext({ viewport: { width: 640, height: 450 }, deviceScaleFactor: 2, colorScheme });
        const page = await context.newPage();
        await page.goto("http://127.0.0.1:4173" + route + "&projectGrid=1");
        await page.evaluate((id) => window.h4Management.setGrants(id, []), H4_ALPHA);
        await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        const read = dialog.getByRole("group", { name: "Read MCP clients", exact: true });
        const projects = dialog.getByRole("group", { name: "Read projects", exact: true });
        await expect(read.getByText("Not allowed", { exact: true })).toBeVisible();
        await expect(projects.getByText("Not allowed", { exact: true })).toBeVisible();
        await projects.getByRole("combobox", { name: "Project", exact: true }).click();
        const longName = "非常长的项目名称用于检查卡片信息与操作是否保持整齐 Long Project Name";
        await page.getByRole("option", { name: longName, exact: true }).click();
        await expect(projects.locator(".extension-permission-path")).toContainText("Workspace with a long directory name");
        await expect(projects).not.toContainText("\\\\?\\");
        expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").length)).toBe(0);
        const allowProject = projects.getByRole("button", { name: "Allow selected project", exact: true });
        await allowProject.focus();
        await page.keyboard.press("Enter");
        await expect(projects.getByText("Allowed projects: 1", { exact: true })).toBeVisible();
        await expect(projects.locator(".extension-grants > li > span")).toHaveText(longName);
        const remove = projects.getByRole("button", { name: `Revoke access to ${longName}`, exact: true });
        await remove.scrollIntoViewIfNeeded();
        const overflow = await dialog.locator(".extension-permissions").evaluate((element) => element.scrollWidth - element.clientWidth);
        expect(overflow).toBeLessThanOrEqual(1);
        await page.screenshot({ path: `target/m8-zero-grant/permissions-narrow-${colorScheme}.png`, fullPage: true });
        await page.evaluate(() => window.h4Management.failNext("extensions.revokeGrant", "extension_scope_denied"));
        await remove.click();
        await expect(dialog.getByRole("alert")).toBeVisible();
        await expect(dialog.getByRole("alert")).toContainText("This action is outside the extension's allowed permissions or scope. Review its access to the selected resource.");
        await expect(projects.locator(".extension-grants > li > span")).toHaveText(longName);
        await dialog.getByRole("alert").scrollIntoViewIfNeeded();
        await page.screenshot({ path: `target/m8-zero-grant/permissions-error-${colorScheme}.png`, fullPage: true });
        await remove.click();
        await expect(projects.getByText("Not allowed", { exact: true })).toBeVisible();
        await expect(projects.locator(".extension-grants > li")).toHaveCount(0);
        await expect(dialog.getByRole("alert")).toHaveCount(0);
        expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.revokeGrant").length)).toBe(2);
        await context.close();
    });
}
