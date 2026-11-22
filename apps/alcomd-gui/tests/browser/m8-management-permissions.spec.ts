import { expect, test } from "@playwright/test";
import { H4_ALPHA } from "./h4-management-client";
import { confirmPermissionChange } from "./permission-switch-actions";

const route = "/browser-harness.html?route=%2Fextensions%2Fcom.example.alpha&state=ready&h4-management=1&m8-mcp-permissions=1&windows-paths=1";

test("permission switches use keyboard input, lock pending writes and restore authority after failure", async ({ page }) => {
    await page.goto(route);
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const manage = dialog.getByRole("switch", { name: "Manage MCP clients", exact: true });
    const read = dialog.getByRole("switch", { name: "Read MCP clients", exact: true });
    await expect(manage).not.toBeChecked();
    await expect(read).not.toBeChecked();
    await page.evaluate(() => window.h4Management.hold("extensions.setGrant", true));
    await manage.focus();
    await page.keyboard.press("Space");
    await expect(manage).toBeDisabled();
    await expect(read).toBeDisabled();
    expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").length)).toBe(1);
    await page.evaluate(() => {
        window.h4Management.hold("extensions.setGrant", false);
        window.h4Management.release("extensions.setGrant", "extension_permission_denied");
    });
    await expect(dialog.getByRole("alert")).toContainText("This action is outside the extension's allowed permissions or scope.");
    await expect(manage).toBeEnabled();
    await expect(manage).not.toBeChecked();
    await expect(read).not.toBeChecked();
    await confirmPermissionChange(page, "extensions.setGrant", () => manage.click());
    await expect(manage).toBeChecked();
    await expect(manage).toBeEnabled();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    await expect(read).not.toBeChecked();
    await page.evaluate(() => window.h4Management.failNext("extensions.revokeGrant", "store_unavailable"));
    await manage.click();
    await expect(dialog.getByRole("alert")).toContainText("Core could not read or save its state.");
    await expect(manage).toBeEnabled();
    await expect(manage).toBeChecked();
    await confirmPermissionChange(page, "extensions.revokeGrant", () => manage.click());
    await expect(manage).not.toBeChecked();
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    const writes = await page.evaluate(() => window.h4Management.requests.filter((r) => ["extensions.setGrant", "extensions.revokeGrant"].includes(r.method)));
    expect(writes.map((r) => [r.method, r.params.permission, r.params.resourceKind, r.params.resourceId, r.params.expectedGrantRevision])).toEqual([
        ["extensions.setGrant", "mcp.clients.manage", "Global", "*", 3],
        ["extensions.setGrant", "mcp.clients.manage", "Global", "*", 3],
        ["extensions.revokeGrant", "mcp.clients.manage", "Global", "*", 4],
        ["extensions.revokeGrant", "mcp.clients.manage", "Global", "*", 4]
    ]);
});

test("permission reload never exposes old writable switches after a successful grant", async ({ page }) => {
    await page.goto(route);
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const manage = dialog.getByRole("switch", { name: "Manage MCP clients", exact: true });
    await expect(manage).not.toBeChecked();
    await expect(manage).toBeEnabled();
    await page.evaluate(() => window.h4Management.hold("extensions.setGrant", true));
    await manage.click();
    await expect(manage).toBeDisabled();
    await page.evaluate(() => window.h4Management.hold("extensions.permissions.list", true));
    const observed = await dialog.evaluateHandle((element) => {
        const state = { writable: false, observer: undefined as MutationObserver | undefined };
        state.observer = new MutationObserver(() => {
            for (const control of element.querySelectorAll("md-switch")) {
                if (!(control as HTMLElement & { disabled: boolean }).disabled) state.writable = true;
            }
        });
        state.observer.observe(element, { subtree: true, childList: true, attributes: true, attributeFilter: ["disabled"] });
        return state;
    });
    let writable: boolean;
    try {
        await page.evaluate(() => {
            window.h4Management.hold("extensions.setGrant", false);
            window.h4Management.release("extensions.setGrant");
        });
        await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBeGreaterThan(0);
        writable = await observed.evaluate((state) => state.writable);
    } finally {
        await observed.evaluate((state) => state.observer?.disconnect());
        await observed.dispose();
        await page.evaluate(() => {
            window.h4Management.hold("extensions.permissions.list", false);
            while (window.h4Management.release("extensions.permissions.list")) { /* Release only held read responses. */ }
        });
    }
    expect(writable).toBe(false);
    await expect(manage).toBeChecked();
    await expect(manage).toBeEnabled();
});

test("permission readback retains disabled rows without moving the next pointer target", async ({ page }) => {
    await page.goto(route);
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const manage = dialog.getByRole("switch", { name: "Manage MCP clients", exact: true });
    await expect(manage).not.toBeChecked();
    await expect(manage).toBeEnabled();
    await page.evaluate(() => window.h4Management.hold("extensions.setGrant", true));
    await manage.click();
    await expect(manage).toBeDisabled();
    // Measure readback from the accepted, held write after the opening click has settled.
    const before = await manage.boundingBox();
    expect(before).not.toBeNull();
    await page.evaluate(() => {
        window.h4Management.hold("extensions.permissions.list", true);
        window.h4Management.hold("extensions.setGrant", false);
        window.h4Management.release("extensions.setGrant");
    });
    try {
        await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBeGreaterThan(0);
        await expect(dialog.getByRole("switch")).toHaveCount(12);
        await expect(manage).toBeDisabled();
        const pending = await manage.boundingBox();
        expect(pending).not.toBeNull();
        expect(Math.abs(pending!.y - before!.y)).toBeLessThanOrEqual(1);
    } finally {
        await page.evaluate(() => {
            window.h4Management.hold("extensions.permissions.list", false);
            while (window.h4Management.release("extensions.permissions.list")) { /* Deliver existing readbacks only. */ }
        });
    }
    await expect(manage).toBeChecked();
    await expect(manage).toBeEnabled();
    const after = await manage.boundingBox();
    expect(after).not.toBeNull();
    expect(Math.abs(after!.y - before!.y)).toBeLessThanOrEqual(1);
    await manage.click();
    await expect(manage).not.toBeChecked();
    expect(await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.revokeGrant").length)).toBe(1);
});

test("MCP management permission rows read every grant page and use the exact global scope", async ({ page }) => {
    await page.goto(route);
    await page.evaluate((id) => window.h4Management.setGrants(id, [
        { permission: "projects.read", resourceKind: "Global", resourceId: "*" },
        { permission: "mcp.clients.read", resourceKind: "Global", resourceId: "*" }
    ]), H4_ALPHA);
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const read = dialog.getByRole("group", { name: "Read MCP clients", exact: true });
    const manage = dialog.getByRole("group", { name: "Manage MCP clients", exact: true });
    const projects = dialog.getByRole("group", { name: "Read projects", exact: true });
    await expect(read.getByRole("switch", { name: "Read MCP clients", exact: true })).toBeChecked();
    await expect(manage.getByRole("switch", { name: "Manage MCP clients", exact: true })).not.toBeChecked();
    await expect(projects.getByRole("switch", { name: "Read projects", exact: true })).toBeChecked();
    await expect(dialog.getByRole("combobox", { name: "Permission", exact: true })).toHaveCount(0);
    await expect(dialog.getByRole("region", { name: "Requested permissions", exact: true })).toHaveCount(0);
    await expect(dialog.getByRole("region", { name: "Required permissions", exact: true }).getByText("None", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("region", { name: "Optional permissions", exact: true }).getByRole("switch")).toHaveCount(3);
    await expect(dialog.getByRole("region", { name: "Other permissions", exact: true }).getByRole("switch")).toHaveCount(9);
    await expect(dialog.getByText("Required", { exact: true })).toHaveCount(0);
    await expect(manage).toContainText("Create and permanently revoke MCP clients.");
    expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").length)).toBe(0);
    await confirmPermissionChange(page, "extensions.setGrant", () => manage.getByRole("switch", { name: "Manage MCP clients", exact: true }).click());
    await expect(manage.getByRole("switch", { name: "Manage MCP clients", exact: true })).toBeChecked();
    await expect.poll(() => page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.setGrant").at(-1)?.params)).toEqual({
        extensionId: H4_ALPHA, permission: "mcp.clients.manage", resourceKind: "Global", resourceId: "*", expectedGrantRevision: 4
    });
    await expect(read.getByRole("switch", { name: "Read MCP clients", exact: true })).toBeChecked();
    await expect(projects.getByRole("switch", { name: "Read projects", exact: true })).toBeChecked();
    const reads = await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.permissions.list"));
    expect(reads.some((r) => r.params.cursor === `${H4_ALPHA}:4:1`)).toBe(true);
    expect(reads.some((r) => r.params.cursor === `${H4_ALPHA}:5:2`)).toBe(true);
    await page.screenshot({ path: "target/m8-zero-grant/permissions-explicit-scopes.png", fullPage: true });
});

for (const colorScheme of ["light", "dark"] as const) {
    test(`narrow ${colorScheme} permission switches keep keyboard, error feedback and retry usable`, async ({ browser, baseURL }) => {
        const context = await browser.newContext({ baseURL, viewport: { width: 640, height: 450 }, deviceScaleFactor: 2, colorScheme });
        const page = await context.newPage();
        await page.goto(route + "&projectGrid=1");
        await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        const projects = dialog.getByRole("switch", { name: "Read projects", exact: true });
        await expect(projects).not.toBeChecked();
        await expect(dialog.getByRole("switch")).toHaveCount(12);
        await expect(dialog.getByRole("combobox")).toHaveCount(0);
        await confirmPermissionChange(page, "extensions.setGrant", async () => {
            await projects.focus();
            await page.keyboard.press("Space");
        });
        await expect(projects).toBeChecked();
        await expect(projects).toBeEnabled();
        expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "projects.list").length)).toBe(0);
        const overflow = await dialog.locator(".extension-permissions").evaluate((element) => element.scrollWidth - element.clientWidth);
        expect(overflow).toBeLessThanOrEqual(1);
        await page.screenshot({ path: `target/m8-zero-grant/permission-switches-${colorScheme}.png`, fullPage: true });
        await page.evaluate(() => window.h4Management.failNext("extensions.revokeGrant", "extension_scope_denied"));
        await projects.click();
        await expect(dialog.getByRole("alert")).toContainText("This action is outside the extension's allowed permissions or scope.");
        await expect(projects).toBeChecked();
        await expect(projects).toBeEnabled();
        await dialog.getByRole("alert").scrollIntoViewIfNeeded();
        await page.screenshot({ path: `target/m8-zero-grant/permission-switches-error-${colorScheme}.png`, fullPage: true });
        await confirmPermissionChange(page, "extensions.revokeGrant", () => projects.click());
        await expect(projects).not.toBeChecked();
        await expect(dialog.getByRole("alert")).toHaveCount(0);
        expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.revokeGrant").length)).toBe(2);
        await context.close();
    });
}


test("Core observation permissions are optional global switches and never grant client management", async ({ page }) => {
    await page.goto(route + "&m8-runtime-permissions=1");
    await page.getByRole("button", {name:"Manage permissions",exact:true}).click();
    const dialog=page.locator("md-dialog[open]");
    const connections=dialog.getByRole("switch",{name:"Read MCP Core connections",exact:true});
    const calls=dialog.getByRole("switch",{name:"Read MCP Core calls",exact:true});
    await expect(dialog.getByRole("switch")).toHaveCount(12);
    await expect(connections).not.toBeChecked();await expect(calls).not.toBeChecked();
    await expect(dialog.getByText("Required",{exact:true})).toHaveCount(0);
    await confirmPermissionChange(page,"extensions.setGrant",()=>connections.click());
    await expect(connections).toBeChecked();await expect(calls).not.toBeChecked();
    await page.evaluate(()=>window.h4Management.failNext("extensions.setGrant","extension_permission_denied"));
    await calls.click();
    await expect(dialog.getByRole("alert")).toContainText("This action is outside the extension's allowed permissions or scope.");
    await expect(calls).not.toBeChecked();await expect(connections).toBeChecked();
    await confirmPermissionChange(page,"extensions.setGrant",()=>calls.click());
    await expect(calls).toBeChecked();await expect(dialog.getByRole("alert")).toHaveCount(0);
    await confirmPermissionChange(page,"extensions.revokeGrant",()=>connections.click());
    await expect(connections).not.toBeChecked();await expect(calls).toBeChecked();
    const writes=await page.evaluate(()=>window.h4Management.requests.filter(r=>["extensions.setGrant","extensions.revokeGrant"].includes(r.method)).map(r=>r.params));
    expect(writes.map(p=>[p.permission,p.resourceKind,p.resourceId,p.expectedGrantRevision])).toEqual([
        ["mcp.connections.read","Global","*",3],["mcp.requests.read","Global","*",4],
        ["mcp.requests.read","Global","*",4],["mcp.connections.read","Global","*",5]
    ]);
    expect(await page.evaluate(()=>window.h4Management.requests.filter(r=>r.method==="projects.list"||r.method==="mcp.clients.create").length)).toBe(0);
});
