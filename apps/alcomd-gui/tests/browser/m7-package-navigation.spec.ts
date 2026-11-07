import { expect, test, type Page } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";

const projectId = "00000000-0000-4000-8000-000000000101";

async function prepareTimedNavigation(page: Page, hold = "holdPackageCandidates") {
    await page.clock.install();
    await page.goto(`/browser-harness.html?route=%2Fprojects&state=ready&${hold}=1`);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await page.getByRole("button", { name: "Manage", exact: true }).evaluate((button: HTMLElement) => button.click());
    await expect.poll(() => page.evaluate(() => typeof (window.resumePackageCandidates ?? window.resumePackageProject))).toBe("function");
}

test("Manage keeps Projects for 99ms and reveals only the pending list at the 100ms deadline", async ({ page }) => {
    await prepareTimedNavigation(page);
    await page.clock.runFor(99);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "true");
    await expect(page.locator(".project-workspace-header")).toHaveCount(0);
    await page.clock.runFor(1);
    await expect(page.locator(".project-workspace-header")).toBeVisible();
    await expect(page.getByRole("searchbox", { name: "Search packages", exact: true })).toBeEnabled();
    await expect(page.locator(".package-workspace-list-region")).toHaveAttribute("aria-busy", "true");
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "summary" && !request.cursor && !request.view.filter && request.view.bulkPackageIds === undefined).length)).toBe(1);
    await page.clock.resume();
});

test("A first list ready within 100ms appears with the page without a list loading frame", async ({ page }) => {
    await prepareTimedNavigation(page);
    await page.clock.runFor(50);
    await page.evaluate(() => {
        window.capabilityLoadingScreens = [];
        new MutationObserver(() => {
            if (document.querySelector(".project-workspace .loading-state")) window.capabilityLoadingScreens.push("package-list");
        }).observe(document.querySelector(".route-content")!, { childList: true, subtree: true });
        window.resumePackageCandidates?.();
    });
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    await expect(page.locator(".package-workspace-list-region")).toHaveAttribute("aria-busy", "false");
    expect(await page.evaluate(() => window.capabilityLoadingScreens)).toEqual([]);
    await page.clock.runFor(100);
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    await page.clock.resume();
});

test("Slow project metadata cannot keep the old page indefinitely past 100ms", async ({ page }) => {
    await prepareTimedNavigation(page, "holdPackageProject");
    await page.clock.runFor(99);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.clock.runFor(1);
    await expect(page.locator(".route-content > .page-loading-state")).toBeAttached();
    await expect(page.locator(".route-content > .page-loading-state md-linear-progress")).toBeVisible();
    await expect(page.locator(".projects-page")).toBeHidden();
    await page.evaluate(() => window.resumePackageProject?.());
    await page.clock.resume();
    await expect(page.locator(".project-workspace-header")).toBeVisible();
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
});

test("A candidate error within the short wait reveals the header with Retry instead of an empty loading page", async ({ page }) => {
    await prepareTimedNavigation(page);
    await page.clock.runFor(50);
    await page.evaluate(() => window.resumePackageCandidates?.("package_candidate_limit_exceeded"));
    await expect(page.locator(".project-workspace-header")).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeEnabled();
    await expect(page.locator(".package-workspace-list-region")).toHaveAttribute("aria-busy", "false");
    await expect(page.locator(".loading-state")).toHaveCount(0);
    await page.clock.resume();
});

test("Leaving during the short wait cancels its deadline and ignores the late candidate result", async ({ page }) => {
    await prepareTimedNavigation(page);
    await page.clock.runFor(50);
    await page.locator("#primary-navigation .navigation-item").filter({ hasText: "Resources" }).evaluate((button: HTMLElement) => button.click());
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
    await page.clock.runFor(500);
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
    await expect(page.locator(".project-workspace")).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await page.clock.resume();
});

async function openProjects(page: Page) {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready&holdPackageCandidates=1");
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Manage", exact: true }).click();
    await expect(page.getByRole("heading", { level: 1, name: "<private-project>", exact: true })).toBeFocused();
    await expect.poll(() => page.evaluate(() => typeof window.resumePackageCandidates)).toBe("function");
}

test("Only the package list waits: project context, Unity actions and toolbar appear before candidates", async ({ page }) => {
    await openProjects(page);
    await expect(page.locator(".project-workspace-header .page-header-subtitle")).toContainText("<private-project>");
    await expect(page.getByRole("button", { name: "Back", exact: true })).toBeEnabled();
    await expect(page.getByRole("combobox", { name: "Unity version", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open Unity", exact: true })).toBeEnabled();
    await expect(page.getByRole("searchbox", { name: "Search packages", exact: true })).toBeEnabled();
    await expect(page.getByRole("button", { name: "Filter packages", exact: true })).toBeEnabled();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    await expect(page.locator(".route-content")).not.toHaveAttribute("inert", "");
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toHaveCount(0);
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toHaveCount(0);
    await expect(page.getByText("No packages", { exact: true })).toHaveCount(0);
    await expect(page.locator(".package-workspace-list-region")).toHaveAttribute("aria-busy", "true");
    await expect(page.locator(".package-workspace-list-region .loading-state-indicator")).toBeVisible();
    const title = await page.locator("#route-title").elementHandle();
    await page.screenshot({ path: test.info().outputPath("only-package-list-loading.png"), animations: "disabled" });
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    expect(await title!.evaluate((element) => element.isConnected)).toBe(true);
    await expect(page.locator("#route-title")).toBeFocused();
    await expect(page.locator(".loading-state")).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    expect(await page.evaluate(() => window.unityRequests.filter((request) => request.method === "unity.launch"))).toEqual([]);
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([]);
});

test("Search remains focused and its input is applied when the first package list arrives", async ({ page }) => {
    await openProjects(page);
    const search = page.getByRole("searchbox", { name: "Search packages", exact: true });
    await search.fill("Remote");
    await expect(search).toBeFocused();
    await page.evaluate(() => window.resumePackageCandidates?.());
    const rows = page.getByRole("table", { name: "Packages", exact: true }).locator("tbody tr");
    await expect(rows).toHaveCount(1);
    await expect(rows).toContainText(["Remote tools"]);
    await expect(search).toHaveValue("Remote");
    await expect(search).toBeFocused();
});

test("Back leaves a pending candidate read and its late response cannot replace Projects", async ({ page }) => {
    await openProjects(page);
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.evaluate(() => window.resumePackageCandidates?.());
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeFocused();
    await expect(page.getByRole("heading", { name: "Manage packages", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Candidate failure keeps the project header and toolbar available and retry recovers the list", async ({ page }) => {
    await openProjects(page);
    await page.evaluate(() => window.resumePackageCandidates?.("package_candidate_limit_exceeded"));
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeEnabled();
    await expect(page.locator(".package-refresh-action")).toHaveAttribute("title", requestErrorMessage("package_candidate_limit_exceeded"));
    await expect(page.getByRole("heading", { level: 1, name: "<private-project>", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Open Unity", exact: true })).toBeEnabled();
    await expect(page.getByRole("searchbox", { name: "Search packages", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect(page.getByRole("table", { name: "Packages", exact: true }).getByText("Avatar tools", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("The direct packages route preserves header controls and keyboard focus during candidate loading", async ({ page }) => {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${projectId}/packages`)}&holdPackageCandidates=1`);
    await expect(page.getByRole("heading", { level: 1, name: "<private-project>", exact: true })).toBeFocused();
    await expect(page.locator(".package-workspace-list-region .loading-state-indicator")).toBeVisible();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Back", exact: true })).toBeFocused();
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Back", exact: true })).toBeFocused();
});

test("An invalidation during the candidate read discards its stale response and re-reads before enabling package actions", async ({ page }) => {
    await openProjects(page);
    const before = await page.evaluate(() => ({ requests: window.candidateRequests.length, acknowledgements: window.coreEvents.requests.filter((request) => request.method === "ack").length }));
    await page.evaluate(() => window.coreEvents.emit("repository", "00000000-0000-4000-8000-000000000201", "repository.refreshed", 2));
    await expect.poll(() => page.evaluate(() => window.coreEvents.requests.filter((request) => request.method === "ack").length)).toBeGreaterThan(before.acknowledgements);
    await expect(page.getByRole("button", { name: /^Update All/ })).toHaveCount(0);
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.candidateRequests.length)).toBeGreaterThan(before.requests);
    await expect(page.getByRole("button", { name: "Update All (1)", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});
