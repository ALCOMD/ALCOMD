import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

test("Repeated menu activation closes once and creates one reviewed package intent", async ({ page }) => {
    await open(page, "versions=1");
    const trigger = page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" });
    await trigger.click();
    const option = page.locator("md-menu-item").filter({ hasText: "1.1.0 · Local packages" });
    await expect(option).toBeVisible();
    await option.evaluate((element) => {
        element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    });
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["downgrade"]);
    await page.locator("md-dialog").filter({ hasText: "Apply package changes?" }).getByRole("button", { name: "Cancel", exact: true }).click();
    await trigger.click();
    await page.keyboard.press("Escape");
    await expect(trigger).toBeFocused();
    expect(await page.evaluate(() => window.packageRequests)).toHaveLength(1);
});
async function open(page: Page, params = "") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&state=package-multiple&${params}`);
    await expect(page.getByRole("heading", { name: "Manage packages" })).toBeVisible();
}

test("Core candidate order and relation drive update; all locked IDs ignore search and selection", async ({ page }) => {
    await open(page, "twoInstalled=1&knownExclusion=1");
    await expect(page.getByRole("row").filter({ hasText: "Avatar tools" })).toContainText("1.10.0");
    await page.getByRole("checkbox", { name: "Select Remote tools", exact: true }).check();
    await page.getByRole("searchbox", { name: "Search packages" }).fill("Local tools");
    await page.getByRole("button", { name: "Update All (1)", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    await expect(page.locator("md-dialog").filter({ hasText: "Apply package changes?" })).toContainText("Remote tools: installed newer");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "bulk", params: { intents: [{ kind: "upgrade", packageId: "com.example.avatar", versionRange: "=1.10.0", source: { kind: "repository" } }] } }]);
    expect(await page.evaluate(() => window.candidateRequests.some((request) => request.view.kind === "summary" && request.view.packageIds?.join() === "com.example.avatar,com.example.remote" && request.expectedSnapshot !== undefined))).toBe(true);
});

test("Mixed selected install and upgrade keeps search-hidden selected IDs in one Bulk Plan", async ({ page }) => {
    await open(page);
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await page.getByRole("checkbox", { name: "Select Remote tools", exact: true }).check();
    await page.getByRole("searchbox", { name: "Search packages" }).fill("Local tools");
    await page.getByRole("button", { name: "Install / Update selected", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "bulk", params: { intents: [{ kind: "upgrade", packageId: "com.example.avatar" }, { kind: "install", packageId: "com.example.remote" }] } }]);
    expect(await page.evaluate(() => window.packageRequests.some((item) => item.method === "apply"))).toBe(false);
});

test("Unknown or ambiguous evidence blocks updates without probing Plans; explicit source resolves ambiguity", async ({ page }) => {
    await open(page, "unknown=1");
    await expect(page.getByRole("button", { name: "Update All (0)", exact: true })).toBeDisabled();
    await expect(page.getByText("Resolve unknown or ambiguous package evidence before Update All.")).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await open(page, "ambiguous=1");
    await page.getByRole("button", { name: "Source for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Example packages", exact: true }).click();
    await expect(page.getByRole("button", { name: "Update All (1)", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Incomplete catalog keeps versions readable and disables Plan actions including bulk remove", async ({ page }) => {
    await open(page, "catalogIncomplete=1");
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await expect(page.getByRole("button", { name: "Remove selected", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" }).click();
    const option = page.locator("md-menu-item").filter({ hasText: "1.10.0 · Example packages" });
    await expect(option).toHaveJSProperty("disabled", true);
    await option.evaluate((element) => element.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    await page.keyboard.press("Enter");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Transitive selection cannot be silently removed but can be reinstalled", async ({ page }) => {
    await open(page, "transitive=1");
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await expect(page.getByRole("button", { name: "Remove selected", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Reinstall selected", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Stale versions and stale pre-Plan evidence discard eligibility without automatic mutation retry", async ({ page }) => {
    await open(page, "versionsStale=1");
    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" }).click();
    await expect(page.getByText("Candidate evidence is stale.")).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await open(page, "intentStale=1");
    await page.getByRole("button", { name: "Update All (1)", exact: true }).click();
    await expect(page.getByText("Candidate evidence is stale.")).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.limit === 1).length)).toBe(1);
});

test("Candidate quota failure is unavailable evidence, never empty/latest or a Plan probe", async ({ page }) => {
    await open(page, "candidateError=package_candidate_limit_exceeded");
    await expect(page.getByText(/package_candidate_limit_exceeded/)).toBeVisible();
    await expect(page.getByRole("button", { name: "Reload package evidence", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Summary pages share the first fingerprint while versions are read only after opening a row", async ({ page }) => {
    await open(page, "paged=1");
    const requests = await page.evaluate(() => window.candidateRequests);
    expect(requests.filter((request) => request.view.kind === "versions")).toHaveLength(0);
    expect(requests.filter((request) => request.cursor).every((request) => request.expectedSnapshot === request.cursor?.snapshotFingerprint)).toBe(true);
    await page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" }).click();
    await expect(page.getByRole("menuitem", { name: "Load more versions", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "versions").length)).toBe(1);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Candidate source actions never drop the source when plan.v2 is missing", async ({ page }) => {
    await open(page, "noPlanV2=1");
    await expect(page.getByRole("button", { name: "Update", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Update All (1)", exact: true })).toBeDisabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Source filter includes nonwinning and ambiguous providers beyond display metadata", async ({ page }) => {
    await open(page, "ambiguous=1");
    await page.getByRole("button", { name: "Filter packages" }).click();
    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Local repository", exact: true }).click();
    await expect(page.getByRole("checkbox", { name: "Select Avatar tools", exact: true })).toBeVisible();
    await expect(page.getByRole("checkbox", { name: "Select Remote tools", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("257 targets and selected rows are disabled without splitting the batch", async ({ page }) => {
    await open(page, "bulk257=1");
    await expect(page.getByRole("button", { name: "Update All (257)", exact: true })).toBeDisabled();
    await page.getByRole("checkbox", { name: "Select all visible packages", exact: true }).check();
    await expect(page.getByRole("button", { name: "Install / Update selected", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Reinstall selected", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Remove selected", exact: true })).toBeDisabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("One invalid selected row blocks the whole mixed action, including a search-hidden row", async ({ page }) => {
    await open(page, "mixedInvalid=1");
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await page.getByRole("checkbox", { name: "Select Remote tools", exact: true }).check();
    await page.getByRole("searchbox", { name: "Search packages" }).fill("Avatar tools");
    await expect(page.getByRole("button", { name: "Install / Update selected", exact: true })).toBeDisabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("No-update and empty selection create no Plan", async ({ page }) => {
    await open(page, "noUpdate=1");
    await expect(page.getByRole("button", { name: "Update All (0)", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Update", exact: true })).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Selected package actions" })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Stable bulk actions use the distinct Core stable target and one explicit Apply", async ({ page }) => {
    await open(page, "stableAlternate=1");
    await page.getByRole("button", { name: "Update All Stable (1)", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "bulk", params: { intents: [{ kind: "upgrade", packageId: "com.example.avatar", versionRange: "=1.10.0", includePrerelease: false }] } }]);
    await review.getByRole("button", { name: "Apply changes", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Package changes", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests.map((item) => item.method))).toEqual(["bulk", "apply"]);
});

test("Candidate reads share a two-request bound across independent menu/traversal callers", async ({ page }) => {
    await open(page);
    const maximum = await page.evaluate(async () => {
        const modulePath = "/src/package-candidates.ts";
        const module = await import(modulePath);
        let active = 0;
        let maximum = 0;
        const client = { packageQueryProjectCandidates: async () => { active += 1; maximum = Math.max(maximum, active); await new Promise((resolve) => setTimeout(resolve, 5)); active -= 1; return {}; } };
        await Promise.all(Array.from({ length: 7 }, () => module.queryCandidates(client, {})));
        return maximum;
    });
    expect(maximum).toBe(2);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Changing source immediately invalidates the old target until the matching source query completes", async ({ page }) => {
    await open(page, "versions=1&delaySource=1");
    await expect(page.getByRole("button", { name: "Update", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Source for Avatar tools" }).click();
    await page.getByRole("menuitem", { name: "Local packages", exact: true }).click();
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateSource)).toBe("function");
    await expect(page.getByRole("button", { name: "Update", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Version for Avatar tools: 1.2.3" })).toBeDisabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await page.evaluate(() => window.resumeCandidateSource?.());
    await page.getByRole("button", { name: "Update", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "upgrade", params: { source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000112" } } }]);
});
