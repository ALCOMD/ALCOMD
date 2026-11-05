import { expectRequestError } from "./request-error-assertions";
import { requestErrorMessage } from "../../src/request-errors";
import { expect, test, type Page } from "@playwright/test";
import { H4_ADDED, H4_ALPHA, H4_BETA, H4_GAMMA } from "./h4-management-client";

test.use({ viewport: { width: 1440, height: 900 } });

const packagePath = "\\\\?\\C:\\Fixture\\長い拡張機能\\extension.alcomdext";

async function open(page: Page, detail = false, extra = "") {
    const route = detail ? `/extensions/${H4_ALPHA}` : "/extensions";
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready&h4-management=1${extra}`);
}
const modal = (page: Page) => page.locator("md-dialog[open]");
async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.h4Management.requests.filter((request) => request.method === name), method);
}
async function installReview(page: Page) {
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await modal(page).getByLabel("Extension package", { exact: true }).fill(packagePath);
    await modal(page).getByLabel("Approve this publisher for this extension", { exact: true }).check();
    await expect(modal(page).getByLabel("Expected registry revision", { exact: true })).toHaveCount(0);
    await modal(page).getByRole("button", { name: "Create install plan", exact: true }).click();
    await expect(modal(page).getByRole("heading", { name: "Review extension install", exact: true })).toBeVisible();
    const requested = modal(page).getByRole("region", { name: "Requested permissions", exact: true });
    await expect(requested.getByText("Read projects", { exact: true })).toBeVisible();
    await expect(requested.getByText("Run in background", { exact: true })).toBeVisible();
    await expect(modal(page).getByRole("region", { name: "Allowed permissions", exact: true })).toHaveCount(0);
}
async function uninstallReview(page: Page, deleteData = false) {
    await page.getByRole("button", { name: "Uninstall extension", exact: true }).click();
    const removal = modal(page).getByLabel("Delete extension-owned data during uninstall", { exact: true });
    await expect(removal).not.toBeChecked();
    if (deleteData) await removal.check();
    await modal(page).getByRole("button", { name: "Create uninstall plan", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Review extension uninstall", exact: true })).toBeVisible();
}

test("H4 extension pagination keeps loaded cards on failure and retries the same opaque cursor", async ({ page }) => {
    await open(page);
    await expect(page.getByRole("article", { name: H4_ALPHA, exact: true })).toBeVisible();
    await expect(page.getByRole("article")).toHaveCount(1);
    await page.evaluate(() => window.h4Management.failNext("extensions.list", "daemon_unavailable"));
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("button", { name: "Retry loading more", exact: true })).toBeVisible();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.getByRole("article", { name: H4_ALPHA, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Retry loading more", exact: true }).click();
    await expect(page.getByRole("article", { name: H4_BETA, exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("article", { name: H4_GAMMA, exact: true })).toBeVisible();
    await expect(page.getByRole("article")).toHaveCount(3);
    await expect(page.getByRole("button", { name: "Load more", exact: true })).toHaveCount(0);
    const paged = (await requests(page, "extensions.list")).filter((request) => request.params.cursor !== undefined);
    expect(paged.map((request) => request.params.cursor)).toEqual([`h4-after:${H4_ALPHA}`, `h4-after:${H4_ALPHA}`, `h4-after:${H4_BETA}`]);
});

test("H4 failed extension toggle retains its state and retries without changing page layout", async ({ page }) => {
    await open(page);
    const card = page.getByRole("article", { name: H4_ALPHA, exact: true });
    const toggle = card.getByRole("switch", { name: "Enabled", exact: true });
    await expect(toggle).toBeChecked();
    const before = await card.boundingBox();
    await page.evaluate(() => window.h4Management.failNext("extensions.disable", "permission_denied"));
    await toggle.click();
    await expect(toggle).toBeChecked();
    await expect(toggle).toBeFocused();
    await expect(toggle).toBeEnabled();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    const failed = await card.boundingBox();
    expect(failed!.width).toBe(before!.width);
    expect(failed!.height).toBe(before!.height);
    await page.clock.install();
    await page.clock.fastForward(6000);
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await toggle.click();
    await expect(toggle).not.toBeChecked();
    await expect(card.getByRole("alert")).toHaveCount(0);
    await expect(toggle).toBeEnabled();
    await expect(card.locator(".reminder-floating")).toHaveCount(0);
    await page.clock.runFor(500);
    await expect(card).toBeVisible();
    const after = await card.boundingBox();
    expect(after!.y).toBe(before!.y);
    expect(after!.height).toBe(before!.height);
    expect(await requests(page, "extensions.disable")).toHaveLength(2);
});

test("H4 install preserves its raw path, reviews expected absence, and follows one accepted operation through completion", async ({ page }) => {
    await open(page);
    await installReview(page);
    expect((await requests(page, "extensions.planInstall"))[0]!.params).toEqual({ packagePath, expectedRevision: 0, publisherApproval: "approve_for_extension" });
    expect(await requests(page, "extensions.applyInstall")).toHaveLength(0);
    await modal(page).getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(modal(page).getByLabel("Extension package", { exact: true })).toHaveValue(packagePath);
    await modal(page).getByRole("button", { name: "Create install plan", exact: true }).click();
    await modal(page).getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(modal(page).locator(".operation-follow")).toContainText(/running/i);
    await expect(page.getByRole("main").locator(".operation-follow")).toHaveCount(0);
    await expect(page.locator(".utility-workspace-toolbar").getByRole("button", { name: "Install extension", exact: true, includeHidden: true })).toBeDisabled();
    const operationId = await page.evaluate(() => window.h4Management.lastOperationId!);
    await page.evaluate(() => window.h4Management.complete("succeeded"));
    await expect(modal(page).locator(".operation-follow")).toContainText(/succeeded/i);
    await modal(page).getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("article", { name: H4_ADDED, exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Install extension", exact: true })).toBeEnabled();
    expect(await requests(page, "extensions.applyInstall")).toHaveLength(1);
    const reads = await requests(page, "operations.get");
    expect(reads.length).toBeGreaterThan(0);
    expect(reads.every((request) => request.params.operationId === operationId)).toBe(true);
});

test("H4 install keeps an unknown Apply result visible and never repeats the write", async ({ page }) => {
    await open(page);
    await installReview(page);
    await page.evaluate(() => window.h4Management.failNext("extensions.applyInstall", "daemon_unavailable"));
    await modal(page).getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toContainText("The result of this request is unknown");
    await expect(modal(page).getByRole("button", { name: "Apply reviewed plan", exact: true })).toBeDisabled();
    await expect(page.locator(".operation-follow")).toHaveCount(0);
    expect(await requests(page, "extensions.applyInstall")).toHaveLength(1);
    await modal(page).getByRole("button", { name: "Cancel", exact: true }).click();
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(modal(page).getByLabel("Extension package", { exact: true })).toHaveValue("");
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    expect(await requests(page, "extensions.applyInstall")).toHaveLength(1);
});

test("H4 quarantined extension recovery uses explicit enable and preserves a refusal without granting permissions", async ({ page }) => {
    await open(page, true, "&h4-quarantined=1");
    await expect(page.getByRole("button", { name: "Open Portable UI", exact: true })).toBeDisabled();
    await expect(page.getByText("Quarantine", { exact: true }).locator("..")).toContainText("quarantined");
    await page.evaluate(() => window.h4Management.failNext("extensions.enable", "extension_package_untrusted"));
    await page.getByRole("button", { name: "Recover extension", exact: true }).click();
    await expect.poll(() => requests(page, "extensions.enable").then((items) => items.length)).toBe(1);
    await expect(page.getByRole("button", { name: "Recover extension", exact: true })).toBeEnabled();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Open Portable UI", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Recover extension", exact: true }).click();
    await expect(page.getByRole("button", { name: "Recover extension", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Open Portable UI", exact: true })).toBeEnabled();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await requests(page, "extensions.enable")).toHaveLength(2);
    expect(await requests(page, "extensions.setGrant")).toHaveLength(0);
    expect(await requests(page, "extensions.revokeGrant")).toHaveLength(0);
});

test("H4 uninstall defaults to retaining data and keeps a failed delete-data operation and recovery error visible", async ({ page }) => {
    await open(page, true);
    await uninstallReview(page);
    expect((await requests(page, "extensions.planUninstall"))[0]!.params.dataDisposition).toBe("retain_data");
    await modal(page).getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await uninstallReview(page, true);
    expect((await requests(page, "extensions.planUninstall"))[1]!.params.dataDisposition).toBe("delete_data");
    await modal(page).getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(modal(page).locator(".operation-follow")).toContainText(/running/i);
    await expect(page.getByRole("main").locator(".operation-follow")).toHaveCount(0);
    await expect(page.getByRole("main").getByRole("button", { name: "Uninstall extension", exact: true, includeHidden: true })).toBeDisabled();
    await page.evaluate(() => window.h4Management.complete("failed", "extension_recovery_required"));
    await expectRequestError(modal(page).getByRole("alert"), "extension_recovery_required");
    await expect(modal(page).locator(".operation-follow")).toContainText(/failed/i);
    await modal(page).getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Extension detail", exact: true })).toBeVisible();
    expect(await requests(page, "extensions.applyUninstall")).toHaveLength(1);
    await expect(page.getByRole("main").getByRole("button", { name: "Uninstall extension", exact: true, includeHidden: true })).toBeDisabled();
});

test("H4 successful uninstall returns to the refreshed list instead of showing a missing-detail error", async ({ page }) => {
    await open(page, true);
    await uninstallReview(page);
    await modal(page).getByRole("button", { name: "Apply reviewed plan", exact: true }).click();
    await expect(page.locator(".operation-follow")).toContainText(/running/i);
    await page.evaluate(() => window.h4Management.complete("succeeded"));
    await expect(page.getByRole("heading", { name: "Extensions", exact: true })).toBeVisible();
    await expect(page.getByRole("article", { name: H4_BETA, exact: true })).toBeVisible();
    await expect(page.getByRole("article", { name: H4_ALPHA, exact: true })).toHaveCount(0);
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await requests(page, "extensions.applyUninstall")).toHaveLength(1);
});

test("H4 late lifecycle completion does not leave a success notification on the next page", async ({ page }) => {
    await open(page);
    await page.evaluate(() => window.h4Management.hold("extensions.disable", true));
    await page.getByRole("article", { name: H4_ALPHA, exact: true }).getByRole("switch", { name: "Enabled", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.h4Management.pending.length)).toBe(1);
    await page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Settings$/ }) }).click();
    await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
    await page.evaluate(() => window.h4Management.release("extensions.disable"));
    await expect(page.getByRole("heading", { name: "Settings", exact: true })).toBeVisible();
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    expect(await requests(page, "extensions.disable")).toHaveLength(1);
});

async function openPermissions(page: Page) {
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
}
const allowed = (page: Page) => modal(page).getByRole("region", { name: "Allowed permissions", exact: true });

test("H4 permission declarations remain separate from empty and partial actual grants", async ({ page }) => {
    await open(page, true);
    await page.evaluate((extensionId) => window.h4Management.setGrants(extensionId, []), H4_ALPHA);
    await openPermissions(page);
    const requested = modal(page).getByRole("region", { name: "Requested permissions", exact: true });
    await expect(requested.getByText("Read projects", { exact: true })).toBeVisible();
    await expect(requested.getByText("Run in background", { exact: true })).toBeVisible();
    await expect(allowed(page)).toContainText("No permissions granted");
    await expect(allowed(page).locator("li")).toHaveCount(0);
    await page.evaluate((extensionId) => window.h4Management.setGrants(extensionId, [{ permission: "background.run", resourceKind: "Extension", resourceId: extensionId }]), H4_ALPHA);
    await modal(page).getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(allowed(page)).toContainText("Run in background");
    await expect(allowed(page)).toContainText("This extension");
    await expect(allowed(page)).not.toContainText("Read projects");
    expect(await requests(page, "extensions.setGrant")).toHaveLength(0);
});

test("H4 project permission selection sends the exact project identity and refreshes after grant and revoke", async ({ page }) => {
    await open(page, true, "&windows-paths=1");
    await page.evaluate((extensionId) => window.h4Management.setGrants(extensionId, []), H4_ALPHA);
    await openPermissions(page);
    await expect(allowed(page)).toContainText("No permissions granted");
    await modal(page).getByRole("combobox", { name: "Project", exact: true }).click();
    await page.getByRole("option", { name: "C:\\Projects\\Sample", exact: true }).click();
    await modal(page).getByRole("button", { name: "Grant", exact: true }).click();
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(allowed(page)).toContainText("Read projects");
    await expect(allowed(page)).toContainText("C:\\Projects\\Sample");
    await expect(allowed(page)).not.toContainText("\\\\?\\");
    expect((await requests(page, "extensions.setGrant"))[0]!.params).toEqual({ extensionId: H4_ALPHA, permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101", expectedGrantRevision: 4 });
    await allowed(page).getByRole("button", { name: "Revoke", exact: true }).click();
    await expect(allowed(page)).toContainText("No permissions granted");
    await expect(allowed(page).locator("li")).toHaveCount(0);
    expect((await requests(page, "extensions.revokeGrant"))[0]!.params).toEqual({ extensionId: H4_ALPHA, permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101", expectedGrantRevision: 5 });
    expect(await requests(page, "extensions.enable")).toHaveLength(0);
});

test("H4 permission read failure stays visible until an explicit refresh and never becomes a false empty grant list", async ({ page }) => {
    await open(page, true);
    await page.evaluate(() => window.h4Management.hold("extensions.permissions.list", true));
    await openPermissions(page);
    await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBeGreaterThan(0);
    await page.evaluate(() => {
        window.h4Management.hold("extensions.permissions.list", false);
        // React StrictMode may have abandoned an earlier read; fail all outstanding reads.
        while (window.h4Management.release("extensions.permissions.list", "store_unavailable")) { /* Drain only the reads already held. */ }
    });
    await expectRequestError(modal(page).getByRole("alert"), "store_unavailable");
    await expect(allowed(page)).toHaveCount(0);
    await expect(modal(page).getByText("No permissions granted", { exact: true })).toHaveCount(0);
    await modal(page).getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(allowed(page)).toContainText("Run in background");
    expect(await requests(page, "extensions.setGrant")).toHaveLength(0);
    expect(await requests(page, "extensions.revokeGrant")).toHaveLength(0);
    await page.evaluate(() => window.h4Management.failNext("extensions.revokeGrant", "daemon_unavailable"));
    await allowed(page).getByRole("button", { name: "Revoke", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toContainText("The permission change may have completed. Refresh permissions before trying again.");
    await expect(allowed(page)).toHaveCount(0);
    await expect(modal(page).getByRole("button", { name: "Revoke", exact: true })).toHaveCount(0);
    await expect(modal(page).getByRole("button", { name: "Grant", exact: true })).toHaveCount(0);
    expect(await requests(page, "extensions.revokeGrant")).toHaveLength(1);
    await modal(page).getByRole("button", { name: "Refresh", exact: true }).click();
    // This fixture refused before writing; the authoritative permission remains granted.
    await expect(allowed(page)).toContainText("Run in background");
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    expect(await requests(page, "extensions.revokeGrant")).toHaveLength(1);
});

test("H4 grant pagination rejects a changed revision without mixing old scopes and refreshes before revoking", async ({ page }) => {
    await open(page, true);
    await openPermissions(page);
    await expect(allowed(page)).toContainText("Run in background");
    await expect(allowed(page).locator("li")).toHaveCount(1);
    // Another management client revokes the project scope between these pages.
    await page.evaluate(() => window.coreEvents.hold(true));
    await page.evaluate((extensionId) => window.h4Management.setGrants(extensionId, [{ permission: "background.run", resourceKind: "Extension", resourceId: extensionId }]), H4_ALPHA);
    await allowed(page).getByRole("button", { name: "Load more", exact: true }).click();
    await expectRequestError(modal(page).getByRole("alert"), "revision_conflict");
    await expect(allowed(page)).toHaveCount(0);
    await expect(modal(page).locator(".extension-grants li")).toHaveCount(0);
    await page.evaluate(() => window.coreEvents.hold(false));
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(allowed(page)).not.toContainText("Read projects");
    await allowed(page).getByRole("button", { name: "Revoke", exact: true }).click();
    await expect(allowed(page)).toContainText("No permissions granted");
    expect((await requests(page, "extensions.revokeGrant"))[0]!.params).toMatchObject({ permission: "background.run", resourceKind: "Extension", resourceId: H4_ALPHA, expectedGrantRevision: 4 });
    const reads = await requests(page, "extensions.permissions.list");
    expect(reads.some((request) => request.params.cursor === `${H4_ALPHA}:3:1`)).toBe(true);
});

test("H4 late permission reads cannot overwrite a reopened dialog or its newer explicit refresh", async ({ page }) => {
    await open(page, true);
    await page.evaluate(() => window.h4Management.hold("extensions.permissions.list", true));
    await openPermissions(page);
    await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBeGreaterThan(0);
    await modal(page).getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.evaluate((extensionId) => {
        window.h4Management.setGrants(extensionId, [{ permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101" }]);
        window.h4Management.hold("extensions.permissions.list", false);
    }, H4_ALPHA);
    await openPermissions(page);
    await expect(allowed(page)).toHaveCount(0);
    expect(await page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "extensions.permissions.list").length)).toBe(1);
    await page.evaluate(() => window.h4Management.release("extensions.permissions.list"));
    await expect(allowed(page)).toContainText("Read projects");
    await expect(allowed(page)).not.toContainText("Run in background");
    await page.evaluate((extensionId) => window.h4Management.setGrants(extensionId, []), H4_ALPHA);
    await modal(page).getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(allowed(page)).toContainText("No permissions granted");
    await page.evaluate(async () => {
        while (window.h4Management.release("extensions.permissions.list")) { /* Resolve the old dialog's outstanding reads. */ }
        await new Promise<void>((resolve) => window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve())));
    });
    await expect(allowed(page)).toContainText("No permissions granted");
    await expect(allowed(page).locator("li")).toHaveCount(0);
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
});

test("H4 project choice read failure preserves grants and explicit refresh restores scoped granting", async ({ page }) => {
    await open(page, true, "&windows-paths=1");
    await page.evaluate((extensionId) => {
        window.h4Management.setGrants(extensionId, [{ permission: "background.run", resourceKind: "Extension", resourceId: extensionId }]);
        window.h4Management.hold("projects.list", true);
    }, H4_ALPHA);
    await openPermissions(page);
    await expect(allowed(page)).toContainText("Run in background");
    await expect(allowed(page)).toContainText("This extension");
    const projectChoice = modal(page).getByRole("combobox", { name: "Project", exact: true });
    const projectSelect = modal(page).locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: "Project", exact: true }) });
    await expect(projectSelect).toHaveJSProperty("disabled", true);
    await expect(modal(page).getByRole("button", { name: "Grant", exact: true })).toBeDisabled();
    await expect.poll(() => page.evaluate(() => window.h4Management.pending.filter((request) => request.method === "projects.list").length)).toBeGreaterThan(0);
    await page.evaluate(() => {
        window.h4Management.hold("projects.list", false);
        while (window.h4Management.release("projects.list", "store_unavailable")) { /* Fail every initial StrictMode read still pending. */ }
    });
    const fieldError = modal(page).getByRole("alert").filter({ hasText: requestErrorMessage("store_unavailable") });
    await expect(fieldError).toBeVisible();
    await expect(allowed(page).locator("li")).toHaveCount(1);
    await expect(allowed(page)).toContainText("This extension");
    await expect(modal(page).getByRole("button", { name: "Grant", exact: true })).toBeDisabled();
    await projectChoice.click();
    await expect(page.getByRole("option", { name: "Select project", exact: true })).toBeVisible();
    await expect(page.getByRole("option")).toHaveCount(1);
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
    await expect(projectChoice).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("option", { name: "Select project", exact: true })).toHaveCount(0);
    await expect(fieldError).toBeVisible();
    expect(await requests(page, "extensions.setGrant")).toHaveLength(0);
    const readsBeforeRefresh = (await requests(page, "projects.list")).length;
    await fieldError.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(modal(page).getByRole("alert")).toHaveCount(0);
    await expect(projectSelect).toHaveJSProperty("disabled", false);
    expect((await requests(page, "projects.list")).length).toBeGreaterThan(readsBeforeRefresh);
    await projectChoice.click();
    await page.getByRole("option", { name: "C:\\Projects\\Sample", exact: true }).click();
    await modal(page).getByRole("button", { name: "Grant", exact: true }).click();
    await expect(allowed(page).getByRole("button", { name: "Load more", exact: true })).toBeVisible();
    await allowed(page).getByRole("button", { name: "Load more", exact: true }).click();
    await expect(allowed(page).locator("li")).toHaveCount(2);
    await expect(allowed(page)).toContainText("C:\\Projects\\Sample");
    expect((await requests(page, "extensions.setGrant"))[0]!.params).toEqual({ extensionId: H4_ALPHA, permission: "projects.read", resourceKind: "Project", resourceId: "00000000-0000-4000-8000-000000000101", expectedGrantRevision: 4 });
});
