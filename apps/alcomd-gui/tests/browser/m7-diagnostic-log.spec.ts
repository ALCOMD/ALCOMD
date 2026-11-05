import { expect, test, type Page } from "@playwright/test";
import type { TestLogInput } from "./diagnostic-log-client";
test.use({ viewport: { width: 1280, height: 800 } });
const url = "/browser-harness.html?route=/diagnostics&state=empty&diagnostic-log=1&motion=reduced";
const table = (page: Page) => page.getByRole("table", { name: "Technical Logs", exact: true });
const append = (page: Page, detail: TestLogInput) => page.evaluate((detail) => window.dispatchEvent(new CustomEvent("test-diagnostic-log", { detail })), detail);

test("real source JSON retains technical path and error details without any popup", async ({ page }) => {
    await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    const path = "C:\\项目A\\Packages\\vpm-manifest.json";
    await append(page, { message: "Cannot parse the selected manifest", path, diagnosticId: "00000000-0000-4000-8000-000000000921" });
    await expect(table(page)).toContainText("Cannot parse the selected manifest");
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await expect(table(page)).toContainText(JSON.stringify(path));
    await expect(table(page)).toContainText("00000000-0000-4000-8000-000000000921");
});

test("health-only changes are visible without adding a fabricated log record", async ({ page }) => {
    await page.goto(url); await expect(table(page)).toContainText("Core logging initialized.");
    const before = await table(page).locator("tbody tr").count();
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-log-health", { detail: { source: "core", state: "degraded", queueDropped: 7, writeFailed: 2, hasGap: true } })));
    await expect(page.getByRole("alert")).toContainText("Log storage is degraded");
    await expect(page.locator('[data-log-source="core"]')).toContainText("7");
    await expect(table(page).locator("tbody tr")).toHaveCount(before);
});

test("Core offline does not block GUI logging or real frontend exception capture", async ({ page }) => {
    await page.goto(url + "&log-initial-failure=1");
    await expect(page.getByRole("alert")).toContainText("Core logs could not be read.");
    await append(page, { source: "gui", message: "Native window operation failed" });
    await expect(table(page)).toContainText("Native window operation failed");
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await page.evaluate(() => {
        const error = new Error("Actual renderer exception");
        window.dispatchEvent(new ErrorEvent("error", { error }));
        window.dispatchEvent(new ErrorEvent("error", { error }));
    });
    await expect(table(page)).toContainText("Actual renderer exception");
    expect(await page.evaluate(() => window.frontendErrorReports)).toHaveLength(1);
    await expect(page.locator(".reminder-floating")).toHaveCount(0);
});

test("healthy idle and ordinary failed reads do not establish periodic reads", async ({ page }) => {
    await page.clock.install(); await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    const before = await page.evaluate(() => [window.diagnosticLogRequests.length, window.localLogRequests.length]);
    await page.clock.fastForward(120_000);
    expect(await page.evaluate(() => [window.diagnosticLogRequests.length, window.localLogRequests.length])).toEqual(before);
    await page.evaluate(() => window.dispatchEvent(new Event("test-diagnostic-log-failure")));
    await expect(page.getByRole("alert")).toContainText("Core logs could not be read.");
    const failed = await page.evaluate(() => window.diagnosticLogRequests.length);
    await page.clock.fastForward(120_000);
    expect(await page.evaluate(() => window.diagnosticLogRequests.length)).toBe(failed);
});

test("delayed readable updates are not displayed until the source publishes", async ({ page }) => {
    await page.goto(url); await expect(table(page)).toContainText("Core logging initialized.");
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-log-hold", { detail: { held: true } })));
    await append(page, { message: "Delayed complete record" });
    await expect(table(page)).not.toContainText("Delayed complete record");
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-log-hold", { detail: { held: false } })));
    await expect(table(page)).toContainText("Delayed complete record");
});

test("leaving and remounting cancels each source without leaking duplicate readers", async ({ page }) => {
    await page.goto(url); await expect(table(page)).toContainText("Core logging initialized.");
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Projects$/ }) }).click();
    await expect.poll(() => page.evaluate(() => window.diagnosticLogCancels)).toBeGreaterThan(0);
    await expect.poll(() => page.evaluate(() => window.localLogCancels)).toBeGreaterThan(0);
    await page.getByRole("navigation", { name: "Primary", exact: true }).locator("md-list-item").filter({ has: page.locator(".navigation-item-label", { hasText: /^Logs$/ }) }).click();
    await page.getByRole("navigation", { name: "Logs", exact: true }).getByRole("button", { name: "Technical Logs", exact: true }).click();
    await expect(table(page)).toContainText("Core logging initialized.");
    await expect(table(page).locator("tbody tr")).toHaveCount(1);
});

test("a missing diagnostic association is reported without synthesizing a summary", async ({ page }) => {
    await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    await page.getByRole("searchbox", { name: "Search technical logs", exact: true }).fill("00000000-0000-4000-8000-000000009999");
    await expect(page.getByRole("status").filter({ hasText: "No associated record was found" })).toBeVisible();
    await expect(table(page).locator("[data-log-id]")).toHaveCount(0);
});

test("ordinary Core failures are not submitted as frontend exceptions", async ({ page }) => {
    await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    await page.evaluate(() => window.dispatchEvent(new PromiseRejectionEvent("unhandledrejection", { promise: Promise.resolve(), reason: Object.assign(new Error("Already recorded by Core"), { code: "invalid_params" }) })));
    await expect(table(page)).not.toContainText("Already recorded by Core");
    expect(await page.evaluate(() => window.frontendErrorReports)).toHaveLength(0);
});

test("a real retry can report the same Error object in a new propagation turn", async ({ page }) => {
    await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await page.evaluate(async () => {
        const error = new Error("Renderer failure retried");
        window.dispatchEvent(new ErrorEvent("error", { error }));
        window.dispatchEvent(new ErrorEvent("error", { error }));
        await new Promise((resolve) => setTimeout(resolve, 0));
        window.dispatchEvent(new ErrorEvent("error", { error }));
    });
    await expect.poll(() => page.evaluate(() => window.frontendErrorReports.length)).toBe(2);
    await expect(table(page).locator("tbody tr").filter({ hasText: "Renderer failure retried" })).toHaveCount(2);
});

test("long Chinese frontend exceptions fit native UTF-8 limits without splitting Unicode scalars", async ({ page }) => {
    await page.goto(url);
    await expect(table(page)).toContainText("Core logging initialized.");
    const oversizedRejected = await page.evaluate(async () => {
        try { await window.fixtureCore.reportFrontendError({ name: "Error", message: "中".repeat(1366) }); return false; }
        catch (failure: unknown) { return typeof failure === "object" && failure !== null && "code" in failure && failure.code === "invalid_params"; }
    });
    expect(oversizedRejected).toBe(true);
    await page.evaluate(() => {
        const error = new Error("中".repeat(1365) + "😀" + "尾".repeat(1000));
        error.name = "名".repeat(42) + "😀尾";
        error.stack = "栈".repeat(5461) + "😀" + "尾".repeat(2000);
        window.history.replaceState(null, "", "/" + "位".repeat(1000));
        window.dispatchEvent(new ErrorEvent("error", { error }));
    });
    await expect.poll(() => page.evaluate(() => window.frontendErrorReports.length)).toBe(1);
    const report = await page.evaluate(() => window.frontendErrorReports[0]!);
    expect(report.name).toBe("名".repeat(42));
    expect(report.message).toBe("中".repeat(1365));
    expect(report.stack).toBe("栈".repeat(5461));
    for (const [key, limit] of [["name", 128], ["message", 4096], ["stack", 16384], ["location", 2048]] as const) {
        expect(new TextEncoder().encode(report[key] ?? "").byteLength).toBeLessThanOrEqual(limit);
        expect(report[key]).not.toContain("\ufffd");
    }
    await page.getByRole("checkbox", { name: "Show details", exact: true }).check();
    await expect(table(page)).toContainText("unhandled frontend exception");
    await expect(table(page)).toContainText(report.message);
    await expect(table(page)).toContainText(report.stack!);
});

test("a long Unicode path is queried intact and oversize UTF-8 input remains an explicit field error", async ({ page }) => {
    await page.clock.install();
    await page.goto(url);
    const path = "C:\\Fixture\\" + "长路径".repeat(200) + "\\manifest.json";
    await expect(table(page)).toContainText("Core logging initialized.");
    await append(page, { message: "Long path failure", path });
    await expect(table(page)).toContainText("Long path failure");
    const input = page.getByRole("searchbox", { name: "Search technical logs", exact: true });
    await input.fill("  " + path + "  ");
    await page.clock.runFor(500);
    await expect.poll(() => page.evaluate(() => window.diagnosticLogRequests.at(-1)?.query?.search)).toBe(path);
    await expect(table(page)).toContainText("Long path failure");
    await expect(table(page)).not.toContainText("Core logging initialized.");
    const before = await page.evaluate(() => [window.diagnosticLogRequests.length, window.localLogRequests.length]);
    await input.fill("界".repeat(1366));
    await expect(page.locator("md-filled-text-field.logs-search")).toHaveJSProperty("error", true);
    await expect(page.locator("md-filled-text-field.logs-search").getByRole("alert")).toContainText("Search text is too long. Shorten it to apply the filter.");
    await page.clock.runFor(1000);
    expect(await page.evaluate(() => [window.diagnosticLogRequests.length, window.localLogRequests.length])).toEqual(before);
    await expect(table(page)).toContainText("Long path failure");
});
