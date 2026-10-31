import { expect, test, type Page } from "@playwright/test";

const userPackageId = "00000000-0000-4000-8000-000000000113";

test.use({ viewport: { width: 1440, height: 900 } });

test("User Package details read the current Core record and return focus to their row", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    const details = row.getByRole("button", { name: "Details", exact: true });
    await details.click();
    await expect(page.getByRole("dialog", { name: "User Package details", exact: true })).toBeVisible();
    const host = page.locator("md-dialog[open]");
    await expect(host.getByText("Local avatar tools", { exact: true })).toBeVisible();
    await expect(host.getByText("com.example.avatar", { exact: true })).toBeVisible();
    await expect(host.getByText("1.2.3", { exact: true })).toBeVisible();
    await expect(host.getByText("<private-user-package>", { exact: true })).toBeVisible();
    const reads = await requests(page, "packages.userPackages.get");
    expect(reads.length).toBeGreaterThan(0);
    expect(reads.every((request) => (request.params as { userPackageId: string }).userPackageId === userPackageId)).toBe(true);
    expect(await requests(page, "packages.userPackages.refresh")).toEqual([]);
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "User Package details", exact: true })).toBeHidden();
    await expect(details).toBeFocused();
});

test("User Package detail read errors retry the same ID without changing enrollment", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-details-error=1");
    await page.getByRole("button", { name: "Details", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    await expect(host.getByRole("alert")).toContainText("store_unavailable");
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-details-readable")));
    await host.getByRole("button", { name: "Reconnect and retry", exact: true }).click();
    await expect(host.getByText("<private-user-package>", { exact: true })).toBeVisible();
    expect((await requests(page, "packages.userPackages.get")).every((request) => (request.params as { userPackageId: string }).userPackageId === userPackageId)).toBe(true);
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([]);
    expect(await requests(page, "packages.userPackages.remove")).toEqual([]);
});

test("closed User Package details cannot replace the next read with an old snapshot", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&resource-lifecycle=1&hold-user-details=1");
    await page.getByRole("button", { name: "Details", exact: true }).click();
    await expect.poll(async () => (await requests(page, "packages.userPackages.get")).length).toBeGreaterThan(0);
    const initialReads = (await requests(page, "packages.userPackages.get")).length;
    await page.locator("md-dialog[open]").getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "User Package details", exact: true })).toBeHidden();
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-package-advance")));
    await page.getByRole("button", { name: "Details", exact: true }).click();
    await expect.poll(async () => (await requests(page, "packages.userPackages.get")).length).toBeGreaterThan(initialReads);
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-details-ready")));
    await expect(page.locator("md-dialog[open]").getByText("1.3.0", { exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByText("1.2.3", { exact: true })).toHaveCount(0);
});

test("cancelled directory selection does not enroll a User Package", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1&cancel-directory=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    await expect(page.getByText("No User Packages enrolled", { exact: true })).toBeVisible();
    expect(await requests(page, "selectDirectory")).toHaveLength(1);
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([]);
});

test("User Package enrollment, current details, refresh and confirmed removal use their existing RPCs", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    const row = userRow(page);
    await expect(row).toContainText("1.2.3");
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([{ method: "packages.userPackages.enroll", params: { sourcePath: "C:\\Fixture\\Avatar" } }]);
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(row).toContainText("revision 2");
    await expect(row).toContainText("1.3.0");
    await row.getByRole("button", { name: "Details", exact: true }).click();
    const details = page.locator("md-dialog[open]");
    await expect(details.getByText("C:\\Fixture\\Avatar", { exact: true })).toBeVisible();
    await expect(details.getByText("1.3.0", { exact: true })).toBeVisible();
    await details.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "User Package details", exact: true })).toBeHidden();
    await row.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Remove User Package?", exact: true })).toBeHidden();
    expect(await requests(page, "packages.userPackages.remove")).toEqual([]);
    await row.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await expect(page.getByText("No User Packages enrolled", { exact: true })).toBeVisible();
    expect(await requests(page, "packages.userPackages.remove")).toEqual([{ method: "packages.userPackages.remove", params: { userPackageId, expectedRevision: 2 } }]);
});

test("User Package revision conflicts require an explicit list read before a new refresh", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&resource-lifecycle=1");
    const row = userRow(page);
    await expect(row).toContainText("revision 1");
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-package-advance")));
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("revision_conflict");
    expect(await requests(page, "packages.userPackages.refresh")).toHaveLength(1);
    await expect(row).toContainText("revision 1");
    await page.locator(".utility-workspace-actions").getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(row).toContainText("revision 2");
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(row).toContainText("revision 3");
    expect((await requests(page, "packages.userPackages.refresh")).map((request) => request.params)).toEqual([{ userPackageId, expectedRevision: 1 }, { userPackageId, expectedRevision: 2 }]);
});

test("missing User Package capability prevents listing, details and enrollment calls", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=capabilities-missing");
    await expect(page.getByText("Feature unavailable", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Enroll folder", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method.startsWith("packages.userPackages.")))).toEqual([]);
});

for (const kind of ["remote", "local"] as const) {
    test(`${kind} repository inspect is read-only and only explicit registration and removal change the list`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
        await reviewRepository(page, kind);
        expect(await requests(page, "repositories.register")).toEqual([]);
        await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeHidden();
        await expect(page.getByText("No repositories registered", { exact: true })).toBeVisible();
        await registerRepository(page, kind);
        expect(await requests(page, "repositories.register")).toHaveLength(1);
        const row = repositoryRow(page, kind);
        await row.getByRole("button", { name: "Browse packages", exact: true }).click();
        const preview = page.locator("md-dialog[open]");
        await expect(preview.getByText("First catalog package", { exact: true })).toBeVisible();
        await preview.getByRole("button", { name: "Load more", exact: true }).click();
        await expect(preview.getByText("Second catalog package", { exact: true })).toBeVisible();
        expect(await page.evaluate(() => window.repositoryPackageRequests.some((request) => request.cursor?.packageId === "com.example.first"))).toBe(true);
        await preview.getByRole("button", { name: "Close", exact: true }).click();
        await row.getByRole("button", { name: "Remove", exact: true }).click();
        await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(page.getByRole("dialog", { name: "Remove this repository?", exact: true })).toBeHidden();
        expect(await requests(page, "repositories.unregister")).toEqual([]);
        await row.getByRole("button", { name: "Remove", exact: true }).click();
        await page.locator("md-dialog[open]").getByRole("button", { name: "Remove repository", exact: true }).click();
        await expect(page.getByText("No repositories registered", { exact: true })).toBeVisible();
        expect(await requests(page, "repositories.unregister")).toHaveLength(1);
    });
}

test("repository refresh failure retains the last catalog until a successful explicit refresh", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1&resource-refresh-error=1");
    await registerRepository(page, "remote");
    const row = repositoryRow(page, "remote");
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("repository_unavailable");
    await expect(row).toBeVisible();
    await row.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(page.locator("md-dialog[open]").getByText("First catalog package", { exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Close", exact: true }).click();
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await row.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(page.locator("md-dialog[open]").getByText("Updated catalog package", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.refresh")).toHaveLength(2);
});

test("repository CAS conflicts do not auto-retry or replace a stale revision", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await registerRepository(page, "remote");
    const row = repositoryRow(page, "remote");
    await page.evaluate(() => window.dispatchEvent(new Event("test-resource-repository-advance")));
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("revision_conflict");
    expect(await requests(page, "repositories.refresh")).toHaveLength(1);
    await page.locator(".utility-workspace-actions").getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.locator(".utility-workspace-actions").getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    await row.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await expect(row.getByRole("status")).toContainText("Repository refreshed.");
    expect((await requests(page, "repositories.refresh")).map((request) => (request.params as { expectedRevision: number }).expectedRevision)).toEqual([1, 2]);
});

test("invalid repository inspection and duplicate registration preserve the registered list", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1&resource-invalid-source=1");
    await reviewRepository(page, "remote", true);
    await expect(page.getByRole("alert")).toContainText("repository_document_invalid");
    expect(await requests(page, "repositories.register")).toEqual([]);
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await registerRepository(page, "remote");
    await reviewRepository(page, "remote");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Add repository", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("repository_already_registered");
    await expect(page.getByRole("table", { name: "Repositories", exact: true }).locator("tbody tr")).toHaveCount(1);
    expect(await requests(page, "repositories.register")).toHaveLength(2);
});

function userRow(page: Page) { return page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" }); }
function repositoryRow(page: Page, kind: "remote" | "local") { return page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: kind === "remote" ? "Example packages" : "Local packages" }); }
async function requests(page: Page, method: string) { return page.evaluate((name) => window.resourceRequests.filter((request) => request.method === name), method); }

async function reviewRepository(page: Page, kind: "remote" | "local", invalid = false) {
    await page.getByRole("button", { name: "Add repository", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    const host = page.locator("md-dialog[open]");
    if (kind === "local") {
        await host.getByRole("combobox", { name: "Source type", exact: true }).click();
        await host.getByRole("option", { name: "Local manifest", exact: true }).click();
    }
    await host.getByLabel(kind === "local" ? "Local manifest path" : "Repository URL", { exact: true }).fill(kind === "local" ? "C:\\Fixture\\repository.json" : "https://fixture.example/repository.json");
    await host.getByRole("button", { name: "Review repository", exact: true }).click();
    if (!invalid) await expect(host.locator(".dialog-summary")).toContainText(kind === "local" ? "Local packages" : "Example packages");
}

async function registerRepository(page: Page, kind: "remote" | "local") {
    await reviewRepository(page, kind);
    const host = page.locator("md-dialog[open]");
    await host.getByRole("button", { name: "Add repository", exact: true }).click();
    await expect(host.getByRole("status")).toContainText("Repository added.");
    await host.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeHidden();
    await expect(repositoryRow(page, kind)).toBeVisible();
}
