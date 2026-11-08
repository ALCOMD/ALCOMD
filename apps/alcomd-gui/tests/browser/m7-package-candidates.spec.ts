import { expect, test, type Locator, type Page, type Request, type Response } from "@playwright/test";
import { requestErrorMessage } from "../../src/request-errors";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

test("Repeated version selection closes once and creates one reviewed package intent", async ({ page }) => {
    await open(page, "versions=1");
    await chooseVersionSource(page, page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }) }), "Local packages");
    const trigger = page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" });
    await trigger.click();
    const option = page.locator("md-select-option").filter({ hasText: "1.1.0" });
    await expect(option).toBeVisible();
    await expect(option).toHaveJSProperty("disabled", false);
    await option.getByRole("option").evaluate((element) => {
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

async function packageMenuLifecycle(row: Locator, selector: string) {
    return row.evaluateHandle((element, selector) => {
        const once = (type: string) => new Promise<void>((resolve) => {
            const listener = (event: Event) => {
                if (!(event.target instanceof Element) || !event.target.matches(selector)) return;
                element.removeEventListener(type, listener, true);
                resolve();
            };
            element.addEventListener(type, listener, true);
        });
        return { opened: once("opened"), closed: once("closed") };
    }, selector);
}

for (const entry of ["primary Install", "primary Update", "version menu"]) {
    test(`Package review restores the ${entry} origin after asynchronous planning and Escape`, async ({ page }) => {
        await open(page, "versions=1&delaySource=1&delayPackagePlan=1");
        const method = entry === "primary Update" ? "upgrade" : "install";
        const packageName = entry === "primary Update" ? "Avatar tools" : "com.example.remote";
        const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: `Select ${packageName}`, exact: true }) });
        if (entry === "version menu") await chooseVersionSource(page, row, "Example packages");
        const trigger = row.getByRole(entry === "version menu" ? "combobox" : "button", { name: entry === "version menu" ? "Version for com.example.remote: Not installed" : entry === "primary Update" ? "Update" : "Install", exact: true });
        const menu = row.locator(".package-version-picker");
        const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
        if (entry === "version menu") {
            const opened = await packageMenuLifecycle(row, ".package-version-picker");
            await trigger.focus();
            await page.keyboard.press("Enter");
            await opened.evaluate((events) => events.opened);
            await opened.dispose();
            await menu.getByRole("option", { name: "1.1.0", exact: true }).click();
        } else {
            await trigger.focus();
            await page.keyboard.press("Enter");
        }
        await expect.poll(() => page.evaluate(() => typeof window.resumePackagePlan)).toBe("function");
        await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateSource)).toBe("function");
        if (entry === "primary Update") await expect(trigger).toHaveCount(0);
        else if (entry === "version menu") await expect(menu).toHaveJSProperty("disabled", true);
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
            await expect(menu.getByRole("option", { name: "1.1.0", exact: true })).toBeVisible();
            expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual([method]);
        } else {
            await page.keyboard.press("Enter");
            await expect.poll(() => page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual([method, method]);
        }
        expect(await page.evaluate(() => window.packageRequests.some((request) => request.method === "apply"))).toBe(false);
    });
}

for (const key of ["Enter", "Space", "Escape"]) {
    test(`Package version selector ${key} preserves one explicit activation`, async ({ page }) => {
        await open(page, "versions=1");
        const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.remote", exact: true }) });
        await chooseVersionSource(page, row, "Example packages");
        const trigger = row.getByRole("combobox", { name: "Version for com.example.remote: Not installed", exact: true });
        const menu = row.locator(".package-version-picker");
        const opened = await packageMenuLifecycle(row, ".package-version-picker");
        await trigger.focus();
        await page.keyboard.press("Enter");
        await opened.evaluate((events) => events.opened);
        await opened.dispose();
        const option = menu.getByRole("option", { name: "1.1.0", exact: true });
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

test("Disabled package version options reject pointer and Material keyboard activation events", async ({ page }) => {
    await open(page, "versions=1&versionGroups=1");
    const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select com.example.remote", exact: true }) });
    const menu = row.locator(".package-version-picker");
    const opened = await packageMenuLifecycle(row, ".package-version-picker");
    await row.getByRole("combobox", { name: "Version for com.example.remote: Not installed", exact: true }).click();
    await opened.evaluate((events) => events.opened);
    await opened.dispose();
    await expect(menu.locator("md-select-option").filter({ hasText: "9.0.0" })).toHaveCount(0);
    const blocked = menu.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.0.0", exact: true }) });
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
    expect(await page.evaluate(() => window.candidateRequests.some((request) => request.view.kind === "summary" && request.view.packageIds?.join() === "com.example.avatar,com.example.remote"))).toBe(false);
});

test("Candidate pages render names and order installed packages before sources without reading repository catalogs", async ({ page }) => {
    await open(page, "paged=1&deferCandidateSummary=1&reverse-candidate-order=1");
    const first = page.getByRole("checkbox", { name: "Select Local tools", exact: true });
    const updateAll = page.getByRole("button", { name: /^Update All \(\d+\)$/ });
    await expect(first).toBeVisible();
    await expect(updateAll).toBeDisabled();
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateSummary)).toBe("function");
    await page.evaluate(() => window.resumeCandidateSummary?.());
    await expect(updateAll).toBeEnabled();
    await expect(first).toBeVisible();
    await expect(page.getByRole("checkbox", { name: "Select Avatar tools", exact: true })).toBeVisible();
    const table = page.getByRole("table", { name: "Packages", exact: true });
    await expect(table.locator("tbody tr").nth(0)).toContainText("Avatar tools");
    await expect(table.locator("tbody tr").nth(1)).toContainText("Remote tools");
    await expect(table.locator("tbody tr").nth(2)).toContainText("Local tools");
    expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
});

test("A late Core search response cannot replace the current filtered rows", async ({ page }) => {
    await open(page, "deferCandidateFilter=Remote");
    const search = page.getByRole("searchbox", { name: "Search packages" });
    await search.fill("Remote");
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateFilter)).toBe("function");
    await search.fill("Local");
    const table = page.getByRole("table", { name: "Packages", exact: true });
    await expect(table.locator("tbody tr")).toHaveCount(1);
    await expect(table.locator("tbody tr")).toContainText("Local tools");
    await page.evaluate(async () => {
        window.resumeCandidateFilter?.();
        await new Promise(requestAnimationFrame);
        await new Promise(requestAnimationFrame);
    });
    await expect(table.locator("tbody tr")).toHaveCount(1);
    await expect(table.locator("tbody tr")).toContainText("Local tools");
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
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await open(page, "ambiguous=1");
    await page.getByRole("combobox", { name: "Source for Avatar tools" }).click();
    await page.getByRole("option", { name: "Example packages", exact: true }).click();
    await expect(page.getByRole("button", { name: "Update All (1)", exact: true })).toBeEnabled();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Incomplete catalog keeps versions readable and disables Plan actions including bulk remove", async ({ page }) => {
    await open(page, "catalogIncomplete=1");
    await page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }).check();
    await expect(page.getByRole("button", { name: "Remove selected", exact: true })).toBeDisabled();
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    const option = page.locator("md-select-option").filter({ hasText: "1.10.0" });
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
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    await expect(page.locator(".package-refresh-action")).toHaveAttribute("title", "Candidate evidence is stale.");
    await expect(page.getByRole("table", { name: "Packages", exact: true })).not.toContainText("Candidate evidence is stale.");
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await open(page, "intentStale=1");
    await page.getByRole("button", { name: "Update All (1)", exact: true }).click();
    await expect(page.locator(".package-refresh-action")).toHaveAttribute("title", "Candidate evidence is stale.");
    await expect(page.getByRole("table", { name: "Packages", exact: true })).not.toContainText("Candidate evidence is stale.");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.limit === 1).length)).toBe(1);
});

test("Candidate quota failure is unavailable evidence, never empty/latest or a Plan probe", async ({ page }) => {
    await open(page, "candidateError=package_candidate_limit_exceeded");
    await expect(page.getByRole("table", { name: "Packages", exact: true })).not.toContainText(requestErrorMessage("package_candidate_limit_exceeded"));
    await expect(page.locator(".package-refresh-action")).toHaveAttribute("title", requestErrorMessage("package_candidate_limit_exceeded"));
    await expect(page.locator(".reminder-floating .reminder")).toHaveCount(0);
    await page.getByRole("button", { name: "Retry", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.candidateRequests.length)).toBeGreaterThan(1);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "repositories.refresh"))).toEqual([]);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Summary pages share the first fingerprint while versions are read only after opening a row", async ({ page }) => {
    await open(page, "paged=1&versions=1");
    const requests = await page.evaluate(() => window.candidateRequests);
    expect(requests.filter((request) => request.view.kind === "versions")).toHaveLength(0);
    expect(requests.filter((request) => request.cursor).every((request) => request.expectedSnapshot === request.cursor?.snapshotFingerprint)).toBe(true);
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    await expect.poll(() => page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "versions").map((request) => request.cursor?.offset ?? 0))).toEqual([0, 3, 6]);
    const options = page.locator(".package-version-picker").first().locator("md-select-option");
    await expect(options).toHaveText(["1.10.0", "1.2.3", "1.1.0"]);
    await expect(options.filter({ hasText: "1.1.0" })).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await chooseVersionSource(page, page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }) }), "Local packages");
    await page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }).click();
    await expect(options.filter({ hasText: "1.1.0" })).toHaveJSProperty("disabled", false);
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "versions").every((request) => request.limit === 64 && request.expectedSnapshot === (request.cursor?.snapshotFingerprint ?? request.expectedSnapshot)))).toBe(true);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("Closing a loading version selector stops paging and reopening resumes without a package change", async ({ page }) => {
    await open(page, "paged=1&delayVersions=1&versions=1");
    const select = page.locator(".package-version-picker").first();
    const opened = await select.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("opened", () => resolve(), { once: true })) }));
    await select.getByRole("combobox").click();
    await opened.evaluate(({ event }) => event);
    await opened.dispose();
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateVersions)).toBe("function");
    const closed = await select.evaluateHandle((element) => ({ event: new Promise<void>((resolve) => element.addEventListener("closed", () => resolve(), { once: true })) }));
    await page.keyboard.press("Escape");
    await closed.evaluate(({ event }) => event);
    await closed.dispose();
    await page.evaluate(() => window.resumeCandidateVersions?.());
    await expect(select.locator("md-select-option")).toHaveCount(3);
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "versions").length)).toBe(1);
    await expect(select.getByRole("combobox")).toBeFocused();
    await select.getByRole("combobox").click();
    await expect(select.locator("md-select-option")).toHaveCount(3);
    await expect.poll(() => page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "versions").map((request) => request.cursor?.offset ?? 0))).toEqual([0, 3, 6]);
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
    await page.getByRole("combobox", { name: "Source", exact: true }).click();
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

            expect(await page.evaluate(() => window.repositoryPackageRequests)).toEqual([]);
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

test("Package row actions support the first keyboard selection and later Escape", async ({ page }) => {
    await open(page, "versions=1");
    const row = page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: "Select Avatar tools", exact: true }) });
    const trigger = row.getByRole("button", { name: "More actions for Avatar tools", exact: true });
    const menu = row.locator(".package-actions-menu");
    const first = await packageMenuLifecycle(row, ".package-actions-menu");
    try {
        await trigger.focus();
        await page.keyboard.press("Enter");
        await first.evaluate((events) => events.opened);
        const reinstall = menu.getByRole("menuitem", { name: "Reinstall", exact: true });
        await expect(reinstall).toBeEnabled();
        await reinstall.focus();
        await page.keyboard.press("Space");
        await first.evaluate((events) => events.closed);
    } finally {
        await first.dispose();
    }
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(page.getByRole("dialog", { name: "Apply package changes?", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["reinstall"]);
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(trigger).toBeFocused();
    const next = await packageMenuLifecycle(row, ".package-actions-menu");
    try {
        await page.keyboard.press("Enter");
        await next.evaluate((events) => events.opened);
        await expect(menu.getByRole("menuitem", { name: "Reinstall", exact: true })).toBeEnabled();
        await page.keyboard.press("Escape");
        await next.evaluate((events) => events.closed);
    } finally {
        await next.dispose();
    }
    await expect(trigger).toBeFocused();
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["reinstall"]);
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
                return { projectId: request.projectId, snapshot: { fingerprint: "0".repeat(64), projectRevision: request.expectedRevision, configRevision: 1 }, showPrerelease: false, catalogComplete: true, presentationComplete: true, nextCursor: null, view: "summary", items: [] };
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
    await page.getByRole("combobox", { name: "Source for Avatar tools" }).click();
    const initialMetadataRequests = await page.evaluate(() => window.repositoryPackageRequests.length);
    await page.getByRole("option", { name: "Local packages", exact: true }).click();
    await expect.poll(() => page.evaluate(() => typeof window.resumeCandidateSource)).toBe("function");
    await expect(page.getByRole("button", { name: "Update", exact: true })).toHaveCount(0);
    await expect(page.locator(".package-version-picker").filter({ has: page.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3" }) })).toHaveJSProperty("disabled", true);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await page.evaluate(() => window.resumeCandidateSource?.());
    await page.getByRole("button", { name: "Update", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?" })).toBeVisible();
    expect(await page.evaluate(() => window.repositoryPackageRequests.length)).toBe(initialMetadataRequests);
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "upgrade", params: { source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000112" } } }]);
});

async function chooseVersionSource(page: Page, row: Locator, source: string) {
    if (await row.locator(".package-source-picker").count() === 0) {
        await expect(row.locator(".package-source-summary")).toHaveText(source);
        return;
    }
    await row.locator(".package-source-picker").getByRole("combobox").click();
    await page.getByRole("option", { name: source, exact: true }).click();
    await expect(row.locator(".package-version-picker")).toHaveJSProperty("disabled", false);
}
