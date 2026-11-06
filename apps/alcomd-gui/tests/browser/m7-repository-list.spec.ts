import { expect, test, type Page } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";
import { expectRequestError } from "./request-error-assertions";

async function openList(page: Page, action: "Import list" | "Export list") {
    await page.getByRole("button", { name: "Repository list actions", exact: true }).click();
    await page.getByRole("menuitem", { name: action, exact: true }).click();
    await expect(page.getByRole("dialog", { name: action, exact: true })).toBeVisible();
    return page.locator("md-dialog[open]");
}
async function requests(page: Page, method: string) {
    return page.evaluate((name) => window.resourceRequests.filter((entry) => entry.method === name), method);
}
function expectedImport() {
    return { method: "repositories.importListFile", params: { path: "C:/fixture/repositories.txt", expectedFingerprint: "a".repeat(64), idempotencyKey: expect.any(String) } };
}
async function review(page: Page, candidateCount = 3) {
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    await expect(host.getByText("Duplicate", { exact: true })).toBeVisible();
    await expect(host.getByText("Invalid", { exact: true })).toBeVisible();
    await expect(host.getByText("Unsupported", { exact: true })).toBeVisible();
    await expect(host.getByRole("button", { name: `Add ${candidateCount} repositories`, exact: true })).toBeEnabled();
    await expect(host.getByRole("checkbox")).toHaveCount(0);
    expect(await requests(page, "repositories.inspect")).toEqual([]);
    expect(await requests(page, "repositories.register")).toEqual([]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
    return host;
}

test("built-in repositories show their names and sync state without a remove action", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&builtin-repositories=1");
    const official = page.getByRole("row").filter({ hasText: "Official" });
    const curated = page.getByRole("row").filter({ hasText: "Curated" });
    await expect(official).toContainText("https://vrchat.github.io/packages/index.json");
    await expect(curated).toContainText("https://vrchat-community.github.io/vpm-listing-curated/index.json");
    await expect(page.getByRole("button", { name: "Remove", exact: true })).toHaveCount(1);
    await official.getByRole("button", { name: "Browse packages" }).click();
    await expect(page.getByRole("dialog", { name: "Official" })).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByText("Pending first sync")).toBeVisible();
    expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
});

test("long preview content scrolls independently of actions and closing restores the menu trigger", async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 800 });
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    for (const name of ["Close", "Select", "Add 3 repositories"]) await expect(host.getByRole("button", { name, exact: true })).toBeInViewport();
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("button", { name: "Repository list actions", exact: true })).toBeFocused();
    const exportHost = await openList(page, "Export list");
    for (const name of ["Close", "Preview again", "Save new list file"]) await expect(exportHost.getByRole("button", { name, exact: true })).toBeInViewport();
    expect(await exportHost.locator(".material-dialog-content").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});

test("one confirmation adds every candidate once without downloading a preview", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success");
    const host = await review(page);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 3 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    expect(await requests(page, "repositories.register")).toEqual([]);
    expect(await requests(page, "repositories.inspect")).toEqual([]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
    expect(await requests(page, "repositories.unregister")).toEqual([]);
});

test("large lists are sent to Core in one file request and reload once", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=large");
    const host = await review(page, 67);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect(host.getByText("Added: 67 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
});

test("list import keeps partial failures visible without retrying or claiming rollback", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/repositories&list-import=partial");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 2 · Failed: 1 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    await page.clock.fastForward(10_000);
    await expect(host.getByText(requestErrorMessage("repository_unavailable"), { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeVisible();
});

test("an ordered batch result preserves added, unknown, and unprocessed rows", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=unknown");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 1 · Failed: 0 · Unknown: 1 · Not processed: 1", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect(host.getByText("Result unknown", { exact: true })).toBeVisible();
});

test("stopping forwards the current import key and displays Core outcomes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=hold");
    const host = await review(page, 67);
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect.poll(() => requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    const [submission] = await requests(page, "repositories.importListFile");
    await host.getByRole("button", { name: "Stop import", exact: true }).click();
    const { idempotencyKey } = submission!.params as { idempotencyKey: string };
    await expect.poll(() => requests(page, "repositories.cancelListRegistration")).toEqual([
        { method: "repositories.cancelListRegistration", params: { idempotencyKey } }
    ]);
    await page.evaluate(() => window.dispatchEvent(new Event("test-list-register-ready")));
    await expect(host.getByText("Added: 64 · Failed: 0 · Unknown: 0 · Not processed: 3", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
});

test("transport loss shows an unresolved import without inventing per-line outcomes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=transport");
    const host = await review(page, 67);
    const readsBefore = (await requests(page, "repositories.list")).length;
    await host.getByRole("button", { name: "Add 67 repositories", exact: true }).click();
    await expect(host.getByRole("alert")).toContainText(requestErrorMessage("daemon_unavailable"));
    await expect(host.getByText("Completed additions are kept. Check the repository list before importing again.", { exact: true })).toBeVisible();
    await expect(host.getByText("Result unknown", { exact: true })).toHaveCount(0);
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
    await expect.poll(async () => (await requests(page, "repositories.list")).length).toBe(readsBefore + 1);
});

test("cancelled list file picker performs no inspection or registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Import list");
test("a typed repository list path must be reviewed again when it changes", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Import list");
    const input = host.getByLabel("Repository list file", { exact: true });
    await input.fill("C:/fixture/typed.txt");
    await input.press("Enter");
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    await input.fill("C:/fixture/changed.txt");
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toHaveCount(0);
    await host.getByRole("button", { name: "Review list", exact: true }).click();
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([
        { method: "repositories.inspectListFile", params: { path: "C:/fixture/typed.txt" } },
        { method: "repositories.inspectListFile", params: { path: "C:/fixture/changed.txt" } }
    ]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

    await host.getByRole("button", { name: "Select", exact: true }).click();
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([]);
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

test("an already registered source is skipped and the batch continues", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=duplicate");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 2 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    await expect(host.getByText("Already added", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.importListFile")).toEqual([expectedImport()]);
});

test("lists with no candidates cannot start registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=invalid");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Select", exact: true }).click();
    await expect(host.getByText("Invalid", { exact: true })).toBeVisible();
    await expect(host.getByRole("button", { name: "Add 0 repositories", exact: true })).toBeDisabled();
    expect(await requests(page, "repositories.importListFile")).toEqual([]);
});

test("a repository without published checksums adds and displays packages normally", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    const row = host.locator("li").filter({ has: page.getByText("Line 1", { exact: true }) });
    const warning = "Package versions without checksums cannot be installed.";
    await expect(row.getByText("Added", { exact: true })).toBeVisible();
    await expect(row.getByText(warning, { exact: true })).toHaveCount(0);
    await page.goto("/browser-harness.html?route=/repositories/00000000-0000-4000-8000-000000000102");
    await expect(page.getByText("Avatar tools", { exact: true }).first()).toBeVisible();
    await expect(page.getByText(warning, { exact: true })).toHaveCount(0);
});

test("cancelled native save selection never sends a Core file write", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
});

test("export validates a new name near its field and binds it to folder selection", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByLabel("New list file name", { exact: true }).fill("../existing.txt");
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
    await expect(host.locator("#repository-list-file-name")).toHaveJSProperty("error", true);
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
    await host.getByLabel("New list file name", { exact: true }).fill("owner-review.txt");
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByText("Saved 230 repositories (12000 bytes).", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.exportListFile")).toEqual([{ method: "repositories.exportListFile", params: { targetPath: "C:/fixture/owner-review.txt", expectedFingerprint: "1".repeat(64) } }]);
});

test("export reviews full Core scope and exclusions and requires address-risk confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Export list");
    await expect(host.getByText("230 repositories can be exported · 1 excluded · 12000 bytes", { exact: true })).toBeVisible();
    await expect(host.getByText("Local source cannot be represented by this URL list.", { exact: true })).toBeVisible();
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByText("Saved 230 repositories (12000 bytes).", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.exportListFile")).toEqual([{ method: "repositories.exportListFile", params: { targetPath: "C:/fixture/repositories.txt", expectedFingerprint: "1".repeat(64) } }]);
});

for (const code of ["repository_list_target_exists", "repository_list_changed", "repository_list_io_error"]) {
    test(`export preserves ${code} in context and never automatically writes again`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories&list-export-error=${code}`);
        const host = await openList(page, "Export list");
        const consent = host.getByRole("checkbox", { name: "I have reviewed the export scope and understand the file can contain credentials.", exact: true });
        await consent.check();
        await host.getByRole("button", { name: "Save new list file", exact: true }).click();
        await expectRequestError(host.locator("[role=alert]"), code);
        expect(await requests(page, "repositories.exportListFile")).toHaveLength(1);
        if (code === "repository_list_changed") {
            await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeDisabled();
            await host.getByRole("button", { name: "Preview again", exact: true }).click();
            await expect(consent).not.toBeChecked();
            await consent.check();
        }
        await host.getByRole("button", { name: "Save new list file", exact: true }).click();
        await expect(host.getByText("Saved 230 repositories (12000 bytes).", { exact: true })).toBeVisible();
        const calls = await requests(page, "repositories.exportListFile");
        expect(calls).toHaveLength(2);
        if (code === "repository_list_changed") expect(calls[1]?.params).toEqual({ targetPath: "C:/fixture/repositories.txt", expectedFingerprint: "2".repeat(64) });
    });
}
