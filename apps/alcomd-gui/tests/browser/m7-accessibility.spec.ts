import { expect, test, type Page } from "@playwright/test";

test("keyboard navigation moves routes and focuses the destination heading", async ({ page }) => {
    await openHarness(page, "/");
    const primary = page.getByRole("navigation", { name: "Primary" });
    const projects = primary.getByRole("button", { name: "Projects" });
    await projects.focus();
    await expect(projects).toHaveCSS("outline-style", "solid");
    await expect(projects).toHaveCSS("outline-width", "3px");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "Projects" })).toBeFocused();
    await expect(projects).toHaveAttribute("aria-current", "page");

    const settings = page.getByRole("button", { name: "Settings", exact: true }).last();
    await settings.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeFocused();
    const theme = page.getByLabel("Theme");
    await theme.focus();
    await page.keyboard.press("ArrowDown");
    await expect(theme).toHaveValue("light");
});

test("H1 shell exposes the approved user areas without promoting internal routes", async ({ page }) => {
    await openHarness(page, "/projects");
    const primary = page.getByRole("navigation", { name: "Primary" });
    for (const name of ["Projects", "Packages & Templates", "Extensions", "Settings", "Log"]) {
        await expect(primary.getByRole("button", { name, exact: true })).toBeVisible();
    }
    for (const hiddenRoute of ["Home", "Repositories", "Templates", "Unity", "Operations", "Activity", "Diagnostics"]) {
        await expect(primary.getByRole("button", { name: hiddenRoute, exact: true })).toHaveCount(0);
    }
    await expect(page.getByRole("button", { name: /Task Center/ })).toContainText("1 active");
    await expect(page.getByRole("button", { name: /About/ })).toContainText("4.0.0-alpha.0");

    await primary.getByRole("button", { name: "Packages & Templates" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Repositories" })).toBeFocused();
    await expect(primary.getByRole("button", { name: "Packages & Templates" })).toHaveAttribute("aria-current", "page");
});

test("modal traps focus, closes on Escape, and restores the invoking control", async ({ page }) => {
    await openHarness(page, "/projects");
    const root = page.getByLabel("Project root");
    const review = page.getByRole("button", { name: "Review registration" });
    await expect(root).toHaveAttribute("required", "");
    await expect(root).toHaveAttribute("aria-describedby", "project-root-hint");
    await expect(page.locator("#project-root-hint")).toHaveText("The daemon validates and owns this path.");
    await expect(review).toBeDisabled();
    expect(await root.evaluate((element) => (element as HTMLInputElement).validity.valueMissing)).toBe(true);
    await root.fill("C:\\Fixture\\Avatar");
    await review.click();

    const dialog = page.getByRole("dialog", { name: "Register this project?" });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("button", { name: "Confirm" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Go back" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    expect(await dialog.evaluate((element) => element.contains(document.activeElement))).toBe(true);
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(review).toBeFocused();
});

test("loading, empty, error, and disconnected states have stable live semantics", async ({ page }) => {
    await openHarness(page, "/projects", "loading");
    await expect(page.getByRole("status").filter({ hasText: "Loading" })).toBeVisible();

    await openHarness(page, "/projects", "empty");
    await expect(page.getByRole("status").filter({ hasText: "No registered projects" })).toBeVisible();

    await openHarness(page, "/projects", "error");
    await expect(page.getByRole("alert").filter({ hasText: "Request failed" })).toBeVisible();

    await openHarness(page, "/projects", "disconnected");
    await expect(page.getByRole("alert").filter({ hasText: "ALCOMD core disconnected" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Reconnect and retry" })).toBeVisible();
});

test("every M1-M7 official GUI route resolves through the typed client", async ({ page }) => {
    const routes = [
        ["/", "Projects"],
        ["/projects", "Projects"],
        ["/projects/00000000-0000-4000-8000-000000000101", "Project detail"],
        ["/projects/00000000-0000-4000-8000-000000000101/packages", "Packages"],
        ["/projects/00000000-0000-4000-8000-000000000101/unity", "Project Unity"],
        ["/projects/00000000-0000-4000-8000-000000000101/backups", "Backups"],
        ["/repositories", "Repositories"],
        ["/repositories/00000000-0000-4000-8000-000000000102", "Repository"],
        ["/templates", "Templates"],
        ["/templates/com.cqmhv.template.avatar", "Template detail"],
        ["/unity", "Unity"],
        ["/backups/00000000-0000-4000-8000-000000000104", "Backup detail"],
        ["/operations", "Operations"],
        ["/operations/00000000-0000-4000-8000-000000000105", "Operation detail"],
        ["/extensions", "Extensions"],
        ["/extensions/com.cqmhv.discord", "Extension detail"],
        ["/extensions/com.cqmhv.discord/ui", "Extension UI"],
        ["/activity", "Activity"],
        ["/diagnostics", "Diagnostics"],
        ["/settings", "Settings"],
        ["/about", "About"]
    ] as const;
    for (const [route, title] of routes) {
        await openHarness(page, route);
        const main = page.getByRole("main");
        await expect(main.getByRole("heading", { level: 1, name: title })).toBeVisible();
        await expect(main.getByRole("alert")).toHaveCount(0);
    }
});

test("settings are labeled, revisioned, dirty-aware, and applied through the typed client", async ({ page }) => {
    await openHarness(page, "/settings");
    const color = page.getByLabel("Source color");
    await expect(color).toHaveAttribute("aria-describedby", "settings-color-hint");
    await color.selectOption("#315DA8");
    await page.getByLabel("Language").selectOption("zh-CN");

    const dialogPromise = page.waitForEvent("dialog");
    const navigationPromise = page.locator(".brand-button").click();
    const dialog = await dialogPromise;
    expect(dialog.message()).toContain("Discard");
    await dialog.dismiss();
    await navigationPromise;
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();

    await page.getByRole("button", { name: "Save settings" }).click();
    await expect(page.getByText("Config Schema 1 · revision 8")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "blue");
});

test("Plan review applies once and follows one durable Operation to a terminal state", async ({ page }) => {
    await openHarness(page, `/projects/00000000-0000-4000-8000-000000000101/packages`);
    await page.getByLabel("Package ID").fill("com.example.avatar");
    await page.getByRole("button", { name: "Create plan" }).click();
    const dialog = page.getByRole("dialog", { name: "Review package plan" });
    await expect(dialog).toContainText("project revision 2");
    await expect(dialog).toContainText("stale plan fails");
    await dialog.getByRole("button", { name: "Apply reviewed plan" }).click();
    const follow = page.getByRole("status").filter({ hasText: "Operation" });
    await expect(follow).toContainText(/running|succeeded/);
    await expect(follow).toContainText("succeeded", { timeout: 3_000 });
    await expect(follow.getByRole("progressbar", { name: "Operation progress" })).toHaveAttribute("value", "1");
});

test("a stale package Plan fails explicitly and is never silently replanned", async ({ page }) => {
    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101/packages", "stale");
    await page.getByLabel("Package ID").fill("com.example.avatar");
    await page.getByRole("button", { name: "Create plan" }).click();
    await page.getByRole("dialog", { name: "Review package plan" }).getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page.getByRole("alert")).toContainText("plan_stale");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toHaveCount(0);
});

test("direct writes and the remaining high-impact workflows retain confirmation and Operation boundaries", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByLabel("Project root").fill("C:\\Fixture\\Avatar");
    await page.getByRole("button", { name: "Review registration" }).click();
    await page.getByRole("dialog", { name: "Register this project?" }).getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Project registered" })).toBeVisible();

    await openHarness(page, "/repositories");
    await page.getByLabel("Repository URL").fill("https://packages.example.invalid/index.json");
    await page.getByRole("button", { name: "Review repository" }).click();
    await page.getByRole("dialog", { name: "Register this repository?" }).getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository registered" })).toBeVisible();

    await openHarness(page, "/templates");
    await page.getByLabel("Template bundle").fill("C:\\Fixture\\avatar.alcomdtemplate");
    await page.getByRole("button", { name: "Create import plan" }).click();
    await page.getByRole("dialog", { name: "Review template import" }).getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101/backups");
    await page.getByRole("button", { name: "Review backup" }).click();
    await page.getByRole("dialog", { name: "Create this backup?" }).getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/backups/00000000-0000-4000-8000-000000000104");
    await page.getByLabel("Target parent").fill("C:\\Fixture");
    await page.getByLabel("New directory name").fill("Restored");
    await page.getByRole("button", { name: "Create restore plan" }).click();
    await page.getByRole("dialog", { name: "Review backup restore" }).getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/extensions");
    await page.getByLabel("Extension package").fill("C:\\Fixture\\extension.alcomdext");
    await page.getByRole("button", { name: "Create install plan" }).click();
    await page.getByRole("dialog", { name: "Review extension install" }).getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/operations/00000000-0000-4000-8000-000000000105");
    await page.getByRole("button", { name: "Cancel operation" }).click();
    await page.getByRole("dialog", { name: "Request cancellation?" }).getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Cancellation requested" })).toBeVisible();

    await openHarness(page, "/diagnostics");
    await page.getByRole("button", { name: "Run state check" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toContainText(/running|succeeded/);
});

test("failed and cancelled Operations expose stable terminal states", async ({ page }) => {
    for (const state of ["failed", "cancelled"] as const) {
        await openHarness(page, "/operations/00000000-0000-4000-8000-000000000105", state);
        const main = page.getByRole("main");
        await expect(main.getByText(state, { exact: true })).toBeVisible();
        await expect(main.getByRole("button", { name: "Cancel operation" })).toBeDisabled();
    }
});

test("Portable UI renders all 17 node kinds with host-owned chrome and safe form semantics", async ({ page }) => {
    await openHarness(page, "/extensions/com.cqmhv.discord/ui");
    await expect(page.getByRole("heading", { level: 1, name: "Extension UI" })).toBeVisible();
    await expect(page.getByRole("definition").first()).toBeVisible();
    await expect(page.getByRole("heading", { level: 2, name: "Discord Presence" })).toBeVisible();
    await expect(page.getByRole("status").filter({ hasText: "Connecting" }).first()).toBeVisible();
    await expect(page.getByRole("progressbar")).toBeVisible();
    await expect(page.getByRole("group").filter({ has: page.getByLabel("Enable presence") })).toBeVisible();
    await expect(page.getByLabel("Enable presence")).toBeChecked();
    await expect(page.getByLabel("Details")).toHaveValue("project");
    await expect(page.getByLabel("Refresh interval seconds")).toHaveValue("15");
    const readOnly = page.getByLabel("Custom detail");
    await expect(readOnly).toHaveAttribute("readonly", "");
    await expect(readOnly).toHaveAttribute("aria-invalid", "true");
    await expect(readOnly).toHaveAttribute("aria-describedby", "presence-text-validation");
    await expect(page.getByRole("separator")).toBeVisible();
    await expect(page.locator(".portable-stack--vertical")).toBeVisible();
    await expect(page.locator(".portable-group")).toBeVisible();
    await expect(page.locator(".portable-key-value")).toContainText("Editing project");

    await openHarness(page, "/extensions/com.cqmhv.mcp-management/ui");
    await expect(page.getByRole("heading", { level: 2, name: "MCP Management" })).toBeVisible();
    await expect(page.getByRole("list")).toBeVisible();
    await expect(page.getByRole("listitem")).toContainText("Codex desktop");
    await expect(page.getByRole("region", { name: "MCP Management" }).getByRole("button", { name: "Refresh" })).toBeVisible();
    await expect(page.getByText("Connection mcp-client-01")).toBeVisible();
});

test("dirty Portable UI form requires host-owned discard confirmation", async ({ page }) => {
    await openHarness(page, "/extensions/com.cqmhv.discord/ui");
    await page.getByLabel("Enable presence").uncheck();
    const dialogPromise = page.waitForEvent("dialog");
    const navigationPromise = page.getByRole("button", { name: "Settings", exact: true }).first().click();
    const dialog = await dialogPromise;
    expect(dialog.message()).toContain("Discard");
    await dialog.dismiss();
    await navigationPromise;
    await expect(page.getByRole("heading", { level: 2, name: "Discord Presence" })).toBeVisible();
});

test("320 CSS px, deterministic 200 percent layout, reduced motion, and light/dark contrast remain usable", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 720 });
    await openHarness(page, "/settings");
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
    const toggle = page.getByRole("button", { name: "Toggle navigation" });
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await toggle.click();
    await expect(toggle).toHaveAttribute("aria-expanded", "true");
    await expect(page.getByRole("navigation", { name: "Primary" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Primary" }).getByRole("button", { name: "Settings", exact: true })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(toggle).toHaveAttribute("aria-expanded", "false");
    await expect(toggle).toBeFocused();
    await toggle.click();
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await expect(page.getByRole("button", { name: "Save settings" })).toBeVisible();

    await page.setViewportSize({ width: 640, height: 800 });
    await page.evaluate(() => { document.documentElement.style.fontSize = "200%"; });
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await expect(page.getByLabel("Language")).toBeVisible();
    await page.getByRole("navigation", { name: "Primary" }).getByRole("button", { name: "Projects" }).click();
    await page.getByLabel("Project root").fill("C:\\Fixture\\Scaled");
    await page.getByRole("button", { name: "Review registration" }).click();
    await expect(page.getByRole("dialog", { name: "Register this project?" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Confirm" })).toBeVisible();
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await page.keyboard.press("Escape");

    await page.emulateMedia({ reducedMotion: "reduce" });
    await toggle.click();
    const duration = await page.locator("#primary-navigation").evaluate((element) => getComputedStyle(element).transitionDuration);
    expect(seconds(duration)).toBeLessThanOrEqual(0.001);

    await page.evaluate(() => { document.documentElement.style.fontSize = ""; });
    for (const theme of ["light", "dark"] as const) {
        await page.emulateMedia({ colorScheme: theme });
        await openHarness(page, "/diagnostics");
        const ratios = await contrastRatios(page);
        expect(ratios.body).toBeGreaterThanOrEqual(4.5);
        expect(ratios.secondary).toBeGreaterThanOrEqual(4.5);
        expect(ratios.action).toBeGreaterThanOrEqual(4.5);
        expect(ratios.error).toBeGreaterThanOrEqual(4.5);
        expect(ratios.focus).toBeGreaterThanOrEqual(3);
    }
});

async function openHarness(page: Page, route: string, state: "ready" | "empty" | "error" | "disconnected" | "loading" | "stale" | "failed" | "cancelled" = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}

async function hasHorizontalOverflow(page: Page): Promise<boolean> {
    return page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth + 1);
}

function seconds(value: string): number {
    const first = value.split(",")[0]?.trim() ?? "0s";
    return first.endsWith("ms") ? Number.parseFloat(first) / 1000 : Number.parseFloat(first);
}

async function contrastRatios(page: Page): Promise<Record<string, number>> {
    return page.evaluate(() => {
        const root = getComputedStyle(document.documentElement);
        const color = (name: string) => parse(root.getPropertyValue(name));
        const ratio = (left: number[], right: number[]) => {
            const luminance = (rgb: number[]) => rgb
                .map((channel) => channel / 255)
                .map((channel) => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4)
                .reduce((sum, channel, index) => sum + channel * [0.2126, 0.7152, 0.0722][index]!, 0);
            const values = [luminance(left), luminance(right)].sort((a, b) => b - a);
            return (values[0]! + 0.05) / (values[1]! + 0.05);
        };
        function parse(input: string): number[] {
            const hex = input.trim();
            if (/^#[0-9a-f]{6}$/i.test(hex)) return [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16));
            const match = hex.match(/[\d.]+/g);
            if (match === null || match.length < 3) throw new Error(`Unsupported computed color: ${input}`);
            return match.slice(0, 3).map(Number);
        }
        const surface = color("--surface");
        return {
            body: ratio(color("--on-surface"), surface),
            secondary: ratio(color("--on-surface-variant"), color("--surface-container")),
            action: ratio(color("--primary"), color("--surface-container")),
            error: ratio(color("--error"), color("--surface-container")),
            focus: ratio(color("--primary"), surface)
        };
    });
}
