import { expect, test } from "@playwright/test";

test("MCP management permissions show their global scope and use the existing grant RPC", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions%2Fcom.example.alpha&state=ready&h4-management=1&m8-mcp-permissions=1");
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Extension permissions", exact: true }) });
    await expect(dialog.getByRole("region", { name: "Requested permissions", exact: true }).getByText("Read MCP clients", { exact: true })).toBeVisible();
    const select = dialog.locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: "Permission", exact: true }) });
    const events = await select.evaluateHandle((element) => ({
        opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
        closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
    }));
    try {
        await dialog.getByRole("combobox", { name: "Permission", exact: true }).click();
        await events.evaluate((value) => value.opened);
        await page.getByRole("option", { name: "Manage MCP clients", exact: true }).click();
        await events.evaluate((value) => value.closed);
    } finally { await events.dispose(); }
    await expect(dialog.getByText("All MCP clients", { exact: true })).toBeVisible();
    await expect(dialog.getByText("This permission can change grants and permanently revoke any MCP client.", { exact: true })).toBeVisible();
    await page.screenshot({ path: "target/m8-mcp-management/permissions-global-scope.png", fullPage: true });
    await expect(dialog.getByRole("combobox", { name: "Project", exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Grant", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.setGrant").at(-1)?.params)).toMatchObject({
        extensionId: "com.example.alpha", permission: "mcp.clients.manage", resourceKind: "Global", resourceId: "*"
    });
    await expect(dialog.getByRole("button", { name: "Grant", exact: true })).toBeEnabled();
    await dialog.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(dialog.locator(".extension-grants li")).toHaveCount(2);
    await dialog.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(dialog.locator(".extension-grants li")).toHaveCount(3);
    await expect(dialog.locator(".extension-grants").getByText("Manage MCP clients", { exact: true })).toBeVisible();
});
