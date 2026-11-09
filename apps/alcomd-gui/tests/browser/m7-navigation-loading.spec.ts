import { selectSettingsSection } from "./settings-sections";
import { expect, test, type Page } from "@playwright/test";
import { PreloadCache, withPreloadWork } from "../../src/preload-cache";

declare global {
    interface Window {
        pageAnimationCalls: number;
        pageFrames: { populated: boolean; opaque: boolean; settings: boolean; unityRows: number }[];
    }
}

test.use({ viewport: { width: 1440, height: 900 }, video: { mode: "on", size: { width: 1440, height: 900 } } });

test("Preload ownership shares reads, cancels idle eviction and transfers to the visible page", async () => {
    const cache = new PreloadCache(25);
    let calls = 0;
    let discarded = 0;
    const load = async () => { calls++; return "prepared"; };
    const first = cache.retain("page", "pointer", load, () => discarded++);
    expect(cache.retain("page", "focus", load)).toBe(first);
    expect(await first).toBe("prepared");
    cache.release("page", "pointer");
    await new Promise((resolve) => setTimeout(resolve, 35));
    expect(discarded).toBe(0);
    cache.release("page", "focus");
    cache.retain("page", "pointer", load);
    await new Promise((resolve) => setTimeout(resolve, 35));
    expect(discarded).toBe(0);
    expect(cache.take<string>("page")).toBe(first);
    cache.release("page", "pointer");
    await new Promise((resolve) => setTimeout(resolve, 35));
    expect(discarded).toBe(0);
    expect(calls).toBe(1);
    cache.retain("idle", "pointer", load, () => discarded++);
    cache.release("idle", "pointer");
    await expect.poll(() => discarded).toBe(1);
    expect(cache.take("idle")).toBeUndefined();
    cache.dispose();
    cache.activate();
    expect(await cache.retain("remounted", "focus", load)).toBe("prepared");
    cache.dispose();
});

test("Discarded in-flight preloads retain their slots while queued navigation can run in the foreground", async () => {
    const cache = new PreloadCache();
    const started: string[] = [];
    let finishA!: (value: string) => void;
    let finishB!: (value: string) => void;
    const a = cache.retain("a", "a", () => { started.push("a"); return new Promise<string>((resolve) => { finishA = resolve; }); });
    const b = cache.retain("b", "b", () => { started.push("b"); return new Promise<string>((resolve) => { finishB = resolve; }); });
    await expect.poll(() => started.length).toBe(2);
    const c = cache.retain("c", "c", async () => { started.push("c"); return "foreground"; });
    cache.drop("a");
    await expect(a).rejects.toHaveProperty("name", "AbortError");
    expect(started).toEqual(["a", "b"]);
    expect(cache.foreground("c", "c", async () => { throw new Error("duplicate preparation"); }, () => {})).toBe(c);
    expect(cache.take<string>("c")).toBe(c);
    expect(await c).toBe("foreground");
    const d = cache.retain("d", "d", async () => { started.push("d"); return "next"; });
    await Promise.resolve();
    expect(started).toEqual(["a", "b", "c"]);
    finishA("discarded");
    expect(await d).toBe("next");
    finishB("retained");
    expect(await b).toBe("retained");
    cache.dispose();
});

test("Metadata can be claimed before its list finishes without releasing speculative work slots early", async () => {
    const cache = new PreloadCache();
    let finish!: () => void;
    const work = new Promise<void>((resolve) => { finish = resolve; });
    const first = cache.retain<string>("metadata", "pointer", async () => withPreloadWork("metadata", work));
    expect(await first).toBe("metadata");
    expect(await cache.take<string>("metadata")).toBe("metadata");
    let finishOther!: () => void;
    const other = cache.retain("other", "pointer", () => new Promise<void>((resolve) => { finishOther = resolve; }));
    let started = false;
    const queued = cache.retain("queued", "pointer", async () => { started = true; return "list slot"; });
    await Promise.resolve();
    expect(started).toBe(false);
    finish();
    expect(await queued).toBe("list slot");
    finishOther(); await other;
    cache.dispose();
});

test("Preload capacity and resource revisions evict unused data instead of retaining it across visits", async () => {
    const cache = new PreloadCache();
    let evicted = 0;
    for (let index = 0; index < 17; index++) {
        await cache.retain(`page:${index}`, "pointer", async () => index, () => evicted++);
        cache.release(`page:${index}`, "pointer");
    }
    expect(evicted).toBeGreaterThan(0);
    expect(cache.take("page:0")).toBeUndefined();
    let valid = true;
    await cache.retain("revision", "pointer", async () => "old data", () => evicted++, undefined, () => valid);
    valid = false; cache.dropInvalid();
    expect(cache.take("revision")).toBeUndefined();
    await cache.retain("large", "pointer", async () => "x".repeat(2 * 1024 * 1024), () => evicted++);
    cache.release("large", "pointer");
    expect(cache.take("large")).toBeUndefined();
    cache.dispose();
});

test("Resources show their title, navigation and table loading independently and keep the table header fixed", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Frepositories&state=ready&holdRepositories=1");
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
    await expect(page.getByRole("group", { name: "Resource sections" })).toBeVisible();
    await expect(page.getByRole("table", { name: "Repositories", exact: true }).locator("thead")).toBeVisible();
    await expect(page.locator(".page-loading-state")).toHaveCount(0);
    await expect(page.locator(".desktop-list-empty").getByRole("progressbar")).toBeVisible();
    const title = await page.locator("#route-title").elementHandle();
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    const viewport = page.locator(".utility-workspace-body .desktop-list-viewport").first();
    await expect(viewport.locator("tbody tr").first()).not.toContainText("Loading");
    await expect(page.locator(".loading-state--bar")).toHaveCount(0);
    expect(await title!.evaluate((element) => element.isConnected)).toBe(true);
    await viewport.evaluate((element) => { const body = element.querySelector("tbody")!; const row = body.firstElementChild!; for (let index = 0; index < 40; index++) body.append(row.cloneNode(true)); });
    const before = (await viewport.locator("thead").boundingBox())!.y;
    await viewport.evaluate((element) => { element.scrollTop = 500; });
    await expect.poll(async () => (await viewport.locator("thead").boundingBox())!.y).toBeCloseTo(before, 0);
    expect(await viewport.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    expect(await page.locator(".utility-workspace-body").evaluate((element) => element.scrollTop)).toBe(0);
});

test("Navigation hover prepares a read without navigating and a click takes over the same pending request", async ({ page }) => {
    await open(page, "holdRepositories=1");
    const target = navigation(page, "Resources");
    const before = await page.evaluate(() => window.repositoryListReads);
    await target.hover();
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await target.click();
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
    await expect(page.locator(".desktop-list-empty").getByRole("progressbar")).toBeVisible();
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await expect(page.getByRole("table", { name: "Repositories", exact: true }).locator("tbody")).toContainText("Example packages");
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
});

test("Navigation hover ignores a quick pass and starts preparing at 50ms", async ({ page }) => {
    await open(page, "holdRepositories=1");
    await page.clock.install();
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    const target = navigation(page, "Resources");
    const before = await page.evaluate(() => window.repositoryListReads);
    await target.dispatchEvent("pointerover", { pointerType: "mouse", isPrimary: true });
    await page.clock.runFor(20);
    await target.dispatchEvent("pointerout", { pointerType: "mouse", isPrimary: true });
    await page.clock.runFor(100);
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before);
    await target.dispatchEvent("pointerover", { pointerType: "mouse", isPrimary: true });
    await page.clock.runFor(49);
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before);
    await page.clock.runFor(1);
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.clock.resume();
});

for (const pointerType of ["mouse", "touch"]) {
    test(`A fast primary ${pointerType} press prepares navigation before focus or click and shares the pending read`, async ({ page }) => {
        await open(page, "holdRepositories=1");
        await page.clock.install();
        await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
        const target = navigation(page, "Resources");
        const before = await page.evaluate(() => window.repositoryListReads);
        await target.dispatchEvent("pointerover", { pointerType, isPrimary: true });
        await page.clock.runFor(10);
        expect(await page.evaluate(() => window.repositoryListReads)).toBe(before);
        await target.dispatchEvent("pointerdown", { pointerType, isPrimary: true, button: 0 });
        await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
        expect(await target.evaluate((element) => element.contains(document.activeElement))).toBe(false);
        await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
        await target.dispatchEvent("pointerup", { pointerType, isPrimary: true, button: 0 });
        await target.dispatchEvent("click");
        await page.clock.runFor(100);
        await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeVisible();
        await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
        await page.clock.resume();
        await expect(page.getByRole("table", { name: "Repositories", exact: true })).toContainText("Example packages");
        expect(await page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    });
}

test("Non-primary presses do not prepare navigation and a cancelled press expires without navigating", async ({ page }) => {
    await open(page, "holdRepositories=1");
    await page.clock.install();
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    const target = navigation(page, "Resources");
    const before = await page.evaluate(() => window.repositoryListReads);
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 2 });
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 1 });
    await target.dispatchEvent("pointerdown", { pointerType: "touch", isPrimary: false, button: 0 });
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before);
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 0 });
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    await target.dispatchEvent("pointercancel", { pointerType: "mouse", isPrimary: true });
    await page.clock.runFor(3_000);
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 0 });
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 2);
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.clock.resume();
});

test("A project Manage press prepares its package workspace before navigation", async ({ page }) => {
    await open(page, "holdPackageCandidates=1");
    const target = page.getByRole("button", { name: "Manage", exact: true });
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 0 });
    await expect.poll(() => page.evaluate(() => typeof window.resumePackageCandidates)).toBe("function");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await target.click();
    await page.evaluate(() => window.resumePackageCandidates?.());
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.candidateRequests.filter((request) => request.view.kind === "summary" && !request.cursor && !request.view.filter && request.view.bulkPackageIds === undefined).length)).toBe(1);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("A resource segment press prepares the destination before switching sections", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fuser-packages&state=ready&holdRepositories=1");
    const target = page.getByRole("navigation", { name: "Resources", exact: true }).getByRole("button", { name: "Repositories", exact: true });
    await expect(target).toBeVisible();
    const before = await page.evaluate(() => window.repositoryListReads);
    await target.dispatchEvent("pointerdown", { pointerType: "mouse", isPrimary: true, button: 0 });
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
    await target.click();
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toContainText("Example packages");
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(before + 1);
});

test("Keyboard navigation preloads expire after blur and a late discarded read cannot refill them", async ({ page }) => {
    await open(page, "holdRepositories=1");
    const target = navigation(page, "Resources");
    await target.focus();
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(1);
    await page.locator("#route-title").focus();
    await page.waitForTimeout(3200);
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await target.focus();
    await expect.poll(() => page.evaluate(() => window.repositoryListReads)).toBe(2);
    await target.click();
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toContainText("Example packages");
    expect(await page.evaluate(() => window.repositoryListReads)).toBe(2);
});

test("Repository details are ready while only their package table is still loading", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Frepositories%2F00000000-0000-4000-8000-000000000102&state=ready&holdRepositoryPackages=1");
    await expect(page.getByRole("heading", { level: 1, name: "Repository", exact: true })).toBeVisible();
    await expect(page.locator(".detail-grid")).toContainText("Example packages");
    await expect(page.getByRole("table", { name: "Repository packages", exact: true }).locator("thead")).toBeVisible();
    await expect(page.locator(".page-loading-state")).toHaveCount(0);
    await page.evaluate(() => window.resumeRepositoryPackages?.());
    await expect(page.getByRole("table", { name: "Repository packages", exact: true }).locator("tbody")).toContainText("Avatar tools");
});

async function open(page: Page, params = "") {
    await page.goto(`/browser-harness.html?route=%2Fprojects&state=ready&${params}`);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.locator(".route-content").evaluate((content) => {
        window.pageAnimationCalls = 0;
        const animate = Element.prototype.animate;
        Element.prototype.animate = function (frames, options) {
            if (content.contains(this)) window.pageAnimationCalls += 1;
            return animate.call(this, frames, options);
        };
    });
}

function navigation(page: Page, name: string) {
    return page.locator("#primary-navigation .navigation-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) })
    });
}

for (const width of [480, 1440]) {
    test(`Whole-page loading stays at the content canvas top at ${width}px`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await open(page, "holdSettings=1&armSettingsHold=1&motion=full");
        await page.evaluate(() => window.dispatchEvent(new Event("test-settings-hold")));
        await navigation(page, "Settings").click();
        const loading = page.locator(".page-loading-state");
        const progress = loading.getByRole("progressbar", { name: "Loading page…", exact: true });
        await expect(progress).toBeVisible();
        await expect(page.locator(".projects-page")).toBeHidden();
        await expect(progress).not.toHaveAttribute("aria-valuenow", /.+/);
        const canvasBounds = (await page.locator("#main-content").boundingBox())!;
        const progressBounds = (await progress.boundingBox())!;
        expect(Math.abs(progressBounds.x - canvasBounds.x)).toBeLessThan(1);
        expect(Math.abs(progressBounds.y - canvasBounds.y)).toBeLessThan(1);
        expect(Math.abs(progressBounds.width - canvasBounds.width)).toBeLessThan(1);
        expect(progressBounds.height).toBe(4);
        await expect(loading).toHaveCSS("pointer-events", "none");
        await expect(page.locator("#primary-navigation .page-loading-state")).toHaveCount(0);
        await expect(page.locator(".route-content > .loading-state")).toHaveCount(0);
        const animation = await loading.locator("md-linear-progress").evaluate((element) => getComputedStyle(element.shadowRoot!.querySelector(".primary-bar")!).animationName);
        expect(animation).toContain("primary-indeterminate-translate");
        await page.screenshot({ path: test.info().outputPath("page-top-loading-bar.png") });
        await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
        await expect(loading).toHaveCount(0);
    });
}

test("Whole-page loading honors reduced motion while retaining its progress semantics", async ({ page }) => {
    await open(page, "holdSettings=1&armSettingsHold=1&motion=reduced");
    await page.evaluate(() => window.dispatchEvent(new Event("test-settings-hold")));
    await navigation(page, "Settings").click();
    const loading = page.locator(".page-loading-state");
    const progress = loading.getByRole("progressbar", { name: "Loading page…", exact: true });
    await expect(progress).toBeVisible();
    await expect(loading).toHaveAttribute("aria-busy", "true");
    await expect.poll(() => loading.locator("md-linear-progress").evaluate((element) => getComputedStyle(element.shadowRoot!.querySelector(".primary-bar")!).animationDuration)).toBe("1e-06s");
    await expect(loading).toContainText("Loading page…");
});

for (const view of ["list", "grid"] as const) {
    test(`Projects ${view} shows its toolbar while the initial rows are still loading`, async ({ page }) => {
        await page.addInitScript((mode) => window.localStorage.setItem("alcomd.gui.projects.view", mode), view);
        await page.goto("/browser-harness.html?route=%2Frepositories&state=ready&holdProjectsList=1");
        await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
        await navigation(page, "Projects").click();
        await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeFocused();
        await expect(page.getByRole("searchbox", { name: "Search projects", exact: true })).toBeEnabled();
        await expect(page.getByRole("button", { name: view === "list" ? "List view" : "Grid view", exact: true })).toBeEnabled();
        await expect(page.getByRole("button", { name: "Create project", exact: true })).toBeEnabled();
        await expect(page.locator(".projects-content")).toHaveAttribute("aria-busy", "true");
        await expect(page.locator(".projects-empty")).toHaveCount(0);
        if (view === "list") {
            await expect(page.getByRole("table", { name: "Projects", exact: true }).locator("thead")).toBeVisible();
            await expect(page.getByRole("progressbar", { name: "Loading projects", exact: true })).toBeVisible();
        } else await expect(page.getByRole("table", { name: "Projects", exact: true })).toHaveCount(0);
        await expect(page.locator(".project-card")).toHaveCount(0);
        const title = await page.locator("#route-title").elementHandle();
        const search = page.getByRole("searchbox", { name: "Search projects", exact: true });
        await search.focus();
        await page.evaluate(() => window.resumeProjectsList?.());
        await expect(page.locator(view === "list" ? ".projects-list" : ".projects-grid")).toBeVisible();
        await expect(page.locator(".projects-content")).toHaveAttribute("aria-busy", "false");
        await expect(search).toBeFocused();
        expect(await title!.evaluate((element) => element.isConnected)).toBe(true);
        expect(await page.evaluate(() => window.projectListReads)).toBe(1);
    });
}

test("Projects search made before the initial rows arrive wins over the old initial read", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready&holdProjectsList=1");
    const search = page.getByRole("searchbox", { name: "Search projects", exact: true });
    await search.fill("no-project-matches-this");
    await expect(search).toHaveValue("no-project-matches-this");
    await expect(page.getByRole("table", { name: "Projects", exact: true }).locator("thead")).toBeVisible();
    await page.evaluate(() => window.resumeProjectsList?.());
    await expect(page.getByRole("heading", { name: "No matching projects", exact: true })).toBeVisible();
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(page.getByRole("heading", { name: "No matching projects", exact: true })).toBeVisible();
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toHaveCount(0);
    await expect(search).toHaveValue("no-project-matches-this");
    await expect(search).toBeFocused();
});

test("Projects initial read failure preserves the toolbar and retries the list", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready&holdProjectsList=1");
    await expect(page.getByRole("searchbox", { name: "Search projects", exact: true })).toBeEnabled();
    await page.evaluate(() => window.resumeProjectsList?.("internal_error"));
    await expect(page.getByRole("alert")).toContainText("internal_error");
    await expect(page.getByRole("searchbox", { name: "Search projects", exact: true })).toBeEnabled();
    await page.getByRole("button", { name: "Reconnect and retry", exact: true }).click();
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
});

test("A late initial Projects list cannot replace another page", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready&holdProjectsList=1");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeFocused();
    await navigation(page, "Resources").click();
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
    await page.evaluate(() => window.resumeProjectsList?.());
    await page.evaluate(() => new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeFocused();
    await expect(page.locator(".projects-page")).toHaveCount(0);
});

async function frames(page: Page) {
    await page.evaluate(() => {
        window.pageFrames = [];
        const sample = () => {
            const content = document.querySelector(".route-content")!;
            const settings = content.querySelector(".settings-workspace");
            window.pageFrames.push({
                populated: content.querySelector("table, .settings-workspace, .logs-workspace") !== null,
                opaque: [content, content.firstElementChild!].every((node) => getComputedStyle(node).opacity === "1"),
                settings: settings !== null,
                unityRows: settings?.querySelectorAll('table[aria-label="Unity installations"] tbody tr').length ?? 0
            });
            requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
    });
}

for (const origin of ["Projects", "Resources", "Logs"]) {
    test(`${origin} to Settings waits at most 100ms before showing the destination loading state`, async ({ page }) => {
        await page.clock.install();
        await open(page, "holdSettings=1&armSettingsHold=1");
        if (origin !== "Projects") {
            await navigation(page, origin).click();
            await expect(page.getByRole("heading", { level: 1, name: origin, exact: true })).toBeFocused();
        }
        await page.evaluate(() => window.dispatchEvent(new Event("test-settings-hold")));
        await frames(page);
        await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
        await navigation(page, "Settings").evaluate((button: HTMLElement) => button.click());
        await page.clock.runFor(100);
        await expect(page.locator(".route-content #route-title")).toHaveText("Settings");
        await expect(page.locator(".route-content .settings-workspace")).toHaveCount(0);
        await expect(page.locator(".route-content")).not.toHaveAttribute("inert", "");
        await expect(page.locator(".page-loading-state")).toBeAttached();
        await page.clock.runFor(160);
        await expect(page.locator(".page-loading-state md-linear-progress")).toBeVisible();
        await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
        await page.clock.resume();
        await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
        await expect(page.locator('table[aria-label="Unity installations"] tbody tr')).toHaveCount(1);
        await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
        const samples = await page.evaluate(() => window.pageFrames);
        expect(samples.every((frame) => frame.opaque && (!frame.settings || frame.unityRows === 1))).toBe(true);
        expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
    });
}

test("Settings shows other sections while only Unity installations are loading, without requesting repositories", async ({ page }) => {
    await page.clock.install();
    await open(page, "holdSettings=1&holdUnity=1&holdRepositories=1");
    await frames(page);
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await navigation(page, "Settings").evaluate((button: HTMLElement) => button.click());
    await page.evaluate(() => window.dispatchEvent(new Event("test-settings-ready")));
    await page.clock.runFor(100);
    await expect(page.locator(".route-content .projects-page")).toBeHidden();
    await expect(page.getByRole("region", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.locator(".page-loading-state")).toHaveCount(0);
    await selectSettingsSection(page, "unity");
    await page.clock.runFor(160);
    await expect(page.getByRole("table", { name: "Unity installations", exact: true }).locator("thead")).toBeVisible();
    await expect(page.getByRole("table", { name: "Unity installations", exact: true }).getByRole("progressbar")).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new Event("test-unity-ready")));
    await page.clock.resume();
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeVisible();
    await expect(page.locator(".utility-workspace-toolbar nav.resource-navigation").getByRole("button").nth(1)).toBeFocused();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
    await expect(page.getByRole("region", { name: "Packages", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.pageFrames.every((frame) => frame.opaque && (!frame.settings || frame.unityRows === 1)))).toBe(true);
});

test("abandoned pending reads cannot replace the latest route", async ({ page }) => {
    await page.clock.install();
    await open(page, "holdSettings=1");
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await navigation(page, "Settings").evaluate((button: HTMLElement) => button.click());
    await page.clock.runFor(100);
    await expect(page.locator("#route-title")).toHaveText("Settings");
    await page.clock.runFor(160);
    await expect(page.locator(".route-content .page-loading-state md-linear-progress")).toBeVisible();
    await navigation(page, "Logs").evaluate((button: HTMLElement) => button.click());
    await page.clock.resume();
    await expect(page.getByRole("table", { name: "Logs", exact: true })).toBeVisible();
    await page.evaluate(async () => {
        window.dispatchEvent(new Event("test-settings-ready"));
        await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    });
    await expect(page.getByRole("heading", { level: 1, name: "Logs", exact: true })).toBeFocused();
    await expect(page.locator(".settings-workspace")).toHaveCount(0);
    expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
});

test("a failed pending read commits a real error with a retry action", async ({ page }) => {
    await page.clock.install();
    await open(page, "holdRepositories=1");
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 1_000));
    await navigation(page, "Resources").evaluate((button: HTMLElement) => button.click());
    await page.clock.runFor(100);
    await expect(page.locator("#route-title")).toHaveText("Resources");
    await expect(page.locator(".page-loading-state")).toHaveCount(0);
    await page.clock.runFor(160);
    await expect(page.locator(".desktop-list-empty md-linear-progress")).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new CustomEvent("test-repositories-ready", { detail: "error" })));
    await page.clock.resume();
    const alert = page.getByRole("alert");
    await expect(alert).toContainText("Core is unavailable. Reconnect and check any submitted task before repeating a change.");
    await expect(alert).toContainText("daemon_unavailable");
    await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeEnabled();
    await expect(page.locator("#main-content")).toHaveAttribute("aria-busy", "false");
});

test("Settings never requests repository visibility data across its sections", async ({ page }) => {
    await open(page, "h5-settings=1");
    await page.evaluate(() => window.h5Settings.failNext("repositories.list", "permission_denied"));
    await navigation(page, "Settings").click();
    await expect(page.getByRole("heading", { level: 1, name: "Settings", exact: true })).toBeFocused();
    await expect(page.locator('table[aria-label="Unity installations"] tbody tr')).toHaveCount(1);
    await selectSettingsSection(page, "unity");
    await expect(page.getByRole("table", { name: "Unity installations", exact: true }).locator("tbody tr")).toHaveCount(1);
    for (const section of ["appearance", "general"] as const) await selectSettingsSection(page, section);
    await expect(page.getByRole("region", { name: "Projects", exact: true })).toBeVisible();
    await expect(page.getByRole("region", { name: "Backups", exact: true })).toBeVisible();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(await page.evaluate(() => window.h5Settings.requests.filter((request) => request.method === "repositories.list"))).toEqual([]);
});

test("business updates never start page animations or replace current rows with loading", async ({ page }) => {
    await open(page);
    await navigation(page, "Resources").click();
    const rows = page.getByRole("table", { name: "Repositories", exact: true });
    await expect(rows).toBeVisible();
    const table = await rows.elementHandle();
    await expect(page.locator('.page-header [data-icon-name="refresh"]')).toHaveCount(0);
    await rows.getByRole("button", { name: "Browse packages", exact: true }).first().click();
    const refresh = page.locator("md-dialog[open]").getByRole("button", { name: "Refresh repository", exact: true });
    await refresh.click();
    await expect(refresh).toBeEnabled();
    expect(await table!.evaluate((element) => element.isConnected)).toBe(true);
    expect(await page.evaluate(() => window.pageAnimationCalls)).toBe(0);
    await expect(page.locator(".loading-state")).toHaveCount(0);
});

test("filter opens without custom motion and Escape restores focus", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects%2F00000000-0000-4000-8000-000000000101%2Fpackages&state=package-multiple");
    const trigger = page.getByRole("button", { name: "Filter packages", exact: true });
    await trigger.click();
    const panel = page.getByRole("dialog", { name: "Package filters", exact: true });
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("transition-duration", "0s");
    await expect(panel).toHaveCSS("transform", "none");
    await page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
    await expect(trigger).toBeFocused();
});
