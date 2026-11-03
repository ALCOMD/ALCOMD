import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

const projectId = "00000000-0000-4000-8000-000000000101";
const backupId = "00000000-0000-4000-8000-000000000104";
const operationId = "00000000-0000-4000-8000-000000000105";
const restoredProjectId = "00000000-0000-4000-8000-000000000109";

test.use({ viewport: { width: 1440, height: 900 } });

test("backup creation leaves one contextual task and refreshes its project history only after completion", async ({ page }) => {
    await open(page, `/projects/${projectId}/backups`, "h3-create=1");
    await expect(page.getByText("No backups for this project")).toBeVisible();
    await page.getByRole("button", { name: "Create backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("heading", { name: "Create backup", exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Create backup", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".operation-follow")).toContainText("running");
    expect(await requests(page, "backups.create")).toEqual([{ method: "backups.create", params: { projectId, expectedRevision: 2, compressionMode: "fast", excludeVpmPackages: true } }]);
    expect(await requests(page, "backups.list")).toHaveLength(1);
    await state(page, "succeeded");
    await expect(page.getByRole("table", { name: "Backups", exact: true }).getByRole("row")).toHaveCount(2);
    expect(await requests(page, "backups.list")).toHaveLength(2);
    await page.getByRole("button", { name: "View operation", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/operations/${operationId}$`));
    await page.getByRole("button", { name: "View backup", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/backups/${backupId}$`));
    expect(await requests(page, "backups.create")).toHaveLength(1);
});

test("project backup history and Tasks preserve their Core pagination cursors", async ({ page }) => {
    await open(page, `/projects/${projectId}/backups`, "h3-pages=1");
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("table", { name: "Backups", exact: true }).getByRole("row")).toHaveCount(3);
    expect(await requests(page, "backups.list")).toEqual([
        { method: "backups.list", params: { projectId } },
        { method: "backups.list", params: { projectId, cursor: "older-backups" } }
    ]);
    await expect(page.getByRole("button", { name: "Load more", exact: true })).toHaveCount(0);

    await open(page, "/operations", "h3-pages=1");
    await page.getByRole("button", { name: "Load more", exact: true }).click();
    await expect(page.getByRole("table", { name: "Tasks", exact: true }).getByRole("row")).toHaveCount(3);
    expect((await requests(page, "operations.list")).filter((request) => request.params.cursor !== undefined)).toEqual([
        { method: "operations.list", params: { cursor: { createdAtMs: 1_700_000_000_000, operationId } } }
    ]);
    await expect(page.getByRole("button", { name: "Load more", exact: true })).toHaveCount(0);
});

test("operation detail retries a failed progress read on the same ID and keeps terminal failures visible", async ({ page }) => {
    await open(page, `/operations/${operationId}`, "h3-read-error-at=2");
    await expect(page.locator(".operation-follow")).toContainText("running");
    await expect(page.locator(".operation-follow")).toContainText("temporarily unavailable");
    await expect.poll(async () => (await requests(page, "operations.get")).length).toBeGreaterThanOrEqual(3);
    await expect(page.locator(".operation-follow")).not.toContainText("temporarily unavailable");
    await page.evaluate(() => { window.h3Operation = { ...window.h3Operation, state: "failed", errorCode: "backup_archive_invalid", diagnosticId: "d".repeat(500) }; });
    const error = page.getByRole("alert");
    await expectRequestError(error, "backup_archive_invalid");
    await expect(error).toContainText("d".repeat(500));
    await expect(page.getByRole("button", { name: "Cancel operation", exact: true })).toBeDisabled();
    expect((await requests(page, "operations.get")).every((request) => request.params.operationId === operationId)).toBe(true);
    const box = await error.boundingBox();
    expect(box!.x + box!.width).toBeLessThanOrEqual(1440);
    expect(await requests(page, "operations.cancel")).toEqual([]);
});

test("cancellation failure stays in its dialog and a retry uses the latest revision without claiming rollback", async ({ page }) => {
    await open(page, `/operations/${operationId}`, "h3-cancel-conflict=1");
    await page.getByRole("button", { name: "Cancel operation", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("heading", { name: "Request cancellation?", exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "resource_revision_conflict");
    await page.evaluate(() => { window.h3Operation.revision = 10; });
    const prior = (await requests(page, "operations.get")).length;
    await expect.poll(async () => (await requests(page, "operations.get")).length).toBeGreaterThan(prior);
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator(".operation-follow")).toContainText("cancelling");
    expect(await requests(page, "operations.cancel")).toEqual([
        { method: "operations.cancel", params: { operationId, expectedRevision: 3 } },
        { method: "operations.cancel", params: { operationId, expectedRevision: 10 } }
    ]);
    await state(page, "cancelled");
    await expect(page.locator(".operation-follow")).toContainText("cancelled");
    await expect(page.getByRole("button", { name: "Cancel operation", exact: true })).toBeDisabled();
});

test("backup detail reuses managed restore, survives its first progress failure, and opens the registered project", async ({ page }) => {
    const selected = "00000000-0000-4000-8000-000000000611";
    await open(page, `/backups/${selected}`, "h3-read-error-until=2&h3-excluded-packages=1");
    await page.getByRole("button", { name: "Restore backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("heading", { name: "Restore project", exact: true })).toBeVisible();
    await expect(dialog.getByRole("combobox", { name: "Project backup", exact: true })).toHaveCount(0);
    await reviewRestore(page);
    await expect(dialog.getByRole("alert")).toContainText("separate resolve");
    await dialog.getByRole("button", { name: "Restore project", exact: true }).click();
    await expect(dialog.locator(".operation-follow")).toContainText("temporarily unavailable");
    await expect(dialog.locator(".operation-follow")).toContainText("running");
    expect(await requests(page, "backups.planRestore")).toEqual([{ method: "backups.planRestore", params: { backupId: selected, targetParent: "C:\\Fixture\\Avatar", targetLeaf: "H3 Restored" } }]);
    await state(page, "succeeded");
    await expect(page).toHaveURL(new RegExp(`/projects/${restoredProjectId}$`));
    expect(await requests(page, "backups.applyRestore")).toHaveLength(1);
    expect((await requests(page, "operations.get")).every((request) => request.params.operationId === operationId)).toBe(true);
});

test("an unknown restore write remains unknown and cannot be submitted again", async ({ page }) => {
    await open(page, `/backups/${backupId}`, "h3-restore-unknown=1");
    await page.getByRole("button", { name: "Restore backup", exact: true }).click();
    await reviewRestore(page);
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Restore project", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("result of this request is unknown");
    await expect(dialog.getByRole("button", { name: "Restore project", exact: true })).toBeDisabled();
    await expect(dialog.getByRole("button", { name: "Back", exact: true })).toBeDisabled();
    expect(await requests(page, "backups.applyRestore")).toHaveLength(1);
    expect(await requests(page, "operations.get")).toEqual([]);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
});

test("restore can retry a failed list read and select a backup beyond its first page", async ({ page }) => {
    await open(page, "/projects", "h3-list-error=1&h3-pages=1");
    await page.getByRole("button", { name: "More project creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
    await expect(dialog.getByText("No managed backups available")).toHaveCount(0);
    await dialog.getByRole("button", { name: "Retry", exact: true }).click();
    await dialog.getByRole("button", { name: "Load more", exact: true }).click();
    await dialog.getByRole("combobox", { name: "Project backup", exact: true }).click();
    await page.getByRole("option").last().click();
    await reviewRestore(page);
    expect((await requests(page, "backups.planRestore"))[0]!.params.backupId).toBe("00000000-0000-4000-8000-000000000611");
    expect(await requests(page, "backups.applyRestore")).toEqual([]);
});

test("an unknown backup creation does not start a second task or label closing as cancellation", async ({ page }) => {
    await open(page, `/projects/${projectId}/backups`, "h3-backup-create-error=daemon_unavailable");
    await page.getByRole("button", { name: "Create backup", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Create backup", exact: true }).click();
    await expect(dialog.getByRole("alert")).toContainText("result of this request is unknown");
    await expect(dialog.getByRole("button", { name: "Create backup", exact: true })).toBeDisabled();
    await expect(dialog.getByRole("button", { name: "Cancel", exact: true })).toHaveCount(0);
    expect(await requests(page, "backups.create")).toHaveLength(1);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    expect(await requests(page, "operations.cancel")).toEqual([]);
});

test("Settings Unity errors stay with the action and a corrected registration uses a layout-free success reminder", async ({ page }) => {
    await open(page, "/settings", "h3-unity-register-error=1");
    await selectSettingsSection(page, "unity");
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByLabel("Unity executable", { exact: true }).fill("C:/invalid-editor.exe");
    await dialog.getByRole("button", { name: "Register", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "unity_installation_invalid");
    await dialog.getByLabel("Unity executable", { exact: true }).fill("C:/Editors/Unity.exe");
    await expect(dialog.getByRole("alert")).toHaveCount(0);
    const before = await dialog.getByRole("button", { name: "Register", exact: true }).boundingBox();
    await dialog.getByRole("button", { name: "Register", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("Unity installation registered.");
    await expect(dialog.locator(".reminder")).toHaveCount(0);
    const after = await dialog.getByRole("button", { name: "Register", exact: true }).boundingBox();
    expect(after!.y).toBeCloseTo(before!.y, 0);
    expect(await requests(page, "unity.installations.register")).toHaveLength(2);
});

test("a successful Unity launch keeps the project row unchanged and closing an error leaves no stale feedback", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=unity-automatic");
    const row = page.locator(".project-row-actions").first().locator("xpath=ancestor::tr");
    const before = await row.boundingBox();
    await row.getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("Unity launch opening.");
    const after = await row.boundingBox();
    expect(after!.height).toBe(before!.height);
    await expect(row.getByRole("status")).toHaveCount(0);

    await page.goto("/browser-harness.html?route=/projects&unityOptionsError=1");
    const launch = page.locator(".project-row-actions").first().getByRole("button", { name: "Open Unity", exact: true });
    await launch.click();
    const outlet = page.locator(".reminder-floating");
    await expectRequestError(outlet.getByRole("alert"), "revision_conflict");
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await outlet.getByRole("button", { name: "Dismiss notification", exact: true }).click();
    await expect(launch).toBeFocused();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method !== "unity.launchOptions"))).toEqual([]);
});

for (const route of ["/projects", `/projects/${projectId}/packages`]) {
    test(`Unity launch status is an explicit same-record read with retry and focus return in ${route}`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=unity-automatic&unityStatusError=1`);
        const controls = route === "/projects" ? page.locator(".project-row-actions").first() : page.getByRole("navigation", { name: "Project actions" });
        const more = controls.getByRole("button", { name: /^More actions for/ });
        await more.click();
        await expect(page.getByRole("menuitem", { name: "Unity launch status", exact: true })).toHaveCount(0);
        await page.keyboard.press("Escape");
        await controls.getByRole("button", { name: "Open Unity", exact: true }).click();
        await expect(page.locator(".reminder-floating")).toContainText("Unity launch opening.");
        expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launchStatus"))).toEqual([]);
        await more.click();
        await page.getByRole("menuitem", { name: "Unity launch status", exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        await expectRequestError(dialog.getByRole("alert"), "daemon_unavailable");
        await dialog.getByRole("button", { name: "Refresh", exact: true }).click();
        await expect(dialog.getByRole("status")).toContainText("Last observed launch state: open");
        const reads = await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launchStatus"));
        expect(reads).toHaveLength(2);
        expect(reads[0]!.params).toEqual(reads[1]!.params);
        expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toHaveLength(1);
        await dialog.getByRole("button", { name: "Close", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        await expect(more).toBeFocused();
    });
}

test("Unity status preserves Core failure and rejects another launch's DTO", async ({ page }) => {
    for (const [params, expected] of [["unityStatusState=failed", "Last observed launch state: failed"], ["unityStatusMismatch=1", "internal_error"]]) {
        await page.goto(`/browser-harness.html?route=/projects&state=unity-automatic&${params}`);
        const controls = page.locator(".project-row-actions").first();
        await controls.getByRole("button", { name: "Open Unity", exact: true }).click();
        await expect(page.locator(".reminder-floating")).toContainText("Unity launch opening.");
        await controls.getByRole("button", { name: /^More actions for/ }).click();
        await page.getByRole("menuitem", { name: "Unity launch status", exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        if (params === "unityStatusMismatch=1") await expectRequestError(dialog.getByRole("alert"), expected!);
        else await expect(dialog.getByRole("alert")).toContainText(expected!);
        expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toHaveLength(1);
        await dialog.getByRole("button", { name: "Close", exact: true }).click();
        await expect(dialog).toHaveCount(0);
        await expect(page.getByRole("alert")).toHaveCount(0);
    }
});

test("a late Unity status response cannot reopen its dialog after leaving the project", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=unity-automatic&hold-unity-status=1");
    const controls = page.locator(".project-row-actions").first();
    await controls.getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect(page.locator(".reminder-floating")).toContainText("Unity launch opening.");
    await controls.getByRole("button", { name: /^More actions for/ }).click();
    await page.getByRole("menuitem", { name: "Unity launch status", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByRole("button", { name: "Close", exact: true })).toBeDisabled();
    await page.evaluate(() => { history.pushState(null, "", "/operations"); dispatchEvent(new PopStateEvent("popstate")); });
    await expect(page.getByRole("heading", { name: "Tasks", exact: true })).toBeVisible();
    await page.evaluate(() => dispatchEvent(new Event("test-unity-status-ready")));
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toBeHidden();
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toHaveLength(1);
});

async function open(page: Page, route: string, params = "") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&h3=1&${params}`);
}

async function requests(page: Page, method: string) {
    return page.evaluate((method) => window.h3Requests.filter((request) => request.method === method), method);
}

async function state(page: Page, state: string) {
    await page.evaluate((state) => { window.h3Operation = { ...window.h3Operation, state, revision: window.h3Operation.revision + 1 }; }, state);
}

async function reviewRestore(page: Page) {
    await page.getByRole("button", { name: "Choose directory", exact: true }).click();
    await page.getByLabel("Restore project name").fill("H3 Restored");
    await page.getByRole("button", { name: "Review restore", exact: true }).click();
}
