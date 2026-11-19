import { openLocalExtensionInstall } from "./extension-install-entry";
import { H4_ADDED, H4_ALPHA } from "./h4-management-client";
import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("Extensions expose installed cards and their lifecycle and Open actions", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1");
    await expect(page.getByRole("heading", { name: "Installed", exact: true })).toBeVisible();
    const card = page.getByRole("article", { name: "com.example.alpha", exact: true });
    await expect(card.getByRole("heading", { name: "com.example.alpha", exact: true })).toBeVisible();
    await expect(page.getByRole("table", { name: "Extensions", exact: true })).toHaveCount(0);
    await card.getByRole("switch", { name: "Enabled", exact: true }).click();
    await expect(card.getByRole("switch", { name: "Enabled", exact: true })).not.toBeChecked();
    await card.getByRole("button", { name: "Manage extension", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Extension detail", exact: true })).toBeVisible();
});

test("Extensions open only their declared Portable UI through the card", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready");
    await page.getByRole("article", { name: "com.vrclearn.discord", exact: true }).getByRole("button", { name: "Open", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Extension UI", exact: true })).toBeVisible();
});

test("A scoped extension asks once for consent before enabling its signed target", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1&m9-activation=1");
    const enabled = page.getByRole("article", { name: H4_ALPHA, exact: true }).getByRole("switch", { name: "Enabled", exact: true });
    const dialog = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Enable extension", exact: true }) });
    const accessibleDialog = page.getByRole("dialog", { name: "Enable extension", exact: true });
    await expect(enabled).not.toBeChecked();
    await enabled.click();
    await expect(accessibleDialog).toBeVisible();
    await expect(dialog.getByText("Read active Unity sessions", { exact: true })).toBeVisible();
    await expect(dialog.getByText("Discord desktop (discord · discord.exe · Discord Inc.)", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(accessibleDialog).toHaveCount(0);
    expect(await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.enable"))).toHaveLength(0);
    await enabled.click();
    await dialog.getByRole("button", { name: "Enable", exact: true }).click();
    await expect(enabled).toBeChecked();
    expect(await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.enable").map((request) => request.params))).toEqual([
        { extensionId: H4_ALPHA, expectedRevision: 4, activationPackageDigest: "a".repeat(64) }
    ]);
    await page.getByRole("article", { name: H4_ALPHA, exact: true }).getByRole("button", { name: "Manage extension", exact: true }).click();
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    const permissions = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Extension permissions", exact: true }) });
    const revoke = permissions.getByRole("group", { name: "Connect to local applications", exact: true }).getByRole("button", { name: "Revoke", exact: true });
    await revoke.click();
    await expect(revoke).toHaveCount(0);
    expect(await page.evaluate(() => window.h4Management.requests.filter((request) => request.method === "extensions.revokeGrant").map((request) => request.params))).toEqual([
        { extensionId: H4_ALPHA, permission: "local-ipc.connect", resourceKind: "LocalIpcTarget", resourceId: "b".repeat(64), expectedGrantRevision: 4 }
    ]);
});

test("Extension install review shares its input modal and declining retains the path without Apply", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready&h4-management=1");
    await openLocalExtensionInstall(page);
    await expect(page.getByRole("dialog", { name: "Install extension", exact: true })).toBeVisible();
    // Material's named native dialog owns the slotted light-DOM form.
    const dialog = page.locator("md-dialog[open]");
    const packagePath = "C:\\Fixture\\extension.alcomdextension";
    await dialog.getByLabel("Extension package", { exact: true }).fill(packagePath);
    await expect(dialog.getByLabel("Expected registry revision", { exact: true })).toHaveCount(0);
    await dialog.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(dialog.getByRole("heading", { name: "Required permissions", exact: true })).toBeVisible();
    await expect(dialog.getByText(H4_ADDED, { exact: true })).toBeVisible();
    await expect(dialog.getByText("1.0.0", { exact: true })).toBeVisible();
    await expect(dialog.getByRole("region", { name: "Requested permissions", exact: true }).getByText("Read projects", { exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.h4Management.requests.filter((r) => r.method === "extensions.applyInstall"))).toHaveLength(0);
    await dialog.getByRole("button", { name: "Decline", exact: true }).click();
    await expect(dialog.getByLabel("Extension package", { exact: true })).toHaveValue(packagePath);
    await expect(dialog.getByRole("status")).toContainText("required permissions were declined");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    const requests = await page.evaluate(() => window.h4Management.requests);
    expect(requests.filter((r) => r.method === "extensions.planInstall").map((r) => r.params)).toEqual([{ packagePath, expectedRevision: 0 }]);
    expect(requests.filter((r) => r.method === "extensions.applyInstall")).toHaveLength(0);
});

test("Extension permission and uninstall workflows open named modals with cancel", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions%2Fcom.vrclearn.discord&state=ready");
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Extension permissions", exact: true }) }).getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "Uninstall extension", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Uninstall extension", exact: true })).toBeVisible();
    const preparation = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Uninstall extension", exact: true }) });
    await preparation.getByRole("button", { name: "Create uninstall plan", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Review extension uninstall", exact: true })).toBeVisible();
    const review = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Review extension uninstall", exact: true }) });
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(review.getByText("retain data", { exact: true })).toBeVisible();
    await review.getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
});
