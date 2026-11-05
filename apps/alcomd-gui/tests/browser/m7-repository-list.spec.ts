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
async function review(page: Page) {
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Choose list file", exact: true }).click();
    await expect(host.getByText("Duplicate", { exact: true })).toBeVisible();
    await expect(host.getByText("Invalid", { exact: true })).toBeVisible();
    await expect(host.getByText("Unsupported", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toEqual([]);
    await host.getByRole("button", { name: "Check selected repositories", exact: true }).click();
    await expect(host.getByRole("button", { name: "Add 3 repositories", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.register")).toEqual([]);
    return host;
}

test("long preview content scrolls independently of actions and closing restores the menu trigger", async ({ page }) => {
    await page.setViewportSize({ width: 1400, height: 800 });
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Choose list file", exact: true }).click();
    for (const name of ["Close", "Choose list file", "Check selected repositories"]) await expect(host.getByRole("button", { name, exact: true })).toBeInViewport();
    await host.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("button", { name: "Repository list actions", exact: true })).toBeFocused();
    const exportHost = await openList(page, "Export list");
    for (const name of ["Close", "Preview again", "Save new list file"]) await expect(exportHost.getByRole("button", { name, exact: true })).toBeInViewport();
    expect(await exportHost.locator(".material-dialog-content").evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});

test("list import previews classifications and requires selection and confirmation before appending", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success");
    const host = await review(page);
    const checked = await requests(page, "repositories.inspect");
    expect(checked).toHaveLength(3);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 3 · Failed: 0 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toEqual(checked.map((entry) => ({ ...entry, method: "repositories.register" })));
    expect(JSON.stringify(checked)).toContain("?z=2&a=1");
    expect(await requests(page, "repositories.unregister")).toEqual([]);
});

test("list import keeps partial failures visible without retrying or claiming rollback", async ({ page }) => {
    await page.clock.install();
    await page.goto("/browser-harness.html?route=/repositories&list-import=partial");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 2 · Failed: 1 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    await page.clock.fastForward(10_000);
    await expect(host.getByText(requestErrorMessage("repository_unavailable"), { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toHaveLength(3);
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeVisible();
});

test("unknown registration result stops subsequent writes and remains distinct from failure", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=unknown");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("Added: 1 · Failed: 0 · Unknown: 1 · Not processed: 1", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toHaveLength(2);
    await expect(host.getByText("Result unknown", { exact: true })).toBeVisible();
});

test("cancellation waits for the in-flight registration and starts no later entry", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=hold");
    const host = await review(page);
    await host.getByRole("button", { name: "Add 3 repositories", exact: true }).click();
    await expect(host.getByText("In flight", { exact: true })).toBeVisible();
    await host.getByRole("button", { name: "Stop after current request", exact: true }).click();
    expect(await requests(page, "repositories.register")).toHaveLength(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-list-register-ready")));
    await expect(host.getByText("Added: 1 · Failed: 0 · Unknown: 0 · Not processed: 2", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toHaveLength(1);
});

test("cancelled list file picker performs no inspection or registration", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Choose list file", exact: true }).click();
    await expect(host.getByRole("button", { name: "Close", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.inspectListFile")).toEqual([]);
    expect(await requests(page, "repositories.register")).toEqual([]);
});

test("only selected and successfully checked entries are offered for explicit append", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-import=success&list-check=partial");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Choose list file", exact: true }).click();
    await host.getByRole("checkbox", { name: "Select line 3", exact: true }).uncheck();
    await host.getByRole("button", { name: "Check selected repositories", exact: true }).click();
    await expect(host.getByText("Ready: 1 · Check failed: 1 · Not processed: 0", { exact: true })).toBeVisible();
    await expect(host.getByText("Not selected", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.inspect")).toHaveLength(2);
    expect(await requests(page, "repositories.register")).toHaveLength(0);
    await host.getByRole("button", { name: "Add 1 repositories", exact: true }).click();
    await expect(host.getByText("Added: 1 · Failed: 1 · Unknown: 0 · Not processed: 0", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.register")).toHaveLength(1);
});

test("cancelling read-only repository checks does not register or start the next check", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-check=hold");
    const host = await openList(page, "Import list");
    await host.getByRole("button", { name: "Choose list file", exact: true }).click();
    await host.getByRole("button", { name: "Check selected repositories", exact: true }).click();
    await expect(host.getByText("Checking repository", { exact: true })).toBeVisible();
    await host.getByRole("button", { name: "Stop after current request", exact: true }).click();
    await page.evaluate(() => window.dispatchEvent(new Event("test-list-check-ready")));
    await expect(host.getByText("Ready: 1 · Check failed: 0 · Not processed: 2", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.inspect")).toHaveLength(1);
    expect(await requests(page, "repositories.register")).toEqual([]);
});

test("cancelled native save selection never sends a Core file write", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&list-picker=cancel");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and the address-sharing risk.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByRole("button", { name: "Save new list file", exact: true })).toBeEnabled();
    expect(await requests(page, "repositories.exportListFile")).toEqual([]);
});

test("export validates a new name near its field and binds it to folder selection", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories");
    const host = await openList(page, "Export list");
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and the address-sharing risk.", exact: true }).check();
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
    await host.getByRole("checkbox", { name: "I have reviewed the export scope and the address-sharing risk.", exact: true }).check();
    await host.getByRole("button", { name: "Save new list file", exact: true }).click();
    await expect(host.getByText("Saved 230 repositories (12000 bytes).", { exact: true })).toBeVisible();
    expect(await requests(page, "repositories.exportListFile")).toEqual([{ method: "repositories.exportListFile", params: { targetPath: "C:/fixture/repositories.txt", expectedFingerprint: "1".repeat(64) } }]);
});

for (const code of ["repository_list_target_exists", "repository_list_changed", "repository_list_io_error"]) {
    test(`export preserves ${code} in context and never automatically writes again`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories&list-export-error=${code}`);
        const host = await openList(page, "Export list");
        const consent = host.getByRole("checkbox", { name: "I have reviewed the export scope and the address-sharing risk.", exact: true });
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
