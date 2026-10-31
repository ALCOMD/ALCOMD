import { expect, test, type Page, type Request, type Response } from "@playwright/test";

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
    const pageErrors: string[] = [];
    const failedRequests: Array<{ path: string; error: string }> = [];
    const errorResponses: Array<{ path: string; status: number }> = [];
    const path = (url: string) => new URL(url).pathname.slice(0, 160);
    const onPageError = (error: Error) => { if (pageErrors.length < 8) pageErrors.push(error.message.slice(0, 240)); };
    const onRequestFailed = (request: Request) => {
        if (failedRequests.length < 8) failedRequests.push({ path: path(request.url()), error: (request.failure()?.errorText ?? "unknown").slice(0, 160) });
    };
    const onResponse = (response: Response) => {
        if (response.status() >= 400 && errorResponses.length < 8) errorResponses.push({ path: path(response.url()), status: response.status() });
    };
    page.on("pageerror", onPageError);
    page.on("requestfailed", onRequestFailed);
    page.on("response", onResponse);
    try {
        await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&state=package-multiple&${params}`);
        await expect(page.getByRole("heading", { name: "Manage packages" })).toBeVisible();
    } catch (error) {
        try {
            const document = await page.evaluate(() => ({
                readyState: window.document.readyState,
                rootChildren: window.document.getElementById("root")?.childElementCount ?? null,
                scripts: Array.from(window.document.scripts).slice(0, 8).map((script) => new URL(script.src, location.href).pathname.slice(0, 160)),
                resources: performance.getEntriesByType("resource").slice(0, 12).map((entry) => ({ path: new URL(entry.name, location.href).pathname.slice(0, 160), durationMs: Math.round(entry.duration) }))
            }));
            console.log("M7_CANDIDATE_HARNESS_LOAD " + JSON.stringify({ pageErrors, failedRequests, errorResponses, document }));
        } catch (diagnosticError) {
            console.log("M7_CANDIDATE_HARNESS_LOAD " + JSON.stringify({ pageErrors, failedRequests, errorResponses, diagnosticError: String(diagnosticError).slice(0, 240) }));
        }
        throw error;
    } finally {
        page.off("pageerror", onPageError);
        page.off("requestfailed", onRequestFailed);
        page.off("response", onResponse);
    }
}

for (const entry of ["primary Install", "primary Update", "version menu"]) {
    test(`Package review restores the ${entry} origin after asynchronous planning and Escape`, async ({ page }) => {
        await open(page, "versions=1&delaySource=1&delayPackagePlan=1");
        const method = entry === "primary Update" ? "upgrade" : "install";
        const packageName = entry === "primary Update" ? "Avatar tools" : "com.example.remote";
        const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: `Select ${packageName}`, exact: true }) });
        const trigger = row.getByRole("button", { name: entry === "version menu" ? "Version for com.example.remote: Not installed" : entry === "primary Update" ? "Update" : "Install", exact: true });
        const menu = row.locator("md-menu").first();
        const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
        if (entry === "version menu") {
            const opened = await menu.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
            await trigger.focus();
            await page.keyboard.press("Enter");
            await opened.evaluate(({ event }) => event);
            await opened.dispose();
            await menu.getByRole("menuitem", { name: "1.1.0 · Example packages", exact: true }).click();
        } else {
            await trigger.focus();
            await page.keyboard.press("Enter");
        }
        await expect.poll(() => page.evaluate(() => typeof window.resumePackagePlan)).toBe("function");
        await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateSource)).toBe("function");
        if (entry === "primary Update") await expect(trigger).toHaveCount(0);
        else await expect(trigger).toBeDisabled();
        expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual([method]);
        await page.evaluate(() => window.resumeCandidateSource?.());
        await expect(row).not.toContainText("Evidence unavailable");
        const opened = await page.locator("md-dialog").filter({ hasText: "Packages are up to date" }).evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
        await page.evaluate(() => window.resumePackagePlan?.());
        await opened.evaluate(({ event }) => event);
        await opened.dispose();
        await expect(review).toBeVisible();
        const cancel = review.getByRole("button", { name: "Cancel", exact: true });
        const apply = review.getByRole("button", { name: "Apply changes", exact: true });
        await apply.focus();
        await page.keyboard.press("Tab");
        await expect(cancel).toBeFocused();
        await page.keyboard.press("Shift+Tab");
        await expect(apply).toBeFocused();
        const closed = await review.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true })) }));
        await page.keyboard.press("Escape");
        await closed.evaluate(({ event }) => event);
        await closed.dispose();
        await expect(review).toBeHidden();
        await expect(trigger).toBeFocused();
        if (entry === "version menu") {
            const reopened = await menu.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
            await page.keyboard.press("Enter");
            await reopened.evaluate(({ event }) => event);
            await reopened.dispose();
            await expect(menu.getByRole("menuitem", { name: "1.1.0 · Example packages", exact: true })).toBeVisible();
            expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual([method]);
        } else {
            await page.keyboard.press("Enter");
            await expect.poll(() => page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual([method, method]);
        }
        expect(await page.evaluate(() => window.packageRequests.some((request) => request.method === "apply"))).toBe(false);
    });
}

for (const key of ["Enter", "Space", "Escape"]) {
    test(`Package version menu ${key} preserves one explicit activation`, async ({ page }) => {
        await open(page, "versions=1");
        const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.remote", exact: true }) });
        const trigger = row.getByRole("button", { name: "Version for com.example.remote: Not installed", exact: true });
        const menu = row.locator("md-menu").first();
        const opened = await menu.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
        await trigger.focus();
        await page.keyboard.press("Enter");
        await opened.evaluate(({ event }) => event);
        await opened.dispose();
        const option = menu.getByRole("menuitem", { name: "1.1.0 · Example packages", exact: true });
        await option.focus();
        await expect(option).toBeFocused();
        const closed = await menu.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true })) }));
        await page.keyboard.press(key);
        await closed.evaluate(({ event }) => event);
        await closed.dispose();
        if (key === "Escape") {
            await expect(trigger).toBeFocused();
            expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
        } else {
            await expect(page.getByRole("dialog", { name: "Apply package changes?", exact: true })).toBeVisible();
            expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["install"]);
        }
    });
}

test("Disabled package menu items reject pointer and Material keyboard activation events", async ({ page }) => {
    await open(page, "versions=1");
    const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.remote", exact: true }) });
    const menu = row.locator("md-menu").first();
    const opened = await menu.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
    await row.getByRole("button", { name: "Version for com.example.remote: Not installed", exact: true }).click();
    await opened.evaluate(({ event }) => event);
    await opened.dispose();
    const blocked = menu.locator("md-menu-item").filter({ hasText: "9.0.0 · Example packages" });
    await expect(blocked).toHaveJSProperty("disabled", true);
    await blocked.evaluate((element) => {
        element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        for (const key of ["Enter", "Space"]) element.dispatchEvent(new CustomEvent("close-menu", {
            bubbles: true, composed: true,
            detail: { initiator: element, itemPath: [element], reason: { kind: "keydown", key } }
        }));
    });
    await expect(page.getByRole("dialog", { name: "Apply package changes?", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

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

async function choosePackageStatus(page: Page, label: string) {
    const select = page.locator("md-filled-select").filter({ has: page.getByRole("combobox", { name: "Status", exact: true }) });
    const lifecycle = await select.evaluateHandle((element) => ({
        opened: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })),
        closed: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true }))
    }));
    try {
        await select.getByRole("combobox").click();
        await lifecycle.evaluate((events) => events.opened);
        await page.getByRole("option", { name: label, exact: true }).click();
        await lifecycle.evaluate((events) => events.closed);
    } finally {
        await lifecycle.dispose();
    }
}

for (const metadata of ["paged", "error"]) {
    for (const evidence of ["ambiguous", "unknown", "blocked"]) {
        test(`Available and missing-source filters use Core providers with ${evidence} candidates and ${metadata} metadata`, async ({ page }) => {
            await open(page, `metadata=${metadata}&remoteEvidence=${evidence}`);
            const remote = page.getByRole("checkbox", { name: "Select com.example.remote", exact: true });
            await expect(remote).toBeVisible();
            const row = page.getByRole("row").filter({ has: remote });
            await expect(row.getByRole("button", { name: "Install", exact: true })).toBeDisabled();

            await page.getByRole("button", { name: "Filter packages", exact: true }).click();
            await choosePackageStatus(page, "Available");
            await expect(remote).toBeVisible();
            await choosePackageStatus(page, "Missing source");
            await expect(remote).toHaveCount(0);
            await choosePackageStatus(page, "All packages");
            await expect(remote).toBeVisible();

            expect(await page.evaluate(() => window.repositoryPackageRequests.every((request) => request.cursor === undefined))).toBe(true);
            expect(await page.evaluate(() => window.candidateRequests.some((request) => request.view.kind === "versions"))).toBe(false);
            expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
        });
    }
}

test("Missing source requires affirmative Core evidence rather than unavailable metadata or candidate reads", async ({ page }) => {
    await open(page, "metadata=error&remoteEvidence=missing");
    const missing = page.getByRole("checkbox", { name: "Select com.example.remote", exact: true });
    await expect(missing).toBeVisible();
    await page.getByRole("button", { name: "Filter packages", exact: true }).click();
    await choosePackageStatus(page, "Available");
    await expect(missing).toHaveCount(0);
    await choosePackageStatus(page, "Missing source");
    await expect(missing).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);

    for (const state of ["remoteEvidence=unavailable", "remoteEvidence=missing&candidateError=internal_error"]) {
        await open(page, `metadata=error&${state}`);
        const unknown = page.getByRole("checkbox", { name: "Select com.example.remote", exact: true });
        await expect(unknown).toBeVisible();
        await page.getByRole("button", { name: "Filter packages", exact: true }).click();
        await choosePackageStatus(page, "Missing source");
        await expect(unknown).toHaveCount(0);
        await choosePackageStatus(page, "Available");
        await expect(unknown).toHaveCount(0);
        await choosePackageStatus(page, "All packages");
        await expect(unknown).toBeVisible();
        expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    }
});

test("257 targets and selected rows are disabled without splitting the batch", async ({ page }) => {
    await open(page, "bulk257=1");
    await expect(page.locator("md-menu .os-scrollbar")).toHaveCount(0);
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
    const maximum = await page.evaluate(async (projectId) => {
        let active = 0;
        let maximum = 0;
        const client: Parameters<typeof window.queryCandidates>[0] = {
            packageQueryProjectCandidates: async (request) => {
                active += 1;
                maximum = Math.max(maximum, active);
                await new Promise((resolve) => setTimeout(resolve, 5));
                active -= 1;
                return { projectId: request.projectId, snapshot: { fingerprint: "0".repeat(64), projectRevision: request.expectedRevision, configRevision: 1 }, showPrerelease: false, catalogComplete: true, nextCursor: null, view: "summary", items: [] };
            }
        };
        await Promise.all(Array.from({ length: 7 }, () => window.queryCandidates(client, { projectId, expectedRevision: 1, view: { kind: "summary" } })));
        return maximum;
    }, PROJECT_ID);
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
