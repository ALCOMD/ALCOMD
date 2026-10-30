import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";

test("Projects grid keeps compact aligned cards with long names and scrollable rows", async ({ page }, testInfo) => {
    await page.addInitScript(() => { Date.now = () => 1_700_000_120_000; });
    for (const width of [1180, 1280, 1440, 1600]) {
        await page.setViewportSize({ width, height: width === 1180 ? 760 : 900 });
        await page.goto("/browser-harness.html?route=%2Fprojects&state=favorite-pages");
        await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
        if (await page.getByRole("button", { name: "Grid view", exact: true }).isVisible()) {
            await page.getByRole("button", { name: "Grid view", exact: true }).click();
        }
        const grid = page.locator(".projects-grid");
        await expect(grid.locator(".project-card")).toHaveCount(2);
        await page.screenshot({ path: testInfo.outputPath(`grid-sparse-${width}.png`), animations: "disabled" });
        const sparse = await grid.locator(".project-card").evaluateAll((cards) => cards.map((card) => card.getBoundingClientRect().height));
        expect(sparse.every((height) => height < 220)).toBe(true);

        await page.goto("/browser-harness.html?route=%2Fprojects&projectGrid=1");
        const cards = page.locator(".projects-grid .project-card");
        await expect(cards).toHaveCount(7);
        for (const button of await cards.first().locator(".project-row-actions--card > md-filled-button, .project-row-actions--card > md-filled-tonal-button").all()) {
            await expect(button).toHaveCSS("padding-inline-start", "24px");
            await expect(button).toHaveCSS("padding-inline-end", "24px");
        }
        for (const appearance of ["light", "dark"]) {
            await page.evaluate((mode) => { document.documentElement.dataset.appearance = mode; }, appearance);
            await page.evaluate(() => document.fonts.ready);
            const geometry = await cards.evaluateAll((elements) => elements.map((card) => {
                const bounds = card.getBoundingClientRect();
                const footer = card.querySelector(".project-card-actions")!.getBoundingClientRect();
                return {
                    x: bounds.x, y: bounds.y, height: bounds.height, width: bounds.width,
                    footer: footer.y - bounds.y,
                    controlsInside: Array.from(card.querySelectorAll("md-icon-button, md-filled-button, md-filled-tonal-button, md-text-button")).every((control) => {
                        const box = control.getBoundingClientRect();
                        const inset = control.tagName === "MD-ICON-BUTTON" ? 7 : 15;
                        return box.left >= bounds.left + inset && box.right <= bounds.right - inset && box.top >= bounds.top + inset && box.bottom <= bounds.bottom - inset;
                    }),
                    actionRows: new Set(Array.from(card.querySelectorAll(".project-row-actions--card > md-filled-button, .project-row-actions--card > md-filled-tonal-button")).map((control) => control.getBoundingClientRect().y)).size,
                    contentFits: card.scrollWidth <= card.clientWidth,
                    titleHeight: card.querySelector("h2")!.getBoundingClientRect().height,
                    pathHeight: card.querySelector(".project-path")!.getBoundingClientRect().height
                };
            }));
            expect(geometry.every((card) => card.height < 220 && card.controlsInside && card.contentFits && card.actionRows === 1 && card.titleHeight === 24 && card.pathHeight === 20)).toBe(true);
            expect(new Set(geometry.map((card) => card.height)).size).toBe(1);
            expect(new Set(geometry.map((card) => card.footer)).size).toBe(1);
            expect(geometry.filter((card) => card.y === geometry[0]!.y)).toHaveLength(width === 1600 ? 3 : 2);
            expect(await grid.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
            await expect(cards.first().getByRole("button", { name: "Remove from favorites", exact: true })).toBeVisible();
            await expect(cards.getByRole("button", { name: "Manage", exact: true })).toHaveCount(7);
            await expect(cards.getByRole("button", { name: "Backups", exact: true })).toHaveCount(7);
            await expect(cards.getByRole("button", { name: "Open Unity", exact: true })).toHaveCount(7);
            await expect(cards.getByRole("button", { name: /^More actions for/ })).toHaveCount(7);
            await page.screenshot({ path: testInfo.outputPath(`grid-${width}-${appearance}.png`), animations: "disabled" });
        }
        const overflows = await grid.evaluate((element) => element.scrollHeight > element.clientHeight);
        await cards.last().scrollIntoViewIfNeeded();
        if (overflows) expect(await grid.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
        else expect(await grid.evaluate((element) => element.scrollTop)).toBe(0);
        const last = cards.last();
        await last.getByRole("button", { name: "Backups", exact: true }).click();
        await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000107\/backups$/);
        await expect(page.getByRole("heading", { name: "Backups", exact: true })).toBeVisible();
    }
});

test("Projects grid retains shared launch, directory, copy review and removal cancellation", async ({ page }) => {
    await page.setViewportSize({ width: 1180, height: 760 });
    await page.goto("/browser-harness.html?route=%2Fprojects&projectGrid=1");
    await expect(page.getByRole("heading", { level: 1, name: "Projects", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Grid view", exact: true }).click();
    const card = page.getByRole("article", { name: "Avatar Studio", exact: true });
    await card.getByRole("button", { name: "Open Unity", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(page.getByText("Unity launch spawned.", { exact: true })).toBeVisible();
    const more = card.getByRole("button", { name: "More actions for Avatar Studio", exact: true });
    await more.click();
    await expect(card.getByRole("menuitem", { name: "Open Project Directory", exact: true })).toBeVisible();
    await expect(card.getByRole("menuitem", { name: "Copy Project", exact: true })).toBeVisible();
    await expect(card.getByRole("menuitem", { name: "Remove Project", exact: true })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(more).toBeFocused();
    await more.click();
    await card.getByRole("menuitem", { name: "Open Project Directory", exact: true }).click();
    await expect(page.getByText("Project directory opened.", { exact: true })).toBeVisible();
    await more.click();
    await card.getByRole("menuitem", { name: "Copy Project", exact: true }).click();
    const copy = page.locator("md-dialog[open]");
    await expect(page.getByRole("dialog", { name: "Copy project", exact: true })).toBeVisible();
    await copy.getByRole("button", { name: "Review copy", exact: true }).click();
    await expect(copy.getByRole("button", { name: "Start copy", exact: true })).toBeEnabled();
    await copy.getByRole("button", { name: "Back", exact: true }).click();
    await copy.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.locator("md-dialog[open]")).toHaveCount(0);
    await more.click();
    await card.getByRole("menuitem", { name: "Remove Project", exact: true }).click();
    const removal = page.getByRole("dialog", { name: "Remove this project?", exact: true });
    await expect(removal).toBeVisible();
    await expect(page.locator("md-dialog[open]").getByRole("button", { name: "Remove from list", exact: true })).toBeEnabled();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(removal).toBeHidden();
    await expect(card).toBeVisible();
    await card.getByRole("button", { name: "Manage", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(page).toHaveURL(new RegExp(`/projects/${PROJECT_ID}$`));
});

test("Projects grid keeps shared capability restrictions and missing-Editor handling", async ({ page }) => {
    await openHarness(page, "/projects", "capabilities-missing");
    await page.getByRole("button", { name: "Grid view", exact: true }).click();
    const card = page.locator(".project-card").first();
    await expect(card.getByRole("button", { name: "Open Unity", exact: true })).toBeDisabled();
    await expect(card.getByRole("button", { name: "Add to favorites", exact: true })).toBeDisabled();
    // This fixture retains backups.read.v1; the grid must match the table's enabled action.
    await expect(card.getByRole("button", { name: "Backups", exact: true })).toBeEnabled();
    await card.getByRole("button", { name: /^More actions for/ }).click();
    await expect(card.locator("md-menu-item").filter({ hasText: "Copy Project" })).toHaveJSProperty("disabled", true);
    await expect(card.locator("md-menu-item").filter({ hasText: "Remove Project" })).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await openHarness(page, "/projects", "unity-zero");
    await expect(page.locator(".project-card").first()).toBeVisible();
    await page.locator(".project-card").first().getByRole("button", { name: "Open Unity", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${PROJECT_ID}/unity$`));
    await expect(page.getByRole("heading", { name: "Project Unity", exact: true })).toBeVisible();
    await expect(page.getByText(/No exact matching Unity installation was found/).first()).toBeVisible();
});

test("Projects view survives navigation, reload and reopening the page in both modes", async ({ page, context }) => {
    await openHarness(page, "/projects");
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Grid view", exact: true }).click();
    await expect(page.locator(".projects-grid")).toBeVisible();
    await page.locator("#primary-navigation md-list-item").filter({ hasText: "Resources" }).click();
    await expect(page.getByRole("heading", { name: "Resources", exact: true })).toBeVisible();
    await page.locator("#primary-navigation md-list-item").filter({ hasText: "Projects" }).click();
    await expect(page.locator(".projects-grid")).toBeVisible();
    await reloadProjectsHarness(page);
    await expect(page.locator(".projects-grid")).toBeVisible();

    const reopened = await context.newPage();
    await openHarness(reopened, "/projects");
    await expect(reopened.locator(".projects-grid")).toBeVisible();
    await reopened.getByRole("button", { name: "List view", exact: true }).click();
    await expect(reopened.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await reloadProjectsHarness(reopened);
    await expect(reopened.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await reopened.close();
    await reloadProjectsHarness(page);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
});

test("Projects view remains usable with invalid or unavailable local storage", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.evaluate(() => localStorage.setItem("alcomd.gui.projects.view", "invalid"));
    await reloadProjectsHarness(page);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.addInitScript(() => {
        Object.defineProperty(window, "localStorage", { get() { throw new DOMException("Storage unavailable", "SecurityError"); } });
    });
    await reloadProjectsHarness(page);
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Grid view", exact: true }).click();
    await expect(page.locator(".projects-grid")).toBeVisible();
    await page.getByRole("button", { name: "List view", exact: true }).click();
    await expect(page.getByRole("table", { name: "Projects", exact: true })).toBeVisible();
});

test("Favorite loads every page and remains globally first after secondary sorting", async ({ page }) => {
    await openHarness(page, "/projects", "favorite-pages");
    const rows = page.getByRole("row");
    await expect(rows.nth(1)).toContainText("Zulu Favorite");
    await expect(rows.nth(2)).toContainText("Alpha");

    await page.getByRole("button", { name: "Project", exact: true }).click();
    await expect(rows.nth(1)).toContainText("Zulu Favorite");
    await page.getByRole("button", { name: "Grid view" }).click();
    await expect(page.locator(".project-card").first()).toContainText("Zulu Favorite");
});

test("Favorite mutation updates list and grid and exposes retryable errors", async ({ page }) => {
    await openHarness(page, "/projects");
    const toggle = page.getByRole("button", { name: "Add to favorites", exact: true });
    await toggle.click();
    await expect(page.getByRole("button", { name: "Remove from favorites" })).toBeVisible();

    await page.getByRole("button", { name: "Grid view" }).click();
    await expect(page.locator(".project-card").getByRole("button", { name: "Remove from favorites" })).toBeVisible();

    await openHarness(page, "/projects", "favorite-error");
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByText("Unable to update favorite: internal_error")).toBeVisible();

    await openHarness(page, "/projects", "favorite-conflict");
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByText("Unable to update favorite: revision_conflict")).toBeVisible();
    await page.getByRole("button", { name: "Add to favorites", exact: true }).click();
    await expect(page.getByRole("button", { name: "Remove from favorites" })).toBeVisible();
});

test("Unity launch arguments are independent from the one-shot installation choice", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity`);
    const open = page.getByRole("button", { name: "Edit launch arguments", exact: true });
    const dialog = page.getByRole("dialog", { name: "Unity launch arguments", exact: true });
    await expect(page.getByLabel("Additional arguments")).toBeHidden();
    await open.click();
    await expect(dialog).toBeVisible();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByRole("button", { name: "Clear launch arguments", exact: true }).click();
    await expect(dialog).toBeHidden();
    // Clearing closes the editor and refreshes the authoritative page. Wait for
    // that refresh before opening a new draft, which belongs to the new snapshot.
    await expect(page.getByRole("button", { name: "Refresh", exact: true })).toBeEnabled();
    await expect(page.getByText("Arguments", { exact: true }).locator("..")).toContainText("Default");
    await open.click();
    await expect(dialog).toBeVisible();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await expect(page.getByText("Exact installations").locator("..")).toContainText("1");
});

test("Unity launch arguments discard drafts on Cancel and Escape", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity`);
    const open = page.getByRole("button", { name: "Edit launch arguments", exact: true });
    const dialog = page.getByRole("dialog", { name: "Unity launch arguments", exact: true });
    await open.click();
    await expect(dialog).toBeVisible();
    await page.getByLabel("Additional arguments").fill("-draft-not-saved");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toBeHidden();
    await open.click();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByLabel("Additional arguments").fill("-second-draft");
    await page.keyboard.press("Escape");
    await expect(dialog).toBeHidden();
    await open.click();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
});

test("Unity exact launch handles zero, one, and multiple matching installations", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-automatic");
    await launch(page);
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();

    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-zero");
    await launch(page);
    await expect(page.getByText("No exact matching Unity installation was found.").first()).toBeVisible();

    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-multiple");
    const chooser = page.getByRole("combobox", { name: "Unity installation for this launch" });
    await expect(chooser).toBeVisible();
    await chooser.click();
    await page.getByRole("option", { name: "Unity 2022.3.22f1 · x86_64" }).nth(1).click();
    await launch(page);
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();
});

test("Project workspace Open Unity uses the exact 0/1/many flow", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/packages`);
    await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: "Open Unity" }).click();
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();

    await openHarness(page, `/projects/${PROJECT_ID}/packages`, "unity-zero");
    await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: "Open Unity" }).click();
    const missing = page.locator("md-dialog[open]");
    await expect(missing).toContainText("No exact matching Unity installation was found.");
    await missing.getByRole("button", { name: "Migrate Project…" }).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${PROJECT_ID}/unity\\?afterMigration=open$`));

    await openHarness(page, `/projects/${PROJECT_ID}/packages`, "unity-multiple");
    await page.getByRole("navigation", { name: "Project actions" }).getByRole("button", { name: "Open Unity" }).click();
    const chooser = page.locator("md-dialog[open]");
    await chooser.getByRole("combobox", { name: "Unity installation for this launch" }).click();
    await page.getByRole("option", { name: "Unity 2022.3.22f1 · x86_64" }).nth(1).click();
    await chooser.getByRole("button", { name: "Open Unity" }).click();
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();
});

test("Project workspace launch split menu retains launch arguments, backup navigation and focus", async ({ page }, testInfo) => {
    await openHarness(page, `/projects/${PROJECT_ID}/packages`);
    const group = page.getByRole("group", { name: "Open Unity", exact: true });
    const primary = group.getByRole("button", { name: "Open Unity", exact: true });
    const arrow = group.getByRole("button", { name: /^More actions for/ });
    const primaryBox = await group.locator(".alcomd-split-button-primary").boundingBox();
    const arrowBox = await group.locator(".alcomd-split-button-menu").boundingBox();
    expect(primaryBox).not.toBeNull(); expect(arrowBox).not.toBeNull();
    expect(arrowBox!.x - primaryBox!.x - primaryBox!.width).toBeCloseTo(2, 1);
    await expect(group.locator(".alcomd-split-button-primary")).toHaveCSS("border-start-end-radius", "8px");
    await expect(group.locator(".alcomd-split-button-menu")).toHaveCSS("border-start-start-radius", "8px");
    expect(arrowBox!.height).toBe(primaryBox!.height);
    await arrow.click();
    await expect(page.getByRole("menuitem")).toHaveCount(6);
    await expect(group.locator("md-menu .menu.open")).not.toHaveClass(/animating/);
    await page.screenshot({ path: testInfo.outputPath("project-launch-menu.png"), fullPage: false });
    await page.getByRole("menuitem", { name: "Edit launch arguments", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Unity launch arguments", exact: true })).toBeVisible();
    await expect(page.getByRole("menu")).toBeHidden();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByLabel("Additional arguments").fill("-discard-this-draft");
    await page.keyboard.press("Escape");
    await expect(arrow).toBeFocused();
    await arrow.click();
    await page.getByRole("menuitem", { name: "Edit launch arguments", exact: true }).click();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByLabel("Additional arguments").fill("-saved-from-workspace");
    await page.getByRole("button", { name: "Save launch arguments", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "Unity launch arguments updated." })).toBeVisible();
    await expect(arrow).toBeFocused();
    await arrow.click();
    await page.getByRole("menuitem", { name: "Edit launch arguments", exact: true }).click();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-saved-from-workspace");
    await page.getByRole("button", { name: "Cancel", exact: true }).click();
    await arrow.click();
    await page.getByRole("menuitem", { name: "Backups", exact: true }).click();
    await expect(page).toHaveURL(new RegExp(`/projects/${PROJECT_ID}/backups$`));
});

test("Project workspace split button keeps unavailable launch and maintenance actions disabled", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/packages`, "capabilities-missing");
    const group = page.getByRole("group", { name: "Open Unity", exact: true });
    await expect(group.getByRole("button", { name: "Open Unity", exact: true })).toBeDisabled();
    await group.getByRole("button", { name: /^More actions for/ }).click();
    await expect(page.locator("md-menu-item").filter({ hasText: "Edit launch arguments" })).toHaveJSProperty("disabled", true);
    await expect(page.locator("md-menu-item").filter({ hasText: "Copy Project" })).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await expect(group.getByRole("button", { name: /^More actions for/ })).toBeFocused();
});

test("Project workspace Unity version selector creates one reviewed migration Operation", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/packages`, "unity-migration");
    const selector = page.getByRole("navigation", { name: "Project actions" }).getByRole("combobox", { name: "Unity version" });
    await selector.click();
    await page.getByRole("option", { name: "Unity 2022.3.23f1" }).click();
    const review = page.locator("md-dialog[open]");
    await expect(review).toContainText("2022.3.22f1");
    await expect(review).toContainText("2022.3.23f1");
    await review.getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page).toHaveURL(new RegExp(`/operations/${OPERATION_ID}$`));
});

test("Open Unity migration composition revalidates options before the exact launch", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity?afterMigration=open`, "unity-migration");
    const selector = page.getByRole("combobox", { name: "Target Unity version" });
    await selector.click();
    await page.getByRole("option", { name: "2022.3.23f1", exact: true }).click();
    await page.getByRole("button", { name: "Review migration" }).click();
    const review = page.locator("md-dialog[open]");
    await review.getByRole("button", { name: "Apply reviewed plan" }).click();
    await expect(page.getByText("Migration completed. Unity launch spawned.")).toBeVisible({ timeout: 10_000 });
});

async function launch(page: Page) {
    await page.getByRole("button", { name: "Open Unity" }).click();
}

async function openHarness(page: Page, route: string, state = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}

async function reloadProjectsHarness(page: Page) {
    // The fixture replaces its URL with the app route; restore its entry before a real reload.
    await page.evaluate(() => history.replaceState(null, "", "/browser-harness.html?route=%2Fprojects&state=ready"));
    await page.reload();
}
