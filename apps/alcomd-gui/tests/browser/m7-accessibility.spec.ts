import { expect, test, type Page } from "@playwright/test";

test("keyboard navigation moves routes and focuses the destination heading", async ({ page }) => {
    await openHarness(page, "/");
    const projects = navigationItem(page, "Projects");
    await projects.focus();
    await expect(projects).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "Projects" })).toBeFocused();
    expect(await navigationItem(page, "Projects").evaluate((element) => element.getAttribute("aria-current"))).toBe("page");

    const settings = navigationItem(page, "Settings");
    await settings.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeFocused();
    const theme = page.getByRole("combobox", { name: "Theme" });
    await theme.focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(page.locator("md-outlined-select#settings-theme")).toHaveJSProperty("value", "light");
});

test("H1 shell exposes the approved user areas without promoting internal routes", async ({ page }) => {
    await openHarness(page, "/projects");
    for (const name of ["Projects", "Resources", "Settings", "Logs"]) {
        await expect(navigationItem(page, name)).toBeVisible();
    }
    await expect(navigationItem(page, "Extensions")).toBeVisible();
    for (const hiddenRoute of ["Home", "Repositories", "Templates", "Unity", "Operations", "Activity", "Diagnostics"]) {
        await expect(navigationItem(page, hiddenRoute)).toHaveCount(0);
    }
    await expect(navigationItem(page, "Task Center")).toBeVisible();
    await expect(navigationItem(page, "About")).toBeVisible();

    await navigationItem(page, "Resources").click();
    await expect(page.getByRole("heading", { level: 1, name: "Repositories" })).toBeFocused();
    expect(await navigationItem(page, "Resources").evaluate((element) => element.getAttribute("aria-current"))).toBe("page");
});

test("modal traps focus, closes on Escape, and restores the invoking control", async ({ page }) => {
    await openHarness(page, "/projects");
    const invoke = page.getByRole("button", { name: "Register project" });
    await invoke.click();
    const root = page.getByLabel("Project root");
    const review = page.getByRole("button", { name: "Review registration" });
    await expect(root).toHaveAttribute("required", "");
    await expect(root).toHaveAttribute("aria-describedby", "description");
    await expect(page.getByText("The daemon validates and owns this path.", { exact: true })).toBeVisible();
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
    await expect(page.getByRole("button", { name: "Confirm" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(invoke).toBeFocused();
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
        ["/projects/00000000-0000-4000-8000-000000000101", "<private-project>"],
        ["/projects/00000000-0000-4000-8000-000000000101/packages", "<private-project>"],
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
        await expect(main.locator('.route-state[role="alert"]')).toHaveCount(0);
    }
});

test("project workspace keeps package discovery and user actions in project context", async ({ page }) => {
    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101");
    await expect(page.getByRole("heading", { level: 1, name: "<private-project>" })).toBeVisible();
    await expect(page.getByRole("heading", { level: 2, name: "Manage packages" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: "Open Unity" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Backups" })).toBeVisible();
    const row = page.getByRole("row").filter({ hasText: "Avatar tools" });
    await expect(row).toContainText("com.example.avatar");
    await expect(row).toContainText("1.2.3");
    await expect(row).toContainText("Example packages");
    await page.getByLabel("Search packages").fill("not present");
    await expect(page.getByRole("status").filter({ hasText: "No matching packages" })).toBeVisible();
    await page.getByLabel("Search packages").fill("");
    await row.getByRole("button", { name: "1.3.0" }).click();
    const dialog = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(dialog).toContainText("com.example.avatar");
    await expect(dialog).not.toContainText(/plan|revision|fingerprint/i);
});

test("settings are labeled, revisioned, dirty-aware, and applied through the typed client", async ({ page }) => {
    await openHarness(page, "/settings");
    const color = page.locator("md-outlined-select#settings-color");
    await expect(page.getByRole("combobox", { name: "Source color" })).toHaveAccessibleDescription("Saved as a canonical #RRGGBB value; extensions never receive this preference.");
    await selectMaterialOption(color, "#315DA8");
    await selectMaterialOption(page.locator("md-outlined-select#settings-locale"), "zh-CN");

    await navigationItem(page, "Projects").click();
    const dialog = page.getByRole("dialog", { name: "Discard unsaved changes?" });
    await expect(dialog).toBeVisible();
    await page.getByRole("button", { name: "Keep editing" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();

    await page.getByRole("button", { name: "Save settings" }).click();
    await expect(page.getByText("Config Schema 1 · revision 8")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "blue");
});

test("package changes use a v3-style confirmation while the durable Plan stays internal", async ({ page }) => {
    await openHarness(page, `/projects/00000000-0000-4000-8000-000000000101/packages`);
    await page.getByRole("row").filter({ hasText: "Avatar tools" }).getByRole("button", { name: "1.3.0" }).click();
    const dialog = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(dialog).toContainText("Review the changes ALCOMD will make");
    await expect(dialog).toContainText("com.example.avatar");
    await expect(dialog).not.toContainText(/plan|revision|fingerprint/i);
    await page.getByRole("button", { name: "Apply changes" }).click();
    const follow = page.getByRole("status").filter({ hasText: "Package changes" });
    await expect(follow).toContainText(/running|succeeded/);
    await expect(follow).toContainText("succeeded", { timeout: 3_000 });
    await expect(follow.getByRole("progressbar", { name: "Package changes progress" })).toHaveAttribute("aria-valuenow", "1");
});

test("a changed project is explained without exposing stale Plan internals", async ({ page }) => {
    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101/packages", "stale");
    await page.getByRole("row").filter({ hasText: "Avatar tools" }).getByRole("button", { name: "1.3.0" }).click();
    await page.getByRole("button", { name: "Apply changes" }).click();
    await expect(page.getByRole("alert")).toContainText("This project changed");
    await expect(page.getByRole("alert")).not.toContainText("plan_stale");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toHaveCount(0);
});

test("direct writes and the remaining high-impact workflows retain confirmation and Operation boundaries", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "Register project" }).click();
    await page.getByLabel("Project root").fill("C:\\Fixture\\Avatar");
    await page.getByRole("button", { name: "Review registration" }).click();
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Project registered" })).toBeVisible();

    await openHarness(page, "/repositories");
    await page.getByLabel("Repository URL").fill("https://packages.example.invalid/index.json");
    await page.getByRole("button", { name: "Review repository" }).click();
    await expect(page.getByRole("dialog", { name: "Register this repository?" })).toBeVisible();
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository registered" })).toBeVisible();

    await openHarness(page, "/templates");
    await page.getByLabel("Template bundle").fill("C:\\Fixture\\avatar.alcomdtemplate");
    await page.getByRole("button", { name: "Create import plan" }).click();
    await clickDialogAction(page, "Review template import", "Apply reviewed plan");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101/backups");
    await page.getByRole("button", { name: "Review backup" }).click();
    await clickDialogAction(page, "Create this backup?", "Confirm");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/backups/00000000-0000-4000-8000-000000000104");
    await page.getByLabel("Target parent").fill("C:\\Fixture");
    await page.getByLabel("New directory name").fill("Restored");
    await page.getByRole("button", { name: "Create restore plan" }).click();
    await clickDialogAction(page, "Review backup restore", "Apply reviewed plan");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/extensions");
    await page.getByLabel("Extension package").fill("C:\\Fixture\\extension.alcomdext");
    await page.getByRole("button", { name: "Create install plan" }).click();
    await clickDialogAction(page, "Review extension install", "Apply reviewed plan");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/operations/00000000-0000-4000-8000-000000000105");
    await page.getByRole("button", { name: "Cancel operation" }).click();
    await clickDialogAction(page, "Request cancellation?", "Confirm");
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
    await expect(page.locator("md-outlined-select#presence-mode")).toHaveJSProperty("value", "project");
    await expect(page.getByLabel("Refresh interval seconds")).toHaveValue("15");
    const readOnly = page.getByLabel("Custom detail");
    await expect(readOnly).toHaveAttribute("readonly", "");
    await expect(readOnly).toHaveAttribute("aria-invalid", "true");
    await expect(readOnly).toHaveAccessibleDescription(/The host rejected the previous value\./);
    await expect(page.getByRole("separator")).toBeVisible();
    await expect(page.locator(".portable-stack--vertical")).toBeVisible();
    await expect(page.locator(".portable-group")).toBeVisible();
    await expect(page.locator(".portable-key-value")).toContainText("Editing project");

    await openHarness(page, "/extensions/com.cqmhv.mcp-management/ui");
    const mcp = page.getByRole("region", { name: "MCP Management" });
    await expect(page.getByRole("heading", { level: 2, name: "MCP Management" })).toBeVisible();
    await expect(mcp.getByRole("list")).toBeVisible();
    await expect(mcp.getByRole("listitem")).toContainText("Codex desktop");
    await expect(mcp.getByRole("button", { name: "Refresh" })).toBeVisible();
    await expect(page.getByText("Connection mcp-client-01")).toBeVisible();
});

test("dirty Portable UI form requires host-owned discard confirmation", async ({ page }) => {
    await openHarness(page, "/extensions/com.cqmhv.discord/ui");
    await page.getByLabel("Enable presence").uncheck();
    await navigationItem(page, "Settings").click();
    const dialog = page.getByRole("dialog", { name: "Discard unsaved changes?" });
    await expect(dialog).toBeVisible();
    await page.getByRole("button", { name: "Keep editing" }).click();
    await expect(page.getByRole("heading", { level: 2, name: "Discord Presence" })).toBeVisible();
    await navigationItem(page, "Settings").click();
    await expect(dialog).toBeVisible();
    await page.getByRole("button", { name: "Discard changes" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
});

test("fixed desktop navigation, deterministic 200 percent layout, reduced motion, and light/dark contrast remain usable", async ({ page }) => {
    await page.setViewportSize({ width: 320, height: 720 });
    await openHarness(page, "/settings");
    await expect(page.getByRole("button", { name: "Menu" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Close navigation" })).toHaveCount(0);
    const navigation = page.locator("#primary-navigation");
    await expect(navigation).toBeVisible();
    expect((await navigation.boundingBox())?.width).toBe(260);
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await expect(page.locator(".main-content")).toHaveCSS("overflow", "hidden");

    await page.setViewportSize({ width: 640, height: 800 });
    await page.evaluate(() => { document.documentElement.style.fontSize = "200%"; });
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await expect(page.getByRole("combobox", { name: "Language" })).toBeVisible();
    await navigationItem(page, "Projects").click();
    await page.getByRole("button", { name: "Register project" }).click();
    await page.getByLabel("Project root").fill("C:\\Fixture\\Scaled");
    await page.getByRole("button", { name: "Review registration" }).click();
    await expect(page.getByRole("dialog", { name: "Register this project?" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Confirm" })).toBeVisible();
    expect(await hasHorizontalOverflow(page)).toBe(false);
    await page.keyboard.press("Escape");

    await page.emulateMedia({ reducedMotion: "reduce" });
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

function navigationItem(page: Page, label: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${escapeRegex(label)}$`) })
    });
}

function escapeRegex(value: string) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

async function selectMaterialOption(select: ReturnType<Page["locator"]>, value: string) {
    await select.evaluate((element, next) => {
        const materialSelect = element as HTMLElement & { select(value: string): void };
        materialSelect.select(next);
        materialSelect.dispatchEvent(new Event("change", { bubbles: true }));
    }, value);
}

async function clickDialogAction(page: Page, title: string, action: string) {
    await expect(page.getByRole("dialog", { name: title })).toBeVisible();
    await page.getByRole("button", { name: action }).click();
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
