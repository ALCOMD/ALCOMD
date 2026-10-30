import { expect, test, type Page } from "@playwright/test";

test("keyboard navigation moves routes and focuses the destination heading", async ({ page }) => {
    await openHarness(page, "/");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeFocused();
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
    await page.locator("#settings-theme-button").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeVisible();
    const light = page.locator("#theme-mode").getByRole("button", { name: "Light", exact: true });
    await light.focus();
    await expect(light).toBeFocused();
    await page.keyboard.press("Enter");
    await expect(light).toHaveAttribute("aria-pressed", "true");
    await page.keyboard.press("Space");
    await expect(light).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator("#theme-mode").getByRole("button", { pressed: true })).toHaveCount(1);
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
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
    await expect(navigationItem(page, "Tasks")).toBeVisible();
    await expect(navigationItem(page, "About")).toBeVisible();

    await navigationItem(page, "Resources").click();
    await expect(page.getByRole("heading", { level: 1, name: "Resources" })).toBeFocused();
    expect(await navigationItem(page, "Resources").evaluate((element) => element.getAttribute("aria-current"))).toBe("page");
});

test("modal traps focus, closes on Escape, and restores the invoking control", async ({ page }) => {
    await openHarness(page, "/projects");
    const invoke = page.getByRole("button", { name: "More project creation options" });
    await invoke.click();
    await page.getByRole("menuitem", { name: "Add existing project", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Register this project?" });
    await expect(dialog).toBeVisible();
    await expect(page.locator(".project-register-review code")).toHaveText("C:\\Fixture\\Avatar");
    await expect(page.getByRole("button", { name: "Confirm" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Cancel" })).toBeFocused();
    await page.keyboard.press("Shift+Tab");
    await expect(page.getByRole("button", { name: "Confirm" })).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await expect(invoke).toBeFocused();
});

test("ordinary route changes never replace content with capability negotiation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready&slowStatus=1");
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.evaluate(() => {
        window.capabilityLoadingScreens = [];
        // Record transient mounts as well as stable states; retrying assertions can miss a single frame.
        const observer = new MutationObserver((records) => {
            for (const record of records) for (const node of record.addedNodes) {
                if (!(node instanceof Element)) continue;
                const loading = node.matches(".loading-state") ? [node] : node.querySelectorAll(".loading-state");
                for (const state of loading) {
                    if (state.textContent?.includes("Checking daemon capabilities.")) window.capabilityLoadingScreens.push(state.textContent);
                }
            }
        });
        observer.observe(document.getElementById("main-content")!, { childList: true, subtree: true });
    });
    for (const [item, heading] of [["Resources", "Resources"], ["Settings", "Settings"], ["Logs", "Logs"], ["Projects", "Projects"]] as const) {
        await navigationItem(page, item).click();
        await expect(page.getByRole("heading", { level: 1, name: heading, exact: true })).toBeVisible();
    }
    await navigationItem(page, "Resources").focus();
    await page.keyboard.press("Enter");
    await expect(page.getByRole("table", { name: "Repositories", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.capabilityLoadingScreens)).toEqual([]);
    await expect(page.getByRole("heading", { level: 1, name: "Resources", exact: true })).toBeFocused();
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

test("same-route reconnect refreshes negotiated capabilities before restoring actions", async ({ page }) => {
    await openHarness(page, "/projects", "disconnected");
    await expect(page.getByRole("alert").filter({ hasText: "ALCOMD core disconnected" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Create project", exact: true })).toBeDisabled();
    await page.getByRole("button", { name: "Reconnect and retry" }).click();
    await expect(page.getByRole("alert").filter({ hasText: "ALCOMD core disconnected" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Create project", exact: true })).toBeDisabled();
    await page.evaluate(() => window.dispatchEvent(new Event("test-daemon-online")));
    await page.getByRole("button", { name: "Reconnect and retry" }).click();
    await expect(page.getByRole("button", { name: "Create project", exact: true })).toBeEnabled();
    await expect(page.getByRole("button", { name: "Manage", exact: true })).toBeEnabled();
    await expect(page.getByRole("alert")).toHaveCount(0);
    expect(new URL(page.url()).pathname).toBe("/projects");
});

test("every M1-M7 official GUI route resolves through the typed client", async ({ page }) => {
    await openHarness(page, "/projects");
    const titleStyle = (element: Element) => {
        const style = getComputedStyle(element);
        return {
            fontSize: style.fontSize, fontWeight: style.fontWeight, lineHeight: style.lineHeight, color: style.color,
            margin: style.margin, flex: style.flex, whiteSpace: style.whiteSpace
        };
    };
    const headerGeometry = (element: Element) => {
        const header = element.closest("header")!;
        const style = getComputedStyle(header);
        const bounds = header.getBoundingClientRect();
        const canvas = document.getElementById("main-content")!.getBoundingClientRect();
        return {
            shared: header.classList.contains("page-header"),
            left: bounds.left - canvas.left, top: bounds.top - canvas.top,
            width: bounds.width, height: bounds.height,
            padding: style.padding, gap: style.gap, alignItems: style.alignItems, flexWrap: style.flexWrap
        };
    };
    const projectsTitleStyle = await page.getByRole("heading", { level: 1, name: "Projects", exact: true }).evaluate(titleStyle);
    const projectsHeaderGeometry = await page.getByRole("heading", { level: 1, name: "Projects", exact: true }).evaluate(headerGeometry);
    const projectsTitlePosition = await page.getByRole("heading", { level: 1, name: "Projects", exact: true }).evaluate((element) => {
        const bounds = element.getBoundingClientRect();
        const canvas = document.getElementById("main-content")!.getBoundingClientRect();
        return { left: bounds.left - canvas.left, top: bounds.top - canvas.top };
    });
    const sidebarRoutes = new Set(["/", "/projects", "/repositories", "/user-packages", "/templates", "/unity", "/operations", "/extensions", "/activity", "/diagnostics", "/settings", "/about"]);
    const routes = [
        ["/", "Projects"],
        ["/projects", "Projects"],
        ["/projects/00000000-0000-4000-8000-000000000101", "<private-project>"],
        ["/projects/00000000-0000-4000-8000-000000000101/packages", "<private-project>"],
        ["/projects/00000000-0000-4000-8000-000000000101/unity", "Project Unity"],
        ["/projects/00000000-0000-4000-8000-000000000101/backups", "Backups"],
        ["/repositories", "Resources"],
        ["/user-packages", "Resources"],
        ["/repositories/00000000-0000-4000-8000-000000000102", "Repository"],
        ["/templates", "Resources"],
        ["/templates/com.cqmhv.template.avatar", "Template detail"],
        ["/unity", "Settings"],
        ["/backups/00000000-0000-4000-8000-000000000104", "Backup detail"],
        ["/operations", "Tasks"],
        ["/operations/00000000-0000-4000-8000-000000000105", "Operation detail"],
        ["/extensions", "Extensions"],
        ["/extensions/com.cqmhv.discord", "Extension detail"],
        ["/extensions/com.cqmhv.discord/ui", "Extension UI"],
        ["/activity", "Logs"],
        ["/diagnostics", "Logs"],
        ["/settings", "Settings"],
        ["/about", "About"]
    ] as const;
    for (const [route, title] of routes) {
        await openHarness(page, route);
        const main = page.getByRole("main");
        const heading = main.getByRole("heading", { level: 1, name: title, exact: true });
        await expect(heading).toBeVisible();
        const refresh = main.locator(".projects-toolbar md-icon-button, .package-workspace-toolbar md-icon-button, .utility-workspace-toolbar md-icon-button, .extension-chrome md-icon-button")
            .filter({ has: page.locator('[data-icon-name="refresh"]') });
        await expect(refresh).toHaveCount(1);
        await expect(refresh.getByRole("button")).toHaveAccessibleName(/^(Refresh|Working)/);
        await expect(refresh).toHaveText("");
        if (sidebarRoutes.has(route)) {
            const sidebarLabel = page.locator('#primary-navigation .navigation-item[selected] .navigation-item-label');
            await expect(sidebarLabel).toHaveCount(1);
            await expect(heading).toHaveText(await sidebarLabel.innerText());
        }
        expect(await heading.evaluate(titleStyle)).toEqual(projectsTitleStyle);
        expect(await heading.evaluate(headerGeometry), route).toEqual(projectsHeaderGeometry);
        // Contextual detail headers may lead with Back or carry a project path.
        // Sidebar destinations must place the heading at exactly the Projects position.
        if (sidebarRoutes.has(route)) {
            expect(await heading.evaluate((element) => {
                const bounds = element.getBoundingClientRect();
                const canvas = document.getElementById("main-content")!.getBoundingClientRect();
                return { left: bounds.left - canvas.left, top: bounds.top - canvas.top };
            }), route).toEqual(projectsTitlePosition);
        }
        // A clipped 1px heading is technically visible to Playwright, but not to the owner.
        expect(await heading.evaluate((element) => {
            const bounds = element.getBoundingClientRect();
            const atCenter = document.elementFromPoint(bounds.left + bounds.width / 2, bounds.top + bounds.height / 2);
            return bounds.width > 20 && bounds.height > 20 && atCenter !== null && element.contains(atCenter);
        })).toBe(true);
        await expect(main.locator('.route-state[role="alert"]')).toHaveCount(0);
    }
});

test("project workspace keeps package discovery and user actions in project context", async ({ page }) => {
    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101");
    await expect(page.getByRole("heading", { level: 1, name: "<private-project>" })).toBeVisible();
    await expect(page.getByRole("heading", { level: 2, name: "Packages" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: "Open Unity" })).toBeVisible();
    await page.getByRole("button", { name: "More actions for <private-project>", exact: true }).click();
    await expect(page.getByRole("menuitem", { name: "Backups", exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("button", { name: "More actions for <private-project>", exact: true })).toBeFocused();
    const row = page.getByRole("row").filter({ hasText: "Avatar tools" });
    await expect(row).toContainText("com.example.avatar");
    await expect(row).toContainText("1.2.3");
    await expect(row).toContainText("Example packages");
    const search = page.getByRole("searchbox", { name: "Search packages", exact: true });
    await search.fill("not present");
    await expect(search).toHaveValue("not present");
    await expect(page.getByRole("status").filter({ hasText: "No matching packages" })).toBeVisible();
    await search.fill("");
    await row.getByRole("button", { name: "Update" }).click();
    const dialog = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(dialog).toContainText("com.example.avatar");
    await expect(dialog).not.toContainText(/plan|revision|fingerprint/i);
});

test("settings are labeled, revisioned, and ordinary preferences save through the typed client", async ({ page }) => {
    await openHarness(page, "/settings");
    await page.locator("#settings-theme-button").click();
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeVisible();
    const color = page.locator("#theme-source").getByRole("textbox");
    await expect(color).toHaveAccessibleName("Source color");
    await expect(color).toHaveAccessibleDescription("");
    await color.fill("#315DA8");
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#315DA8");
    expect(await page.evaluate(() => window.settingsRequests)).toEqual([]);
    await page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Theme", exact: true }) }).getByRole("button", { name: "Save", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Theme", exact: true })).toBeHidden();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await page.getByRole("checkbox", { name: "Show prerelease package versions" }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(2);
    await page.getByRole("checkbox", { name: "Hide local User Packages" }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(3);
    await page.getByRole("checkbox", { name: "Hide Example packages" }).check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(4);
    await selectMaterialOption(page.locator("md-filled-select#settings-locale"), "zh-CN");
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await navigationItem(page, "项目").click();
    await expect(page.getByRole("heading", { level: 1, name: "项目" })).toBeVisible();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await navigationItem(page, "设置").click();
    await expect(page.getByRole("checkbox", { name: "显示预发布软件包" })).toBeChecked();
    await expect(page.getByRole("checkbox", { name: "隐藏本地用户软件包" })).toBeChecked();
    await expect(page.getByRole("checkbox", { name: "隐藏 Example packages" })).toBeChecked();
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([
        { expectedRevision: 7, revision: 8 }, { expectedRevision: 8, revision: 9 },
        { expectedRevision: 9, revision: 10 }, { expectedRevision: 10, revision: 11 }, { expectedRevision: 11, revision: 12 }
    ]);
    await expect(page.locator("html")).toHaveAttribute("data-source-color", "#315DA8");
});

test("package changes use a v3-style confirmation while the durable Plan stays internal", async ({ page }) => {
    await openHarness(page, `/projects/00000000-0000-4000-8000-000000000101/packages`);
    await page.getByRole("row").filter({ hasText: "Avatar tools" }).getByRole("button", { name: "Update" }).click();
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
    await page.getByRole("row").filter({ hasText: "Avatar tools" }).getByRole("button", { name: "Update" }).click();
    await page.getByRole("button", { name: "Apply changes" }).click();
    await expect(page.getByRole("alert")).toContainText("This project changed");
    await expect(page.getByRole("alert")).not.toContainText("plan_stale");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toHaveCount(0);
});

test("direct writes and the remaining high-impact workflows retain confirmation and Operation boundaries", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Add existing project", exact: true }).click();
    await expect(page.locator(".project-register-review code")).toHaveText("C:\\Fixture\\Avatar");
    await page.getByRole("button", { name: "Confirm" }).click();
    await expect(page.getByRole("status").filter({ hasText: "Project registered" })).toBeVisible();

    await openHarness(page, "/repositories");
    await page.getByRole("button", { name: "Add repository", exact: true }).click();
    await page.getByLabel("Repository URL").fill("https://packages.example.invalid/index.json");
    await page.getByRole("button", { name: "Review repository" }).click();
    await expect(page.getByRole("dialog", { name: "Add repository", exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Add repository", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Repository added." })).toBeVisible();

    await openHarness(page, "/templates");
    await page.getByRole("button", { name: "More template creation options", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import template", exact: true }).click();
    await page.getByLabel("Template bundle").fill("C:\\Fixture\\avatar.alcomdtemplate");
    await page.getByRole("button", { name: "Review import" }).click();
    await clickDialogAction(page, "Import template", "Apply reviewed plan");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101/backups");
    await page.getByRole("button", { name: "Create backup", exact: true }).click();
    await clickDialogAction(page, "Create backup", "Create backup");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/backups/00000000-0000-4000-8000-000000000104");
    await page.getByRole("button", { name: "Restore backup", exact: true }).click();
    await page.getByLabel("Target parent").fill("C:\\Fixture");
    await page.getByLabel("New directory name").fill("Restored");
    await page.getByRole("button", { name: "Review backup restore" }).click();
    await clickDialogAction(page, "Restore backup", "Apply reviewed plan");
    await expect(page.getByRole("status").filter({ hasText: "Operation" })).toBeVisible();

    await openHarness(page, "/extensions");
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await page.getByLabel("Extension package").fill("C:\\Fixture\\extension.alcomdext");
    await page.getByLabel("Expected registry revision", { exact: true }).fill("0");
    await page.getByRole("button", { name: "Create install plan" }).click();
    await clickDialogAction(page, "Install extension", "Apply reviewed plan");
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
    await expect(page.locator("md-filled-select#presence-mode")).toHaveJSProperty("value", "project");
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
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Add existing project", exact: true }).click();
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
    await page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: title, exact: true }) }).getByRole("button", { name: action, exact: true }).click();
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
