import { expect, test, type Page } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

// Every production DesktopList caller, including the shared UtilityTable callers.
const listRoutes = [
    ["Projects", "/projects", "ready"],
    ["Packages", "/projects/00000000-0000-4000-8000-000000000101/packages", "package-multiple"],
    ["Repositories", "/repositories", "ready"],
    ["Repository packages", "/repositories/00000000-0000-4000-8000-000000000102", "ready"],
    ["User Packages", "/user-packages", "package-user-source"],
    ["Templates", "/templates", "ready"],
    ["Unity installations", "/unity", "ready"],
    ["Unity installations", "/settings", "ready"],
    ["Backups", "/projects/00000000-0000-4000-8000-000000000101/backups", "ready"],
    ["Tasks", "/operations", "ready"],
    ["Activity", "/activity", "ready"],
    ["Diagnostics", "/diagnostics", "ready"]
] as const;

for (const width of [1280, 1440]) {
    test(`all shared list pages keep long text and controls on their existing lines at ${width}px`, async ({ page }) => {
        test.setTimeout(60_000);
        await page.setViewportSize({ width, height: 900 });
        for (const [label, route, state] of listRoutes) {
            await test.step(`${label} ${route}`, async () => {
                await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
                await page.mouse.move(0, 0);
                const table = page.getByRole("table", { name: label, exact: true });
                await expect(table.locator("tbody tr").first()).toBeVisible();
                const viewport = table.locator("..");
                await viewport.scrollIntoViewIfNeeded();
                for (const corner of ["top-left", "top-right", "bottom-left", "bottom-right"]) {
                    await expect(viewport).toHaveCSS(`border-${corner}-radius`, "28px");
                }
                expect(await viewport.evaluate((element) => {
                    const box = element.getBoundingClientRect();
                    const atCorner = document.elementFromPoint(box.left + 1, box.top + 1);
                    const atHeader = document.elementFromPoint(box.left + 24, box.top + 3);
                    return atCorner !== null && !element.contains(atCorner) && atHeader !== null && element.contains(atHeader);
                })).toBe(true);
                await page.evaluate(() => document.fonts.ready);
                const cards = await table.evaluate((element) => {
                    const rows = Array.from(element.querySelectorAll("tr"));
                    const cells = Array.from(element.querySelectorAll("th, td"));
                    return {
                        rowGaps: rows.slice(1).map((row, index) => row.getBoundingClientRect().top - rows[index]!.getBoundingClientRect().bottom),
                        backgrounds: cells.map((cell) => getComputedStyle(cell).backgroundColor),
                        borders: cells.map((cell) => getComputedStyle(cell).borderBottomWidth),
                        headers: Array.from(element.querySelectorAll("thead th")).map((cell) => {
                            const style = getComputedStyle(cell);
                            const label = cell.querySelector("md-text-button");
                            const labelStyle = label === null ? style : getComputedStyle(label);
                            return {
                                height: cell.getBoundingClientRect().height,
                                fontSize: style.fontSize, lineHeight: style.lineHeight, fontWeight: style.fontWeight,
                                labelSize: labelStyle.fontSize, labelLineHeight: labelStyle.lineHeight, labelWeight: labelStyle.fontWeight
                            };
                        }),
                        corners: rows.map((row) => [
                            getComputedStyle(row.firstElementChild!).borderTopLeftRadius,
                            getComputedStyle(row.firstElementChild!).borderBottomLeftRadius,
                            getComputedStyle(row.lastElementChild!).borderTopRightRadius,
                            getComputedStyle(row.lastElementChild!).borderBottomRightRadius
                        ])
                    };
                });
                expect(cards.rowGaps.every((gap) => Math.abs(gap - 2) <= 1)).toBe(true);
                expect(new Set(cards.backgrounds).size).toBe(1);
                expect(cards.borders.every((border) => border === "0px")).toBe(true);
                // Selection controls retain their existing 48px target; plain headers are 40px.
                expect(new Set(cards.headers.map((header) => header.height)).size).toBe(1);
                expect(cards.headers[0]!.height).toBeGreaterThanOrEqual(40);
                expect(cards.headers.map(({ height: _height, ...typography }) => typography)).toEqual(cards.headers.map(() => ({
                    fontSize: "14px", lineHeight: "20px", fontWeight: "500",
                    labelSize: "14px", labelLineHeight: "20px", labelWeight: "500"
                })));
                expect(cards.corners).toEqual(cards.corners.map((_, index) => [
                    index === 0 ? "28px" : "8px",
                    index === cards.corners.length - 1 ? "28px" : "8px",
                    index === 0 ? "28px" : "8px",
                    index === cards.corners.length - 1 ? "28px" : "8px"
                ]));
                const result = await table.evaluate((element) => {
                    const rows = Array.from(element.querySelectorAll("tbody tr"));
                    const heights = rows.map((row) => row.getBoundingClientRect().height);
                    const probeNodes: Text[] = [];
                    for (const cell of element.querySelectorAll("tbody td")) {
                        const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
                        while (walker.nextNode()) {
                            const node = walker.currentNode as Text;
                            const parent = node.parentElement;
                            if (!node.textContent?.trim() || parent === null || parent.closest("button, input, select, md-menu, md-text-button, md-filled-button, md-filled-tonal-button, md-outlined-button, md-icon-button, md-checkbox, md-icon, .material-toggle, .visually-hidden") !== null) continue;
                            node.textContent += " Long label 中文路径 C:/Projects/very-long-folder/package-without-breaks\n".repeat(8);
                            probeNodes.push(node);
                            break;
                        }
                    }
                    const wrappedText = probeNodes.filter((node) => {
                        const range = document.createRange();
                        range.selectNodeContents(node);
                        return new Set(Array.from(range.getClientRects(), (rect) => Math.round(rect.top))).size > 1;
                    }).length;
                    const wrappingContainers = Array.from(element.querySelectorAll("th, th *, tbody td, tbody td *")).filter((node) => {
                        if (node.closest("md-menu, svg") !== null) return false;
                        const style = getComputedStyle(node);
                        return style.whiteSpace !== "nowrap" || ((style.display === "flex" || style.display === "inline-flex") && style.flexWrap !== "nowrap");
                    }).map((node) => node.tagName + "." + node.className);
                    const clippedControls = Array.from(element.querySelectorAll("tbody :is(md-text-button, md-filled-button, md-filled-tonal-button, md-outlined-button, md-icon-button, md-checkbox)")).filter((control) => {
                        if (control.closest("md-menu") !== null || control.getClientRects().length === 0) return false;
                        const cell = control.closest("td")!;
                        const controlBox = control.getBoundingClientRect();
                        const cellBox = cell.getBoundingClientRect();
                        return controlBox.left < cellBox.left - 1 || controlBox.right > cellBox.right + 1;
                    }).map((control) => control.getAttribute("aria-label") ?? control.textContent?.trim());
                    return {
                        probes: probeNodes.length,
                        wrappedText,
                        wrappingContainers,
                        clippedControls,
                        rowHeightChanges: rows.map((row, index) => Math.abs(row.getBoundingClientRect().height - heights[index]!)),
                        pageOverflows: document.documentElement.scrollWidth > window.innerWidth,
                        defaults: Array.from(element.querySelectorAll("tbody td"), (cell) => cell.getAttribute("data-cell-flow"))
                    };
                });
                expect(result.probes).toBeGreaterThan(0);
                expect(result.defaults.every((flow) => flow === "truncate")).toBe(true);
                expect(result.wrappedText).toBe(0);
                expect(result.wrappingContainers).toEqual([]);
                expect(result.clippedControls).toEqual([]);
                expect(result.rowHeightChanges.every((change) => change <= 1)).toBe(true);
                expect(result.pageOverflows).toBe(false);
            });
        }
    });
}

test("card surfaces share one 28px shape across workspaces", async ({ page }) => {
    test.setTimeout(60_000);
    const filterBarStyle = () => page.locator(".alcomd-filter-bar").evaluate((element) => {
        const style = getComputedStyle(element);
        const select = element.querySelector("md-filled-select")!;
        const selectedText = select.shadowRoot!.querySelector("#label")!;
        const status = element.querySelector(".alcomd-filter-bar-status")!;
        return {
            height: element.getBoundingClientRect().height,
            gap: style.gap,
            padding: style.padding,
            radius: style.borderRadius,
            background: style.backgroundColor,
            font: style.fontSize,
            selectWidth: select.getBoundingClientRect().width,
            selectHeight: select.getBoundingClientRect().height,
            selectFont: getComputedStyle(selectedText).fontSize,
            statusFont: getComputedStyle(status).fontSize
        };
    });
    await openHarness(page, "/projects");
    await expect(page.locator(".main-content")).toHaveCSS("border-radius", "28px");
    await expect(page.locator(".projects-content")).toHaveCSS("border-radius", "28px");
    await page.getByRole("button", { name: "List view", exact: true }).click();
    await expect(page.locator(".projects-secondary-toolbar")).toHaveCSS("border-radius", "28px");
    await expect(page.locator(".project-card").first()).toHaveCSS("border-radius", "28px");
    const cases = [
        ["/settings", ".settings-section"],
        ["/extensions", ".extension-management, .extension-card"],
        ["/about", ".notice-card"],
        ["/activity", ".logs-filter-toolbar"],
        ["/diagnostics", ".logs-filter-toolbar"],
        ["/extensions/com.cqmhv.discord/ui", ".extension-identity > div, .portable-section, .portable-group"],
        ["/extensions/com.cqmhv.mcp-management/ui", ".portable-list-item"],
        ["/projects/00000000-0000-4000-8000-000000000101/packages", ".package-workspace-surface"]
    ] as const;
    for (const mode of ["light", "dark"]) {
        await openHarness(page, "/projects");
        if (await page.getByRole("button", { name: "List view", exact: true }).isVisible()) {
            await page.getByRole("button", { name: "List view", exact: true }).click();
        }
        await page.evaluate((value) => { document.documentElement.dataset.appearance = value; }, mode);
        const sharedFilterStyle = await filterBarStyle();
        expect(sharedFilterStyle).toMatchObject({ height: 56, gap: "8px", padding: "8px 12px", font: "14px", selectWidth: 200, selectHeight: 40, selectFont: "14px", statusFont: "14px" });
        for (const [route, selector] of cases) {
            await openHarness(page, route);
            await page.evaluate((value) => { document.documentElement.dataset.appearance = value; }, mode);
            await expect(page.locator(".main-content")).toHaveCSS("border-radius", "28px");
            for (const part of selector.split(", ")) {
                const surfaces = page.locator(part);
                await expect(surfaces.first()).toBeVisible();
                for (const surface of await surfaces.all()) await expect(surface).toHaveCSS("border-radius", "28px");
            }
            if (route === "/activity" || route === "/diagnostics") expect(await filterBarStyle()).toEqual(sharedFilterStyle);
            if (route === "/about") {
                // These are transparent definition rows with divider lines, not card surfaces.
                for (const row of await page.locator(".detail-grid > div").all()) {
                    await expect(row).toHaveCSS("border-radius", "0px");
                    await expect(row).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
                }
            }
        }
    }
    await page.goto("/browser-harness.html?route=%2Foperations&state=empty");
    await expect(page.locator(".route-state")).toHaveCSS("border-radius", "28px");
});

test("Resources sections and detail return controls work through ordinary clicks", async ({ page }) => {
    await openHarness(page, "/projects");
    await navigationItem(page, "Resources").click();
    await expectHeading(page, "Resources");
    const resources = page.getByRole("navigation", { name: "Resources", exact: true });
    await expect(resources.getByRole("button", { name: "Repositories", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Repositories", exact: true }).getByRole("columnheader")).toHaveText(["Repository", "Source", "Actions"]);
    const repositoryActions = page.getByRole("table", { name: "Repositories", exact: true }).locator("md-icon-button");
    for (const name of ["package_2", "refresh", "close"]) {
        await expect(repositoryActions.filter({ has: page.locator(`[data-icon-name="${name}"]`) })).toHaveCount(1);
    }
    await page.getByRole("button", { name: "Browse packages" }).click();
    const repositoryDialog = page.getByRole("dialog", { name: "Example packages", exact: true });
    const repositoryHost = dialogHost(page, "Example packages");
    await expect(repositoryDialog).toBeVisible();
    await expect(repositoryHost.getByRole("table", { name: "Repository packages" }).getByRole("columnheader")).toHaveText(["Package", "Version", "Status"]);
    for (const header of await repositoryHost.getByRole("columnheader").all()) {
        await expect(header).toHaveCSS("font-size", "14px");
        await expect(header).toHaveCSS("line-height", "20px");
        await expect(header).toHaveCSS("font-weight", "500");
        expect((await header.boundingBox())!.height).toBe(40);
    }
    const modalBounds = await repositoryDialog.boundingBox();
    const firstColumn = await repositoryHost.getByRole("columnheader", { name: "Package", exact: true }).boundingBox();
    const closeBounds = await repositoryHost.getByRole("button", { name: "Close", exact: true }).boundingBox();
    expect(modalBounds).not.toBeNull();
    expect(firstColumn).not.toBeNull();
    expect(closeBounds).not.toBeNull();
    expect(firstColumn!.x).toBeGreaterThanOrEqual(modalBounds!.x);
    expect(closeBounds!.x + closeBounds!.width).toBeLessThanOrEqual(modalBounds!.x + modalBounds!.width);
    await page.screenshot({ path: "../../target/m7-v3-recheck-20260919/browser/repository-packages-dialog.png", fullPage: false, animations: "disabled" });
    await repositoryHost.getByRole("button", { name: "Close", exact: true }).click();
    await expect(repositoryDialog).not.toBeVisible();
    await expectHeading(page, "Resources");
    await resources.getByRole("button", { name: "User Packages", exact: true }).click();
    await expectHeading(page, "Resources");
    await expect(resources.getByRole("button", { name: "User Packages", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("button", { name: "Enroll folder" })).toBeVisible();
    await resources.getByRole("button", { name: "Templates", exact: true }).click();
    await expectHeading(page, "Resources");
    await expect(resources.getByRole("button", { name: "Templates", exact: true })).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("table", { name: "Templates" }).getByRole("columnheader")).toHaveText(["Template", "ID", "Last modified", "Source", "Actions"]);
    await page.getByRole("button", { name: /^More actions for / }).click();
    await page.getByRole("menuitem", { name: "View template", exact: true }).click();
    await expectHeading(page, "Template detail");
    await expect(page.getByRole("button", { name: "Favorite", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Remove template", exact: true })).toHaveCount(0);
    await page.getByRole("button", { name: "Templates", exact: true }).click();
    await expectHeading(page, "Resources");
    await resources.getByRole("button", { name: "Repositories", exact: true }).click();
    await expectHeading(page, "Resources");
});

test("Settings follows supported v3 group order while Logs retains its two real read surfaces", async ({ page }) => {
    await openHarness(page, "/projects");
    await navigationItem(page, "Settings").click();
    await expectHeading(page, "Settings");
    await expect(page.getByRole("navigation", { name: "Settings sections" })).toHaveCount(0);
    await expect(page.locator(".settings-section-heading h2")).toHaveText(["Unity Hub Path", "Unity installations", "Packages", "Appearance", "System information"]);
    await expect(page.getByRole("region", { name: "Appearance", exact: true }).getByRole("button", { name: "Theme", exact: true })).toBeVisible();
    await expect(page.getByRole("table", { name: "Unity installations" }).getByRole("columnheader")).toHaveText(["Version", "Path", "Source"]);
    await page.getByRole("button", { name: "Manage installations", exact: true }).click();
    const unityDialog = page.getByRole("dialog", { name: "Manage Unity installations" });
    await expect(unityDialog).toBeVisible();
    await expect(dialogHost(page, "Manage Unity installations").getByLabel("Unity executable", { exact: true })).toBeVisible();
    await dialogHost(page, "Manage Unity installations").getByRole("button", { name: "Cancel", exact: true }).click();
    await navigationItem(page, "Logs").click();
    await expectHeading(page, "Logs");
    const logs = page.getByRole("navigation", { name: "Logs", exact: true });
    const activity = page.getByRole("table", { name: "Activity", exact: true });
    await expect(activity.getByRole("columnheader")).toHaveText(["Time", "Status", "Activity", "Target", "Actions"]);
    await expect(activity).toHaveCSS("table-layout", "fixed");
    await expect(activity.locator("tbody td[data-cell-flow=\"truncate\"]").first()).toHaveCSS("white-space", "nowrap");
    await expect(activity.locator(".logs-summary small")).toHaveCSS("white-space", "nowrap");
    await logs.getByRole("button", { name: "Diagnostics" }).click();
    await expectHeading(page, "Logs");
    await expect(logs.getByRole("button", { name: "Diagnostics", exact: true })).toHaveAttribute("aria-pressed", "true");
    const diagnostics = page.getByRole("table", { name: "Diagnostics", exact: true });
    await expect(diagnostics.getByRole("columnheader")).toHaveText(["Time", "Severity", "Subsystem", "Diagnostic", "Actions"]);
    await expect(diagnostics).toHaveCSS("table-layout", "fixed");
    await expect(diagnostics.locator(".logs-summary p")).toHaveCSS("white-space", "nowrap");
    await logs.getByRole("button", { name: "Activity", exact: true }).click();
    await expectHeading(page, "Logs");
});

test("utility actions use modal dialogs and cancel clears drafts and returns focus", async ({ page }) => {
    const cases = [
        ["/repositories", "Add repository", "Repository URL", "https://example.invalid/preserved.json"],
        ["/templates", "Import template", "Template bundle", "C:\\Fixture\\preserved.alcomdtemplate"],
        ["/extensions", "Install extension", "Extension package", "C:\\Fixture\\preserved.alcomdext"],
        ["/unity", "Manage installations", "Unity executable", "C:\\Fixture\\Unity.exe"]
    ] as const;
    for (const [route, title, field, value] of cases) {
        await openHarness(page, route);
        const control = page.getByLabel(field, { exact: true });
        await expect(control).toHaveCount(0);
        const trigger = page.getByRole("button", { name: route === "/templates" ? "More template creation options" : title, exact: true });
        const openAction = async () => {
            await trigger.click();
            if (route === "/templates") await page.getByRole("menuitem", { name: title, exact: true }).click();
        };
        await openAction();
        const dialog = page.getByRole("dialog", { name: title, exact: true });
        await expect(dialog).toBeVisible();
        await expect(dialogHost(page, title).getByLabel(field, { exact: true })).toBeVisible();
        await expect(dialog).toHaveCSS("border-radius", "28px");
        await control.fill(value);
        await page.screenshot({ path: `../../target/m7-v3-recheck-20260919/browser/${route.slice(1)}-action-dialog.png`, fullPage: false, animations: "disabled" });
        await dialogHost(page, title).getByRole("button", { name: "Cancel", exact: true }).click();
        await expect(dialog).not.toBeVisible();
        await expect(control).toHaveCount(0);
        await expect(trigger).toBeFocused();
        await openAction();
        await expect(control).toHaveValue("");
        await page.keyboard.press("Escape");
        await expect(dialog).not.toBeVisible();
        await expect(trigger).toBeFocused();
    }
});

test("desktop Settings uses labeled language and animation rows with Theme in its header and a compact control", async ({ page }) => {
    await openHarness(page, "/settings");
    await expectHeading(page, "Settings");
    const fields = page.locator(".settings-editor md-filled-select");
    await expect(fields).toHaveCount(2);
    await expect(page.getByRole("region", { name: "Appearance", exact: true }).getByRole("button", { name: "Theme", exact: true })).toBeVisible();
    await expect(page.getByRole("combobox", { name: "GUI animation", exact: true })).toBeVisible();
    await expect(page.getByRole("checkbox", { name: "Compact GUI", exact: true })).toBeVisible();
    await expect(page.locator("#settings-density")).toHaveCount(0);
    const bounds = await fields.evaluateAll((elements) => elements.map((element) => {
        const rect = element.getBoundingClientRect();
        return { id: element.id, left: rect.left, right: rect.right, top: rect.top, bottom: rect.bottom };
    }));
    for (let index = 0; index < bounds.length; index++) {
        const current = bounds[index]!;
        expect(current.left, current.id).toBeGreaterThanOrEqual(260);
        expect(current.right, current.id).toBeLessThanOrEqual(1440);
        for (const other of bounds.slice(index + 1)) {
            const overlap = Math.min(current.right, other.right) > Math.max(current.left, other.left)
                && Math.min(current.bottom, other.bottom) > Math.max(current.top, other.top);
            expect(overlap, `${current.id} must not overlap ${other.id}`).toBe(false);
        }
    }
    const appearanceRows = page.getByRole("region", { name: "Appearance", exact: true }).locator(".settings-section-content > div");
    await expect(appearanceRows).toHaveCount(3);
    const rows = await appearanceRows.evaluateAll((elements) => elements.map((element) => {
        const rect = element.getBoundingClientRect();
        return { top: rect.top, bottom: rect.bottom };
    }));
    expect(rows[0]!.bottom).toBeLessThanOrEqual(rows[1]!.top);
    expect(rows[1]!.bottom).toBeLessThanOrEqual(rows[2]!.top);
});

test("ordinary Settings save before refresh and remain accepted across navigation", async ({ page }) => {
    await openHarness(page, "/settings");
    const compact = page.getByRole("checkbox", { name: "Compact GUI", exact: true });
    await expect(compact).not.toBeChecked();
    await compact.check();
    await expect.poll(() => page.evaluate(() => window.settingsWrites.length)).toBe(1);
    await expect(page.locator("html")).toHaveAttribute("data-density", "compact");
    const refresh = page.locator(".utility-workspace-toolbar").getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeEnabled();
    await refresh.click();
    await expect(compact).toBeChecked();
    await navigationItem(page, "Projects").click();
    await expectHeading(page, "Projects");
    await expect(page.getByRole("dialog", { name: "Discard unsaved changes?", exact: true })).toHaveCount(0);
    await navigationItem(page, "Settings").click();
    await expect(compact).toBeChecked();
    expect(await page.evaluate(() => window.settingsWrites)).toEqual([{ expectedRevision: 7, revision: 8 }]);
});

test("utility desktop evidence includes the full application window", async ({ page }) => {
    const directory = "../../target/m7-v3-recheck-20260919/browser";
    const routes = [
        ["project-packages", "/projects/00000000-0000-4000-8000-000000000101/packages", "<private-project>"],
        ["project-unity", "/projects/00000000-0000-4000-8000-000000000101/unity", "Project Unity"],
        ["repositories", "/repositories", "Resources"],
        ["repository", "/repositories/00000000-0000-4000-8000-000000000102", "Repository"],
        ["user-packages", "/user-packages", "Resources"],
        ["templates", "/templates", "Resources"],
        ["template", "/templates/com.cqmhv.template.avatar", "Template detail"],
        ["settings", "/settings", "Settings"],
        ["unity", "/unity", "Settings"],
        ["backups", "/projects/00000000-0000-4000-8000-000000000101/backups", "Backups"],
        ["backup", "/backups/00000000-0000-4000-8000-000000000104", "Backup detail"],
        ["tasks", "/operations", "Tasks"],
        ["task", "/operations/00000000-0000-4000-8000-000000000105", "Operation detail"],
        ["extensions", "/extensions", "Extensions"],
        ["extension", "/extensions/com.cqmhv.discord", "Extension detail"],
        ["extension-ui", "/extensions/com.cqmhv.discord/ui", "Extension UI"],
        ["activity", "/activity", "Logs"],
        ["diagnostics", "/diagnostics", "Logs"],
        ["about", "/about", "About"]
    ] as const;
    for (const [name, route, heading] of routes) {
        await openHarness(page, route);
        await expectHeading(page, heading);
        await expect(page.getByRole("main").locator(".loading-state")).toHaveCount(0);
        await expect(page.getByRole("main").locator(".route-state--error")).toHaveCount(0);
        await page.evaluate(() => document.fonts.ready);
        await page.screenshot({ path: `${directory}/${name}.png`, fullPage: false, animations: "disabled" });
    }
    await page.goto("/browser-harness.html?route=%2Fuser-packages&state=package-user-source");
    await expectHeading(page, "Resources");
    const userPackages = page.getByRole("table", { name: "User Packages", exact: true });
    await expect(userPackages.getByRole("row").filter({ hasText: "Local avatar tools" })).toBeVisible();
    await expect(userPackages).toHaveAttribute("data-sizing", "content");
    await expect(userPackages).toHaveCSS("table-layout", "auto");
    const removeEnrollment = page.getByRole("button", { name: "Remove enrollment", exact: true });
    await expect(removeEnrollment).toBeVisible();
    await expect(userPackages.locator('md-icon-button').filter({ has: page.locator('[data-icon-name="close"]') })).toHaveCount(1);
    await expect(userPackages.locator('md-icon-button').filter({ has: page.locator('[data-icon-name="refresh"]') })).toHaveCount(1);
    const [tableBox, removeBox] = await Promise.all([userPackages.boundingBox(), removeEnrollment.boundingBox()]);
    expect(tableBox).not.toBeNull();
    expect(removeBox).not.toBeNull();
    expect((removeBox?.x ?? 0) + (removeBox?.width ?? 0)).toBeLessThanOrEqual((tableBox?.x ?? 0) + (tableBox?.width ?? 0));
    await page.screenshot({ path: `${directory}/user-packages-populated.png`, fullPage: false, animations: "disabled" });
});

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
}

function dialogHost(page: Page, title: string) {
    // The accessible native dialog lives inside the Material host's shadow root;
    // form controls are slotted light DOM and must be located from that host.
    return page.locator("md-dialog").filter({ has: page.getByRole("heading", { name: title, exact: true }) });
}

async function expectHeading(page: Page, name: string) {
    await expect(page.getByRole("heading", { level: 1, name, exact: true })).toBeVisible();
}

function navigationItem(page: Page, name: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${name}$`) })
    });
}
