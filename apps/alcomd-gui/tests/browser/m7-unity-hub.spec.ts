import { expect, test, type Page } from "@playwright/test";

const path = "C:\\Tools\\Unity Hub\\Unity Hub.exe";
const section = (page: Page) => page.getByRole("region", { name: "Unity Hub Path", exact: true });
const requests = (page: Page, method: string) => page.evaluate((name) => window.unityRequests.filter((request) => request.method === name).length, method);
async function open(page: Page, query = "") {
    await page.setViewportSize({ width: 1400, height: 800 });
    await page.goto(`/browser-harness.html?route=/settings&${query}`);
    await expect(section(page).getByRole("button", { name: "Select", exact: true })).toBeVisible();
}
const input = (page: Page) => section(page).locator("md-outlined-text-field input");
const value = (page: Page) => section(page).locator("md-outlined-text-field").evaluate((element) => (element as HTMLElement & { value: string }).value);

test("Hub picker cancellation performs no save or Editor discovery", async ({ page }) => {
    await open(page, "hub-picker=cancel");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect.poll(() => requests(page, "selectUnityHub")).toBe(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
});

test("Hub selection saves one narrow update, refreshes once and survives navigation", async ({ page }) => {
    await open(page);
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect.poll(() => value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { unity: { hubPath: path } } }]);
    await expect.poll(() => requests(page, "unityInstallationsRefresh")).toBe(1);
    await page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Projects$/ }) }).click();
    await page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Settings$/ }) }).click();
    await expect.poll(() => value(page)).toBe(path);
    expect(await section(page).locator("md-outlined-text-field").evaluate((element) => (element as HTMLElement & { readOnly: boolean }).readOnly)).toBe(false);
});

test("rapid repeated selection opens one picker and keeps accepted path while pending", async ({ page }) => {
    await open(page, "hub-picker=hold");
    const select = section(page).getByRole("button", { name: "Select", exact: true });
    await select.evaluate((element) => { (element as HTMLElement).click(); (element as HTMLElement).click(); });
    await expect.poll(() => requests(page, "selectUnityHub")).toBe(1);
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await page.evaluate(() => window.dispatchEvent(new Event("test-hub-picker-ready")));
    await expect.poll(() => value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
});

test("failed Hub save retains accepted evidence and Retry reconciles before saving", async ({ page }) => {
    await open(page, "theme-save=error-once");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub path could not be saved.");
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(2);
    await expect.poll(() => requests(page, "unityInstallationsRefresh")).toBe(1);
});

test("an old picker result cannot save after leaving and reopening Settings", async ({ page }) => {
    await open(page, "hub-picker=hold");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect.poll(() => requests(page, "selectUnityHub")).toBe(1);
    const navigation = (name: string) => page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });
    await navigation("Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await navigation("Settings").click();
    await expect(section(page).getByRole("button", { name: "Select", exact: true })).toBeEnabled();
    await page.evaluate(() => window.dispatchEvent(new Event("test-hub-picker-ready")));
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
});

test("lost Hub save reply reconciles without duplicate write", async ({ page }) => {
    await open(page, "theme-save=commit-error-once");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(section(page).getByRole("alert")).toBeVisible();
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
});

test("ambiguous normalized Hub save requires reselect instead of a second write", async ({ page }) => {
    await open(page, "theme-save=commit-error-once&hub-normalize=1");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(section(page).getByRole("alert")).toBeVisible();
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub path changed.");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
    expect(await value(page)).toBe("C:\\Normalized\\Unity Hub.exe");
});

test("Editor refresh failure is separate from a confirmed Hub save", async ({ page }) => {
    await open(page, "hub-refresh=error");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub path was saved, but Unity installations could not be refreshed.");
    expect(await value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsWrites.length)).toBe(1);
});

test("saving Hub path does not require Unity manage capability", async ({ page }) => {
    await open(page, "noUnityManage=1");
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect.poll(() => value(page)).toBe(path);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
});

test("Auto detect rereads same-revision evidence without picker, config write or Editor refresh", async ({ page }) => {
    await open(page);
    const reads = await page.evaluate(() => window.settingsReads);
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
    await page.evaluate(() => window.dispatchEvent(new Event("test-hub-default-ready")));
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).evaluate((element) => {
        (element as HTMLElement).click(); (element as HTMLElement).click();
    });
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsReads)).toBe(reads + 1);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    expect(await requests(page, "selectUnityHub")).toBe(0);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
});

test("Auto detect clears a manual override once and shows Core default evidence", async ({ page }) => {
    await open(page, `hub-path=${encodeURIComponent(path)}&hub-default=1`);
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { unity: { hubPath: null } } }]);
    expect(await requests(page, "selectUnityHub")).toBe(0);
    await expect.poll(() => requests(page, "unityInstallationsRefresh")).toBe(1);
});

test("failed automatic detection retries a read without changing preferences", async ({ page }) => {
    await open(page);
    await page.evaluate(() => {
        window.dispatchEvent(new Event("test-hub-default-ready"));
        window.dispatchEvent(new Event("test-hub-read-error"));
    });
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub location could not be detected.");
    expect(await value(page)).toBe("");
    await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", "Unity Hub was not found.");
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
});

test("failed manual-to-auto clear retains null intent for Retry", async ({ page }) => {
    await open(page, `hub-path=${encodeURIComponent(path)}&hub-default=1&theme-save=error-once`);
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    await expect(section(page).getByRole("alert")).toBeVisible();
    expect(await value(page)).toBe(path);
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ unity: { hubPath: null } }, { unity: { hubPath: null } }]);
    await expect.poll(() => requests(page, "unityInstallationsRefresh")).toBe(1);
});

test("lost null-clear reply reconciles automatic mode without a duplicate write", async ({ page }) => {
    await open(page, `hub-path=${encodeURIComponent(path)}&hub-default=1&theme-save=commit-error-once`);
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    await expect(section(page).getByRole("alert")).toBeVisible();
    expect(await value(page)).toBe(path);
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { unity: { hubPath: null } } }]);
    await expect.poll(() => requests(page, "unityInstallationsRefresh")).toBe(1);
});

test("direct typing stays local, Enter saves once and preserves focus across Enter plus blur", async ({ page }) => {
    await open(page, "theme-save=hold");
    await input(page).fill(path);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).press("Enter");
    await input(page).press("Enter");
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await expect(input(page)).toBeFocused();
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(input(page)).toBeFocused();
    const outside = page.getByRole("checkbox", { name: "Compact GUI", exact: true });
    await outside.focus();
    await expect(outside).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([{ expectedRevision: 7, update: { unity: { hubPath: path } } }]);
});

test("leaving a changed path commits once without stealing the next control focus", async ({ page }) => {
    await open(page);
    await input(page).fill(path);
    const outside = page.getByRole("checkbox", { name: "Compact GUI", exact: true });
    await outside.focus();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(outside).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests[0]!.update)).toEqual({ unity: { hubPath: path } });
});

test("unchanged automatic path focus and blur does not create a manual override", async ({ page }) => {
    await open(page, "hub-default=1");
    expect(await value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    await input(page).focus();
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).focus();
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
});

test("Escape cancels the draft and composing Enter cannot submit", async ({ page }) => {
    await open(page, `hub-path=${encodeURIComponent(path)}`);
    await input(page).fill("C:\\Unsaved\\Unity Hub.exe");
    await input(page).evaluate((element) => element.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", isComposing: true, keyCode: 229, bubbles: true, composed: true })));
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).press("Escape");
    expect(await value(page)).toBe(path);
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).focus();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
});

test("invalid draft remains visible, unchanged blur does not retry, and corrected Retry keeps errors stable", async ({ page }) => {
    await open(page, "hub-reject-invalid=1&theme-save=hold");
    const invalid = "C:\\Invalid\\Unity Hub.exe";
    await input(page).fill(invalid);
    await input(page).press("Enter");
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub path could not be saved. Check the path and try again.");
    expect(await value(page)).toBe(invalid);
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).focus();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await input(page).fill("");
    await expect(section(page).getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await input(page).fill(invalid);
    await input(page).press("Enter");
    await expect(section(page).getByRole("alert")).toBeVisible();
    await input(page).fill(path);
    await section(page).getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(3);
    await expect(section(page).getByRole("alert")).toBeVisible();
    expect(await page.evaluate(() => window.settingsRequests.map((request) => request.update))).toEqual([{ unity: { hubPath: invalid } }, { unity: { hubPath: invalid } }, { unity: { hubPath: path } }]);
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(section(page).getByRole("alert")).toHaveCount(0);
});

test("Auto and picker actions suppress draft blur, cancellation keeps it editable until group exit", async ({ page }) => {
    await open(page, "hub-default=1&hub-picker=cancel");
    await input(page).fill("C:\\First draft\\Unity Hub.exe");
    await section(page).getByRole("button", { name: "Auto detect", exact: true }).click();
    await expect.poll(() => value(page)).toBe("C:\\Program Files\\Unity Hub\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).fill(path);
    await section(page).getByRole("button", { name: "Select", exact: true }).click();
    await expect(input(page)).toBeFocused();
    expect(await value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).press("Shift+Tab");
    const openAction = section(page).getByRole("button", { name: "Open Unity Hub", exact: true });
    await expect(openAction).toBeFocused();
    await expect(openAction).toBeDisabled();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await openAction.press("Shift+Tab");
    const automatic = section(page).getByRole("button", { name: "Auto detect", exact: true });
    await expect(automatic).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await automatic.press("Tab");
    await expect(openAction).toBeFocused();
    await openAction.press("Tab");
    await expect(input(page)).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).press("Tab");
    const select = section(page).getByRole("button", { name: "Select", exact: true });
    await expect(select).toBeFocused();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await select.press("Tab");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    expect(await page.evaluate(() => window.settingsRequests[0]!.update)).toEqual({ unity: { hubPath: path } });
});

test("no-path statuses are placeholders and an explicitly empty edit never sends a request", async ({ page }) => {
    for (const query of ["", "hub-unavailable=1"]) {
        await open(page, query);
        expect(await value(page)).toBe("");
        await expect(section(page).locator("md-outlined-text-field")).toHaveJSProperty("placeholder", query === "" ? "Unity Hub was not found." : "Unity Hub location is unavailable.");
        await input(page).focus();
        await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).focus();
        expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    }
    await open(page, `hub-path=${encodeURIComponent(path)}`);
    await input(page).fill("");
    await input(page).press("Enter");
    await expect(section(page).getByRole("alert")).toContainText("Enter a Unity Hub executable path, or use Auto detect.");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
});

test("successful direct edit displays the Core-normalized accepted path", async ({ page }) => {
    await open(page, "hub-normalize=1");
    await input(page).fill(path);
    await input(page).press("Enter");
    await expect.poll(() => value(page)).toBe("C:\\Normalized\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
});

test("a broken saved path stays editable and correction uses Core validation", async ({ page }) => {
    const missing = "C:\\Missing\\Unity Hub.exe";
    await open(page, `hub-path=${encodeURIComponent(missing)}&hub-missing=1`);
    expect(await value(page)).toBe(missing);
    await expect(section(page)).toContainText("The saved Unity Hub path is no longer usable.");
    await input(page).fill(path);
    await input(page).press("Enter");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(section(page)).not.toContainText("The saved Unity Hub path is no longer usable.");
    expect(await value(page)).toBe(path);
});

test("accepted ordinary updates do not overwrite an in-progress Hub draft", async ({ page }) => {
    await open(page);
    await input(page).fill(path);
    await page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true }).evaluate((element) => {
        (element as HTMLElement).click();
    });
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    expect(await value(page)).toBe(path);
    expect(await page.evaluate(() => window.settingsRequests[0]!.update)).toEqual({ packages: { showPrerelease: true } });
    await input(page).press("Enter");
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests[1])).toEqual({ expectedRevision: 8, update: { unity: { hubPath: path } } });
});

test("a blur commit queues once behind an unrelated ordinary settings write", async ({ page }) => {
    await open(page, "theme-save=hold");
    await page.getByRole("checkbox", { name: "Show prerelease package versions", exact: true }).evaluate((element) => (element as HTMLElement).click());
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await input(page).fill(path);
    await page.getByRole("checkbox", { name: "Compact GUI", exact: true }).focus();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
    await expect.poll(() => page.evaluate(() => window.settingsRequests.length)).toBe(2);
    expect(await page.evaluate(() => window.settingsRequests[1])).toEqual({ expectedRevision: 8, update: { unity: { hubPath: path } } });
    await page.evaluate(() => window.dispatchEvent(new Event("test-theme-save-ready")));
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    expect(await value(page)).toBe(path);
});

test("Open Unity Hub sends accepted revision and Core path without saving or refreshing Editors", async ({ page }) => {
    await open(page, "hub-default=1");
    await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).click();
    await expect.poll(() => requests(page, "unityHubOpen")).toBe(1);
    expect(await page.evaluate(() => window.unityRequests.find((request) => request.method === "unityHubOpen")!.params)).toEqual({ expectedRevision: 7, expectedPath: "C:\\Program Files\\Unity Hub\\Unity Hub.exe" });
    expect(await requests(page, "openUnityHubDownload")).toBe(0);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await expect(section(page).getByRole("alert")).toHaveCount(0);
});

test("Download Unity Hub opens the closed browser adapter only after the Core decision", async ({ page }, testInfo) => {
    await open(page);
    await section(page).getByRole("button", { name: "Download Unity Hub", exact: true }).click();
    await expect.poll(() => requests(page, "openUnityHubDownload")).toBe(1);
    expect(await page.evaluate(() => window.unityRequests)).toEqual([
        { method: "unityHubOpen", params: { expectedRevision: 7, expectedPath: null } },
        { method: "openUnityHubDownload", params: {} }
    ]);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    expect(await requests(page, "unityInstallationsRefresh")).toBe(0);
    await page.screenshot({ path: testInfo.outputPath("hub-download.png"), animations: "disabled" });
});

test("missing capability, unavailable evidence and broken manual paths cannot open or download", async ({ page }) => {
    for (const query of ["hub-default=1&noHubOpen=1", "hub-unavailable=1", `hub-path=${encodeURIComponent(path)}&hub-missing=1`]) {
        await open(page, query);
        const action = section(page).getByRole("button", { name: "Open Unity Hub", exact: true });
        await expect(action).toBeDisabled();
        const reason = query.includes("noHubOpen") ? "Opening Unity Hub is unavailable on this connection." : "Choose a usable Unity Hub path or use Auto detect before opening it.";
        const host = section(page).locator("md-filled-button").filter({ has: page.getByRole("button", { name: "Open Unity Hub", exact: true }) });
        await expect(host).toHaveAttribute("title", reason);
        await expect(host).toHaveAttribute("aria-describedby", "settings-unity-hub-open-reason");
        await expect(section(page).locator("#settings-unity-hub-open-reason")).toHaveText(reason);
        await action.evaluate((element) => (element as HTMLElement).click());
        expect(await requests(page, "unityHubOpen")).toBe(0);
        expect(await requests(page, "openUnityHubDownload")).toBe(0);
    }
});

test("dirty path disables Open without a blur save or keyboard launch", async ({ page }) => {
    await open(page, "hub-default=1");
    await input(page).fill(path);
    const action = section(page).getByRole("button", { name: "Open Unity Hub", exact: true });
    await expect(action).toBeDisabled();
    const host = section(page).locator("md-filled-button").filter({ has: page.getByRole("button", { name: "Open Unity Hub", exact: true }) });
    await expect(host).toHaveAttribute("title", "Finish or cancel the path edit before continuing.");
    await expect(section(page).locator("#settings-unity-hub-open-reason")).toHaveText("Finish or cancel the path edit before continuing.");
    await action.focus();
    await action.press("Enter");
    await action.press("Space");
    expect(await requests(page, "unityHubOpen")).toBe(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await input(page).focus();
    await input(page).press("Escape");
    await expect(action).toBeEnabled();
});

test("rapid mouse and keyboard repeats produce one Hub open request", async ({ page }) => {
    await open(page, "hub-default=1&hub-open=hold");
    const action = section(page).getByRole("button", { name: "Open Unity Hub", exact: true });
    await action.evaluate((element) => { (element as HTMLElement).click(); (element as HTMLElement).click(); });
    await expect.poll(() => requests(page, "unityHubOpen")).toBe(1);
    await expect(action).toBeDisabled();
    await action.focus();
    await action.press("Enter");
    await action.press("Space");
    expect(await requests(page, "unityHubOpen")).toBe(1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-hub-open-ready")));
    await expect(action).toBeEnabled();
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
});

test("a stale Hub target rereads authority and requires a fresh explicit click", async ({ page }) => {
    await open(page, "hub-default=1&hub-open=stale");
    const reads = await page.evaluate(() => window.settingsReads);
    await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("Unity Hub location changed. Review the current path and click again.");
    expect(await value(page)).toBe("C:\\Changed\\Unity Hub.exe");
    expect(await page.evaluate(() => window.settingsReads)).toBe(reads + 1);
    expect(await requests(page, "unityHubOpen")).toBe(1);
    expect(await requests(page, "openUnityHubDownload")).toBe(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
    await expect(section(page).getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
});

test("Open and Download labels reserve one width when stale evidence changes the action", async ({ page }) => {
    await open(page, "hub-default=1&hub-open=gone");
    const before = await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).boundingBox();
    await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).click();
    const download = section(page).getByRole("button", { name: "Download Unity Hub", exact: true });
    await expect(download).toBeVisible();
    const after = await download.boundingBox();
    expect(before).not.toBeNull(); expect(after).not.toBeNull();
    expect(Math.abs(before!.width - after!.width)).toBeLessThanOrEqual(1);
    expect(await requests(page, "unityHubOpen")).toBe(1);
    expect(await requests(page, "openUnityHubDownload")).toBe(0);
});

test("an unknown Hub open result remains explicit and never retries automatically", async ({ page }) => {
    await open(page, "hub-default=1&hub-open=unknown");
    await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("The result of opening Unity Hub is unknown. Check Unity Hub before trying again.");
    expect(await requests(page, "unityHubOpen")).toBe(1);
    expect(await requests(page, "openUnityHubDownload")).toBe(0);
    await expect(section(page).getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
});

for (const failure of ["failed", "denied"]) {
    test(`known Hub open ${failure} remains an error without an implicit download`, async ({ page }) => {
        await open(page, `hub-default=1&hub-open=${failure}`);
        await section(page).getByRole("button", { name: "Open Unity Hub", exact: true }).click();
        await expect(section(page).getByRole("alert")).toContainText(failure === "failed" ? "Unity Hub could not be opened." : "Opening Unity Hub is not permitted on this connection.");
        expect(await requests(page, "unityHubOpen")).toBe(1);
        expect(await requests(page, "openUnityHubDownload")).toBe(0);
    });
}

test("download adapter failure is retained and never becomes an automatic second attempt", async ({ page }) => {
    await open(page, "hub-download-error=1");
    await section(page).getByRole("button", { name: "Download Unity Hub", exact: true }).click();
    await expect(section(page).getByRole("alert")).toContainText("The Unity Hub download page could not be opened.");
    expect(await requests(page, "unityHubOpen")).toBe(1);
    expect(await requests(page, "openUnityHubDownload")).toBe(1);
    await expect(section(page).getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
});

test("an old download decision cannot open a browser after the Settings visit ends", async ({ page }) => {
    await open(page, "hub-open=hold");
    await section(page).getByRole("button", { name: "Download Unity Hub", exact: true }).click();
    await expect.poll(() => requests(page, "unityHubOpen")).toBe(1);
    const navigation = (name: string) => page.locator("#primary-navigation md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) }) });
    await navigation("Projects").click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await navigation("Settings").click();
    await expect(section(page).getByRole("button", { name: "Download Unity Hub", exact: true })).toBeEnabled();
    await page.evaluate(() => window.dispatchEvent(new Event("test-hub-open-ready")));
    expect(await requests(page, "openUnityHubDownload")).toBe(0);
    expect(await page.evaluate(() => window.settingsRequests.length)).toBe(0);
});

for (const locale of ["zh-CN", "ja-JP"]) {
    test(`Hub path section uses v3 ${locale} labels and fits long paths`, async ({ page }, testInfo) => {
        await open(page, `hub-path=${encodeURIComponent(`C:\\${"Long directory ".repeat(35)}\\Unity Hub.exe`)}`);
        await page.locator("#settings-locale").evaluate((element, next) => {
            (element as HTMLElement & { select(value: string): void }).select(next);
            element.dispatchEvent(new Event("change", { bubbles: true }));
        }, locale);
        await expect(page.locator("html")).toHaveAttribute("lang", locale);
        const localized = page.getByRole("region", { name: locale === "zh-CN" ? "Unity Hub 路径" : "Unity Hubの場所", exact: true });
        await expect(localized.getByRole("button", { name: locale === "zh-CN" ? "选择" : "選択", exact: true })).toBeVisible();
        await expect(localized.getByRole("button", { name: locale === "zh-CN" ? "自动查找" : "自動検出", exact: true })).toBeVisible();
        const automatic = await localized.getByRole("button", { name: locale === "zh-CN" ? "自动查找" : "自動検出", exact: true }).boundingBox();
        const heading = await localized.getByRole("heading", { level: 2 }).boundingBox();
        const field = await localized.locator("md-outlined-text-field").boundingBox();
        const select = await localized.getByRole("button", { name: locale === "zh-CN" ? "选择" : "選択", exact: true }).boundingBox();
        expect(automatic).not.toBeNull(); expect(heading).not.toBeNull(); expect(field).not.toBeNull(); expect(select).not.toBeNull();
        expect(Math.abs((automatic!.y + automatic!.height / 2) - (heading!.y + heading!.height / 2))).toBeLessThanOrEqual(1);
        expect(automatic!.y + automatic!.height).toBeLessThanOrEqual(field!.y);
        const openBox = await localized.getByRole("button", { name: locale === "zh-CN" ? "打开 Unity Hub" : "Unity Hubを開く", exact: true }).boundingBox();
        expect(openBox).not.toBeNull();
        expect(Math.abs((openBox!.x + openBox!.width) - (select!.x + select!.width))).toBeLessThanOrEqual(1);
        const overflow = await localized.evaluate((element) => element.scrollWidth > element.clientWidth);
        expect(overflow).toBe(false);
        await page.screenshot({ path: testInfo.outputPath(`hub-${locale}.png`), animations: "disabled" });
    });
}
