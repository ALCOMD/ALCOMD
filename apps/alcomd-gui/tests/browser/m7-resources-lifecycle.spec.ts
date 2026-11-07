import { expect, test, type Page } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

const userPackageId = "00000000-0000-4000-8000-000000000113";

test.use({ viewport: { width: 1440, height: 900 } });

for (const entry of [
    { route: "/repositories", menu: "Repository list actions", action: "Import list", label: "Repository list file", path: "C:/fixture/repositories.txt", method: "repositoryListInspectFile" },
    { route: "/templates", menu: "More template creation options", action: "Import template", label: "Template file", path: "C:\\Fixture\\input.alcomdtemplate", method: "projectTemplatesSave" },
    { route: "/projects", menu: "More project creation options", action: "Add existing project", label: "Project location", path: "C:\\Fixture\\Avatar", method: "projectRegister" },
    { route: "/user-packages", menu: undefined, action: "Enroll folder", label: "Source folder", path: "C:\\Fixture\\Avatar", method: "userPackageEnroll" }
] as const) {
    test(`${entry.action} uses the shared picker and blocks closure while native selection is pending`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=${entry.route}&resource-lifecycle=1&hold-file-selection=1`);
        if (entry.menu !== undefined) {
            await page.getByRole("button", { name: entry.menu, exact: true }).click();
            await page.getByRole("menuitem", { name: entry.action, exact: true }).click();
        } else await page.getByRole("button", { name: entry.action, exact: true }).click();
        const host = page.locator("md-dialog[open]");
        const input = host.getByRole("textbox", { name: entry.label, exact: true });
        const confirm = host.getByRole("button", { name: "Confirm", exact: true });
        const cancel = host.getByRole("button", { name: "Cancel", exact: true });
        await expect(host.locator(".file-picker__drop")).toBeVisible();
        await expect(confirm).toBeDisabled();
        await expect(host.locator('[slot="actions"]').getByRole("button")).toHaveCount(2);
        await host.getByRole("button", { name: "Select", exact: true }).click();
        await expect(cancel).toBeDisabled();
        await expect(confirm).toBeDisabled();
        await page.keyboard.press("Escape");
        await page.mouse.click(2, 2);
        await expect(host).toBeVisible();
        await page.evaluate(() => window.dispatchEvent(new Event("test-file-selection-ready")));
        await expect(input).toHaveValue(entry.path);
        await expect(cancel).toBeEnabled();
        await expect(confirm).toBeEnabled();
        expect(await page.evaluate((method) => window.coreEvents.rpcRequests.filter((request) => request.method === method).length, entry.method)).toBe(0);
        await confirm.click();
        await expect.poll(() => page.evaluate((method) => window.coreEvents.rpcRequests.filter((request) => request.method === method).length, entry.method)).toBe(1);
    });
}

test("folder picker accepts a typed or dropped directory only after confirmation", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    const host = page.locator("md-dialog[open]");
    const input = host.getByRole("textbox", { name: "Source folder", exact: true });
    await input.fill("C:\\Fixture\\Typed folder");
    const drop = host.getByRole("button", { name: "Drop a folder here or click to select", exact: true });
    const box = await drop.boundingBox();
    if (box === null) throw new Error("Missing folder drop zone");
    const position = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    await page.evaluate((position) => window.dispatchEvent(new CustomEvent("fixture-file-drag", {
        detail: { type: "drop", ...position, paths: ["C:\\Fixture\\One", "C:\\Fixture\\Two"] }
    })), position);
    await expect(host.getByRole("alert")).toHaveText("Select one folder at a time.");
    await expect(input).toHaveValue("C:\\Fixture\\Typed folder");
    await page.evaluate((position) => window.dispatchEvent(new CustomEvent("fixture-file-drag", {
        detail: { type: "drop", ...position, paths: ["C:\\Fixture\\Dropped folder"] }
    })), position);
    await expect(input).toHaveValue("C:\\Fixture\\Dropped folder");
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([]);
    await host.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(userRow(page)).toBeVisible();
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([{ method: "packages.userPackages.enroll", params: { sourcePath: "C:\\Fixture\\Dropped folder" } }]);
});

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
    await expectRequestError(host.getByRole("alert"), "store_unavailable");
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
    await page.evaluate((id) => { window.dispatchEvent(new Event("test-user-package-advance")); window.coreEvents.emit("user-package", id, "user_package.refreshed", 2); }, userPackageId);
    await page.getByRole("button", { name: "Details", exact: true }).click();
    expect((await requests(page, "packages.userPackages.get")).length).toBe(initialReads);
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-details-ready")));
    await expect.poll(async () => (await requests(page, "packages.userPackages.get")).length).toBeGreaterThan(initialReads);
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-details-ready")));
    await expect(page.locator("md-dialog[open]").getByText("1.3.0", { exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByText("1.2.3", { exact: true })).toHaveCount(0);
});

test("cancelled directory selection does not enroll a User Package", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1&cancel-directory=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Select", exact: true }).click();
    await expect(page.getByText("No User Packages enrolled", { exact: true })).toBeVisible();
    expect(await requests(page, "selectDirectory")).toHaveLength(1);
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([]);
});

test("User Package enrollment, current details, refresh and confirmed removal use their existing RPCs", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&resource-lifecycle=1");
    await page.getByRole("button", { name: "Enroll folder", exact: true }).click();
    const picker = page.locator("md-dialog[open]");
    await picker.getByRole("button", { name: "Select", exact: true }).click();
    expect(await requests(page, "packages.userPackages.enroll")).toEqual([]);
    await picker.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(picker).toHaveCount(0);
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

test("User Package revision conflicts reload the list without retrying the mutation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&resource-lifecycle=1");
    const row = userRow(page);
    await expect(row).toContainText("revision 1");
    await page.evaluate(() => window.dispatchEvent(new Event("test-user-package-advance")));
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(row).toContainText("revision 2");
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    expect(await requests(page, "packages.userPackages.refresh")).toHaveLength(1);
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

test("repository authentication is write-only through registration, replacement and clearing", async ({ page }) => {
    const token = "fixture-credential-value";
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await page.getByRole("button", { name: "Add repository", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("textbox", { name: "Repository URL", exact: true }).fill("https://repository.test/index.json");
    await dialog.getByRole("button", { name: "Add request header", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Header name", exact: true }).fill("Authorization");
    await dialog.locator("input[type=password]").fill(token);
    await dialog.getByRole("button", { name: "Review repository", exact: true }).click();
    await expect(dialog.getByText("Uses authentication", { exact: true })).toBeVisible();
    await expect(dialog).not.toContainText(token);
    await dialog.getByRole("button", { name: "Add repository", exact: true }).click();
    const row = repositoryRow(page, "remote");
    await row.getByRole("button", { name: "Manage repository", exact: true }).click();
    await expect(dialog.getByText("Uses authentication", { exact: true })).toBeVisible();
    await expect(dialog.locator("input[type=password]")).toHaveCount(0);
    await dialog.getByRole("button", { name: "Add request header", exact: true }).click();
    await dialog.getByRole("textbox", { name: "Header name", exact: true }).fill("Authorization");
    await dialog.locator("input[type=password]").fill(`${token}-new`);
    await dialog.getByRole("button", { name: "Save", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await row.getByRole("button", { name: "Manage repository", exact: true }).click();
    await dialog.getByRole("button", { name: "Clear authentication", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await row.getByRole("button", { name: "Manage repository", exact: true }).click();
    await expect(dialog.getByRole("button", { name: "Clear authentication", exact: true })).toHaveCount(0);
    expect(await requests(page, "repositories.inspect")).toEqual([expect.objectContaining({ params: expect.objectContaining({ credentialsConfigured: true }) })]);
    expect(await requests(page, "repositories.register")).toEqual([expect.objectContaining({ params: expect.objectContaining({ credentialsConfigured: true }) })]);
    expect(await requests(page, "repositories.setCredentials")).toEqual([true, false].map((credentialsConfigured) => expect.objectContaining({ params: expect.objectContaining({ credentialsConfigured }) })));
    expect(await page.evaluate(() => JSON.stringify(window.resourceRequests))).not.toContain(token);
});

test("repository refresh failure retains the last catalog until a successful explicit refresh", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1&resource-refresh-error=1");
    await registerRepository(page, "remote");
    const row = repositoryRow(page, "remote");
    const manage = row.getByRole("button", { name: "Manage repository", exact: true });
    const dialog = page.locator("md-dialog[open]");
    const refresh = dialog.getByRole("button", { name: "Refresh repository", exact: true });
    await manage.click();
    await refresh.click();
    await expect.poll(() => requests(page, "repositories.refresh")).toHaveLength(1);
    await expect(refresh).toBeEnabled();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(row).toBeVisible();
    await row.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(page.locator("md-dialog[open]").getByText("First catalog package", { exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Close", exact: true }).click();
    await manage.click();
    await refresh.click();
    await expect(refresh).toBeEnabled();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await row.getByRole("button", { name: "Browse packages", exact: true }).click();
    await expect(page.locator("md-dialog[open]").getByText("Updated catalog package", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.refresh")).toHaveLength(2);
});

test("repository revision conflicts reload the list without retrying the mutation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await registerRepository(page, "remote");
    const row = repositoryRow(page, "remote");
    await page.evaluate(() => window.dispatchEvent(new Event("test-resource-repository-advance")));
    await row.getByRole("button", { name: "Manage repository", exact: true }).click();
    const refresh = page.locator("md-dialog[open]").getByRole("button", { name: "Refresh repository", exact: true });
    await refresh.click();
    await expect(row).toContainText("Example packages (updated)");
    expect(await requests(page, "repositories.refresh")).toHaveLength(1);
    await expect(page.locator('.page-header [data-icon-name="refresh"]')).toHaveCount(0);
    await refresh.click();
    await expect(refresh).toBeEnabled();
    await expect(page.getByRole("alert")).toHaveCount(0);
    await expect(row.getByRole("status")).toHaveCount(0);
    expect((await requests(page, "repositories.refresh")).map((request) => (request.params as { expectedRevision: number }).expectedRevision)).toEqual([1, 2]);
});

test("a failed registration stays in review context and Back clears its previous error", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1&resource-register-invalid=1");
    await reviewRepository(page, "remote");
    const host = page.locator("md-dialog[open]");
    await host.getByRole("button", { name: "Add repository", exact: true }).click();
    await expectRequestError(host.getByRole("alert"), "repository_document_invalid");
    await expect(host.locator(".dialog-summary")).toBeVisible();
    await host.getByRole("button", { name: "Back", exact: true }).click();
    await expect(host.getByRole("alert")).toHaveCount(0);
    await expect(host.getByLabel("Repository URL", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toHaveLength(1);
});

test("invalid repository inspection and duplicate registration preserve the registered list", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1&resource-invalid-source=1");
    await reviewRepository(page, "remote", true);
    await expect(page.locator("#repository-source")).toHaveJSProperty("error", true);
    await expect.poll(() => page.locator("#repository-source").evaluate((field) => (field as HTMLElement & { errorText: string }).errorText)).toBe("Check the repository URL or manifest path.");
    expect(await requests(page, "repositories.register")).toEqual([]);
    await page.goto("/browser-harness.html?route=/repositories&resource-lifecycle=1");
    await registerRepository(page, "remote");
    await reviewRepository(page, "remote");
    await page.locator("md-dialog[open]").getByRole("button", { name: "Add repository", exact: true }).click();
    await expectRequestError(page.getByRole("alert"), "repository_already_registered");
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
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeHidden();
    await expect(repositoryRow(page, kind)).toBeVisible();
}
