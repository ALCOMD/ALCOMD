import { expect, test } from "@playwright/test";
import { selectActionMenuItem } from "./material-menu-actions";
import { expectRequestError } from "./request-error-assertions";
test.use({ viewport: { width: 1280, height: 800 } });

for (const state of ["ready", "empty"]) {
    test(`loaded ${state} Unity installations survive a failed reconnect read without a page notice`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/settings&state=${state}`);
        await page.getByRole("navigation", { name: "Settings sections", exact: true }).getByRole("button", { name: "Unity", exact: true }).click();
        const region = page.getByRole("region", { name: "Unity installations", exact: true });
        if (state === "empty") await expect(region.getByText("No Unity installations registered", { exact: true })).toBeVisible();
        else await expect(region.getByRole("table").locator("tbody tr")).toHaveCount(1);
        const content = await region.innerText();
        const bounds = await region.boundingBox();
        const reads = await page.evaluate(() => window.coreEvents.rpcRequests.filter(({ method }) => method === "unityInstallationsList").length);
        await page.evaluate(() => { window.dispatchEvent(new Event("test-unity-read-error")); window.coreEvents.restart(); });
        await expect.poll(() => page.evaluate(() => window.coreEvents.rpcRequests.filter(({ method }) => method === "unityInstallationsList").length)).toBeGreaterThan(reads);
        await expect(region).toHaveText(content, { useInnerText: true });
        expect(await region.boundingBox()).toEqual(bounds);
        await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
        await expect(page.getByRole("dialog")).toHaveCount(0);
        const failedReads = await page.evaluate(() => window.coreEvents.rpcRequests.filter(({ method }) => method === "unityInstallationsList").length);
        await page.evaluate(() => window.coreEvents.restart());
        await expect.poll(() => page.evaluate(() => window.coreEvents.rpcRequests.filter(({ method }) => method === "unityInstallationsList").length)).toBeGreaterThan(failedReads);
        await expect(region).toHaveText(content, { useInnerText: true });
    });
}

test("an empty loaded log view survives a failed reconnect read without a page notice", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/logs&state=ready");
    const main = page.getByRole("main");
    const list = page.locator(".operation-log-list");
    await expect(main.getByText("No logs yet", { exact: true })).toBeVisible();
    const content = await list.innerText();
    const reads = await page.evaluate(() => window.fixtureLogs.reads.length);
    await page.evaluate(() => { window.fixtureLogs.fail = true; window.coreEvents.restart(); });
    await expect.poll(() => page.evaluate(() => window.fixtureLogs.reads.length)).toBeGreaterThan(reads);
    await expect(list).toHaveAttribute("aria-busy", "false");
    await expect(list).toHaveText(content, { useInnerText: true });
    await expect(main.getByRole("alert")).toHaveCount(0);
    await expect(page.getByRole("dialog")).toHaveCount(0);
    const failedReads = await page.evaluate(() => window.fixtureLogs.reads.length);
    await page.evaluate(() => { window.fixtureLogs.fail = false; window.coreEvents.restart(); });
    await expect.poll(() => page.evaluate(() => window.fixtureLogs.reads.length)).toBeGreaterThan(failedReads);
    await expect(list).toHaveText(content, { useInnerText: true });
    await expect(main.getByRole("alert")).toHaveCount(0);
});

for (const state of ["ready", "empty"]) {
    test(`loaded ${state} project view survives a failed read without a page notice`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects&state=${state}`);
        const content = page.locator(".projects-content");
        if (state === "ready") await expect(page.getByRole("button", { name: "Manage", exact: true })).toBeVisible();
        else await expect(content.getByRole("heading", { name: "No registered projects", exact: true })).toBeVisible();
        const text = await content.innerText();
        const bounds = await content.boundingBox();
        const reads = await page.evaluate(() => window.projectListReads);
        await page.evaluate(() => { window.dispatchEvent(new Event("test-project-read-error")); window.coreEvents.restart(); });
        await expect.poll(() => page.evaluate(() => window.projectListReads)).toBeGreaterThan(reads);
        await expect(content).toHaveText(text, { useInnerText: true });
        expect(await content.boundingBox()).toEqual(bounds);
        await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
        await expect(page.getByRole("dialog")).toHaveCount(0);
        const failedReads = await page.evaluate(() => window.projectListReads);
        await page.evaluate(() => window.coreEvents.restart());
        await expect.poll(() => page.evaluate(() => window.projectListReads)).toBeGreaterThan(failedReads);
        await expect(content).toHaveText(text, { useInnerText: true });
        await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    });
}

for (const mode of ["custom", "direct"] as const) {
    test(`proxy ${mode} save failure adds no page notice and can be retried`, async ({ page }) => {
        await page.goto("/browser-harness.html?route=/settings&proxy-save-error-once=1");
        const region = page.getByRole("region", { name: "Network", exact: true });
        await region.locator("#settings-network-proxy").evaluate((element, value) => {
            (element as HTMLElement & { select(value: string): void }).select(value);
            element.dispatchEvent(new Event("change", { bubbles: true }));
        }, mode);
        if (mode === "custom") {
            await region.getByRole("textbox", { name: "Proxy address", exact: true }).fill("http://127.0.0.1:7890");
            await region.getByRole("button", { name: "Save", exact: true }).click();
        }
        await expect.poll(() => page.evaluate(() => window.settingsRequests.filter(({ update }) => update.network !== undefined).length)).toBe(1);
        const retry = region.getByRole("button", { name: mode === "custom" ? "Save" : "Retry", exact: true });
        await expect(retry).toBeEnabled();
        await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
        await expect(region).not.toContainText("permission_denied");
        await expect(region).not.toContainText("You do not have permission");
        await expect(page.getByRole("dialog")).toHaveCount(0);
        await retry.click();
        await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
        expect(await page.evaluate(async () => (await window.fixtureCore.settingsGet()).settings.network.proxy.mode)).toBe(mode);
        await expect(region.locator("#settings-network-proxy")).toHaveJSProperty("value", mode);
        await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    });
}

for (const failure of [false, true]) {
    test(`VCC registration ${failure ? "failure" : "success"} adds no page notice`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/settings&vcc-settings-fixture=1${failure ? "&vcc-registration-error=1" : ""}`);
        const region = page.getByRole("region", { name: "VCC links", exact: true });
        const register = region.getByRole("button", { name: "Register ALCOMD for VCC links", exact: true });
        await expect(register).toBeEnabled();
        const content = await region.innerText();
        const bounds = await region.boundingBox();
        for (let attempt = 1; attempt <= 2; attempt++) {
            await register.click();
            await expect.poll(() => page.evaluate(() => window.resourceRequests.filter(({ method }) => method === "gui_register_vcc_links").length)).toBe(attempt);
            await expect(register).toBeEnabled();
            await expect(region).toHaveText(content, { useInnerText: true });
            expect(await region.boundingBox()).toEqual(bounds);
            await expect(region.locator('[role="alert"], [role="status"]')).toHaveCount(0);
            await expect(page.getByRole("dialog")).toHaveCount(0);
        }
    });
}

for (const state of ["package-filter-conflict", "package-filter-denied", "settings-read-error"]) {
    test(`repository ${state} adds no error notice and retains filter reload`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/repositories&${state === "settings-read-error" ? "settings-read-error=1" : `state=${state}`}`);
        const main = page.getByRole("main");
        const table = main.getByRole("table", { name: "Repositories", exact: true });
        await expect(table).toBeVisible();
        if (state !== "settings-read-error") await table.getByRole("checkbox").first().click();
        const reload = main.getByRole("button", { name: "Reload filters", exact: true });
        await expect(reload).toBeVisible();
        await expect(main.getByRole("alert")).toHaveCount(0);
        for (const code of ["permission_denied", "revision_conflict", "internal_error"]) await expect(main).not.toContainText(code);
        await expect(page.getByRole("dialog")).toHaveCount(0);
        const reads = await page.evaluate(() => window.settingsReads);
        await reload.click();
        await expect.poll(() => page.evaluate(() => window.settingsReads)).toBeGreaterThan(reads);
        await expect(table).toBeVisible();
        await expect(main.getByRole("alert")).toHaveCount(0);
    });
}

test("favorite failure leaves the row intact and the original action retries it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects&state=favorite-conflict");
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("button", { name: "Remove from favorites", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
});
test("package link failure adds no prompt to its row or the next page", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101&state=package-multiple&package-link-error=1");
    const row = page.getByRole("table", { name: "Packages", exact: true }).getByRole("row").filter({ hasText: "Avatar tools" });
    await selectActionMenuItem(row.getByRole("button", { name: "More actions for Avatar tools", exact: true }), page.getByRole("menuitem", { name: "Documentation", exact: true }));
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
});
test("user package refresh retries through the same control and reloads real revision", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("revision 2", { exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.resourceRequests.filter((r) => r.method === "packages.userPackages.refresh"))).toHaveLength(2);
});

test("a repository removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/repositories&state=package-partial-failure&resource-remove-error=1");
    const row = page.getByRole("table", { name: "Repositories", exact: true }).getByRole("row").filter({ hasText: "Local packages" });
    await row.getByRole("button", { name: "Browse packages", exact: true }).click();
    const management = page.locator("md-dialog[open]");
    await management.getByRole("button", { name: "Refresh repository", exact: true }).click();
    await management.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await row.getByRole("button", { name: "Remove", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove repository", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await expect(dialog).toContainText("Packages already installed in projects stay installed.");
});

test("a user package removal error cannot reuse an earlier refresh failure notification", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/user-packages&state=package-user-source&user-refresh-error=1&resource-remove-error=1");
    const row = page.getByRole("table", { name: "User Packages", exact: true }).getByRole("row").filter({ hasText: "Local avatar tools" });
    await row.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("main").getByRole("alert")).toHaveCount(0);
    await row.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await dialog.getByRole("button", { name: "Remove enrollment", exact: true }).click();
    await expectRequestError(dialog.getByRole("alert"), "permission_denied");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await expect(dialog).toContainText("The source folder will stay on disk.");
});

for (const state of ["failed", "cancelled"]) {
    test(`an immediately ${state} project deletion keeps its real result in the task window`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101&state=${state}`);
        await selectActionMenuItem(page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }), page.getByRole("menuitem", { name: "Delete Project Directory…", exact: true }));
        const review = page.locator("md-dialog[open]");
        await review.getByRole("textbox", { name: "Confirm project directory name", exact: true }).fill("Sample");
        await expect(review).toContainText("does not use the Recycle Bin or Trash");
        await review.getByRole("button", { name: "Permanently delete", exact: true }).click();
        await expect(review.getByRole("status")).toContainText(state);
        if (state === "failed") await expectRequestError(review.getByRole("alert"), "package_archive_invalid");
        else await expect(review.getByRole("status")).toContainText("cancelled");
        await expect(page.locator(".reminder-floating .reminder--success")).toHaveCount(0);
        await expect(review.getByRole("button", { name: "Close", exact: true })).toBeVisible();
    });
}

test("a completed project deletion closes its review and reloads the project list", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/projects/00000000-0000-4000-8000-000000000101");
    await selectActionMenuItem(page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: /More actions for/ }), page.getByRole("menuitem", { name: "Delete Project Directory…", exact: true }));
    const review = page.locator("md-dialog[open]");
    await review.getByRole("textbox", { name: "Confirm project directory name", exact: true }).fill("Sample");
    await review.getByRole("button", { name: "Permanently delete", exact: true }).click();
    await expect(page.getByText("Task completed.", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Projects", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "No registered projects", exact: true })).toBeVisible();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await expect(page.locator(".reminder-floating .reminder--error")).toHaveCount(0);
});
