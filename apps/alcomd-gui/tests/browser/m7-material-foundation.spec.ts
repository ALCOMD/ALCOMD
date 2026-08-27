import { expect, test, type Page } from "@playwright/test";

test("shared Material foundation renders real controls with React 19 interaction semantics", async ({ page }) => {
    await page.goto("/browser-harness.html?material=1");
    const open = page.locator("md-filled-button").filter({ hasText: "Open dialog" });
    const disabled = page.locator("md-filled-tonal-button").filter({ hasText: "Disabled action" });
    await expect(open).toBeVisible();
    await expect(disabled).toHaveAttribute("disabled");
    await open.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByText("The dialog is hosted by the shared Material foundation.")).toBeVisible();
    await page.locator("md-text-button").filter({ hasText: "Close" }).click();
    await expect(page.getByText("The dialog is hosted by the shared Material foundation.")).toBeHidden();

    await expect(page.getByRole("button", { name: "Project evidence" })).toBeVisible();
    const iconButton = page.locator("md-icon-button").first();
    const icon = iconButton.locator(".alcomd-icon");
    await expect(icon).toHaveAttribute("aria-hidden", "true");
    await expect(icon).toHaveAttribute("data-filled", "false");
    const lightColors = await icon.evaluate((element) => {
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, foreground: style.color };
    });
    expect(lightColors.background).toBe(lightColors.foreground);
    expect(await icon.evaluate((element) => getComputedStyle(element).webkitMaskImage)).not.toBe("none");
    expect(await icon.evaluate((element) => getComputedStyle(element).webkitMaskSize)).toBe("100%");
    await expect(icon).toHaveAttribute("data-optical-size", "24");
    await page.locator("html").evaluate((element) => { element.dataset.appearance = "dark"; });
    const darkColors = await icon.evaluate((element) => {
        const style = getComputedStyle(element);
        return { background: style.backgroundColor, foreground: style.color };
    });
    expect(darkColors.background).toBe(darkColors.foreground);
    expect(darkColors.background).not.toBe(lightColors.background);
    await expect(page.locator("md-outlined-text-field")).toHaveJSProperty("label", "Project name");
    await expect(page.locator("md-outlined-select")).toHaveJSProperty("label", "Project type");
    await expect(page.locator("md-switch")).toHaveJSProperty("selected", true);
    await expect(page.locator("md-checkbox")).toHaveJSProperty("checked", false);
    await page.locator("md-checkbox").click();
    await expect(page.locator("md-checkbox")).toHaveJSProperty("checked", true);
    await expect(page.locator("md-linear-progress")).toHaveJSProperty("value", 0.62);
});

test("Core and Portable UI share Material controls and the same MD3 theme source", async ({ page }) => {
    await openHarness(page, "/projects");
    await expect(page.getByRole("main").getByRole("button", { name: "Refresh projects" })).toBeVisible();
    const corePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());

    await openHarness(page, "/extensions/com.cqmhv.mcp-management/ui");
    await expect(page.getByRole("region", { name: "MCP Management" }).locator("md-filled-tonal-button").filter({ hasText: "Refresh" })).toBeVisible();
    const portablePrimary = await page.locator("html").evaluate((element) => getComputedStyle(element).getPropertyValue("--md-sys-color-primary").trim());
    expect(portablePrimary).toBe(corePrimary);
});

test("v3-density Material navigation keeps one offline Rounded icon across selection state", async ({ page }) => {
    const remoteFontRequests: string[] = [];
    page.on("request", (request) => {
        if (/fonts\.(?:googleapis|gstatic)\.com/.test(request.url())) remoteFontRequests.push(request.url());
    });
    await openHarness(page, "/projects");

    const navigation = page.locator("#primary-navigation");
    await expect(navigation.locator("md-list")).toHaveCount(2);
    await expect(navigation.locator("md-list-item")).toHaveCount(7);
    await expect(navigation.locator(".navigation-item .alcomd-icon")).toHaveCount(7);
    await expect(navigationItem(page, "Projects")).toBeVisible();
    await expect(navigationItem(page, "Resources")).toBeVisible();
    await expect(navigation.locator("nav[aria-label='Utilities']")).toBeVisible();
    await expect(navigation.locator(".navigation-list--footer .navigation-item")).toHaveCount(3);
    await expect(navigation.locator(".navigation-list--footer .navigation-item-label").last()).toHaveText("About");
    await expect(navigation.locator(".navigation-list--footer .navigation-item-meta").last()).toHaveText("4.0.0-alpha.0");

    const projects = navigationItem(page, "Projects");
    const packages = navigationItem(page, "Resources");
    await expect(projects.locator(".alcomd-icon")).toHaveAttribute("aria-hidden", "true");
    await expect(projects.locator(".alcomd-icon")).toHaveAttribute("data-filled", "false");
    await expect(packages.locator(".alcomd-icon")).toHaveAttribute("data-filled", "false");
    const selectedIconUrl = await projects.locator(".alcomd-icon").evaluate((element) => getComputedStyle(element).getPropertyValue("--alcomd-icon-url"));
    await packages.click();
    await projects.click();
    expect(await projects.locator(".alcomd-icon").evaluate((element) => getComputedStyle(element).getPropertyValue("--alcomd-icon-url"))).toBe(selectedIconUrl);
    const projectsBox = await projects.boundingBox();
    const projectsIconBox = await projects.locator(".alcomd-icon").boundingBox();
    const projectsLabel = projects.locator(".navigation-item-label");
    const projectsLabelBox = await projectsLabel.boundingBox();
    const navigationBox = await navigation.boundingBox();
    expect(projectsBox).not.toBeNull();
    expect(projectsIconBox).not.toBeNull();
    expect(projectsLabelBox).not.toBeNull();
    expect(navigationBox).not.toBeNull();
    expect(projectsBox?.height).toBe(48);
    expect(projectsIconBox?.width).toBe(24);
    await expect(projects.locator(".alcomd-icon")).toHaveCSS("-webkit-mask-size", "100%");
    await expect(projects.locator(".alcomd-icon")).toHaveAttribute("data-optical-size", "24");
    expect((projectsBox?.x ?? 0) - (navigationBox?.x ?? 0)).toBe(12);
    expect((projectsIconBox?.x ?? 0) - (navigationBox?.x ?? 0)).toBe(28);
    expect((projectsLabelBox?.x ?? 0) - ((projectsIconBox?.x ?? 0) + (projectsIconBox?.width ?? 0))).toBe(12);
    await expect(projects).toHaveCSS("border-radius", "9999px");
    await expect(projects).toHaveCSS("margin-top", "0px");
    await expect(projectsLabel).toHaveCSS("font-size", "14px");
    await expect(projectsLabel).toHaveCSS("line-height", "20px");
    await expect(projectsLabel).toHaveCSS("font-weight", "500");
    await expect(packages.locator(".navigation-item-label")).toHaveCSS("font-weight", "500");
    expect((await packages.boundingBox())!.y - ((projectsBox?.y ?? 0) + (projectsBox?.height ?? 0))).toBe(4);
    await expect(projects.locator("md-ripple")).toHaveCount(1);
    await expect(projects.locator("md-focus-ring")).toHaveCount(1);
    await expect(packages.locator("md-ripple")).toHaveCount(1);
    await expect(packages.locator("md-focus-ring")).toHaveCount(1);
    await packages.focus();
    await expect(packages.locator("md-focus-ring")).toHaveJSProperty("visible", true);
    expect(remoteFontRequests).toEqual([]);
});

test("Projects toolbar uses semantic Material icons without replacing clear action labels", async ({ page }) => {
    await page.addInitScript(() => {
        Date.now = () => 1_700_000_120_000;
    });
    await openHarness(page, "/projects");
    const main = page.getByRole("main");
    await expect(main.locator(".projects-toolbar md-icon-button > .alcomd-icon")).toHaveAttribute("aria-hidden", "true");
    await expect(main.locator(".projects-toolbar md-icon-button > .alcomd-icon")).toHaveAttribute("data-icon-name", "refresh");
    await expect(main.locator(".projects-toolbar md-icon-button > .alcomd-icon")).toHaveAttribute("data-optical-size", "24");
    const refreshProjects = main.locator(".projects-toolbar md-icon-button");
    const viewToggle = main.locator(".projects-toolbar md-text-button").filter({ hasText: "Grid view" });
    const registerProject = main.locator(".projects-toolbar md-filled-button").filter({ hasText: "Register project" });
    for (const action of [refreshProjects, viewToggle, registerProject]) {
        await expect(action).toHaveCSS("height", "40px");
    }
    await expect(refreshProjects).toHaveCSS("width", "40px");
    await expect(viewToggle.locator('.alcomd-icon[slot="icon"]')).toBeVisible();
    await expect(main.locator('.projects-search > .alcomd-icon[slot="leading-icon"]')).toBeVisible();
    const search = main.locator("md-filled-text-field.projects-search");
    await expect(search).toHaveJSProperty("placeholder", "Search...");
    await expect(main.locator(".projects-secondary-toolbar")).toHaveCount(0);
    await expect(main.getByRole("columnheader", { name: "Packages" })).toHaveCount(0);
    const addedHeader = main.getByRole("columnheader", { name: "Added" });
    await expect(addedHeader).toHaveAttribute("aria-sort", "none");
    await expect(main.getByRole("cell", { name: "2023-07-22" })).toBeVisible();
    await expect(main.getByRole("cell", { name: "2 minutes ago" })).toBeVisible();
    const typeHeader = main.getByRole("columnheader", { name: "Type" });
    const unityHeader = main.getByRole("columnheader", { name: "Unity" });
    const projectHeader = main.getByRole("columnheader", { name: "Project" });
    const actionsHeader = main.getByRole("columnheader", { name: "Actions" });
    const observedHeader = main.getByRole("columnheader", { name: "Last observed" });
    await expect(observedHeader).toHaveAttribute("aria-sort", "descending");
    const firstRow = main.locator(".projects-table tbody tr").first();
    const typeLine = firstRow.locator(".project-type--table > span:last-child");
    const unityCell = firstRow.locator("td").nth(2);
    const textBox = (element: Element) => {
        const range = document.createRange();
        range.selectNodeContents(element);
        const box = range.getBoundingClientRect();
        return { height: box.height, y: box.y };
    };
    const typeTextBox = await typeLine.evaluate(textBox);
    const unityTextBox = await unityCell.evaluate(textBox);
    expect(Math.abs(typeTextBox.y - unityTextBox.y)).toBeLessThanOrEqual(1);
    expect(Math.abs(typeTextBox.height - unityTextBox.height)).toBeLessThanOrEqual(1);
    await expect(firstRow.locator(".project-type--table > .alcomd-icon")).toHaveCSS("width", "24px");
    await expect(firstRow.locator(".project-type--table > .alcomd-icon")).toHaveCSS("height", "24px");
    const intrinsicColumnWidths = await Promise.all([typeHeader, unityHeader, addedHeader, observedHeader].map(async (header) => (await header.boundingBox())?.width));
    const projectWidth = (await projectHeader.boundingBox())?.width ?? 0;
    const actionsColumnWidth = (await actionsHeader.boundingBox())?.width ?? 0;
    await page.setViewportSize({ width: 1440, height: 720 });
    const wideColumnWidths = await Promise.all([typeHeader, unityHeader, addedHeader, observedHeader].map(async (header) => (await header.boundingBox())?.width));
    const wideProjectWidth = (await projectHeader.boundingBox())?.width ?? 0;
    const wideActionsColumnWidth = (await actionsHeader.boundingBox())?.width ?? 0;
    expect(wideColumnWidths).toEqual(intrinsicColumnWidths);
    expect(wideActionsColumnWidth).toBe(actionsColumnWidth);
    expect(wideProjectWidth).toBeGreaterThan(projectWidth);
    const rowActions = main.locator(".project-row-actions");
    const openUnity = rowActions.locator("md-filled-button.project-open-unity-action");
    const manage = rowActions.locator("md-filled-tonal-button").filter({ hasText: "Manage" });
    const backups = rowActions.locator("md-filled-tonal-button").filter({ hasText: "Backups" });
    const moreActions = rowActions.locator("md-icon-button.project-more-actions");
    for (const action of [openUnity, manage, backups, moreActions]) {
        await expect(action).toHaveCSS("height", "40px");
    }
    await expect(moreActions).toHaveCSS("width", "40px");
    const actionsWidth = (await rowActions.boundingBox())?.width;
    const openUnityWidth = (await openUnity.boundingBox())?.width;
    await openUnity.click();
    await expect(openUnity).toContainText("Opening…");
    expect((await openUnity.boundingBox())?.width).toBe(openUnityWidth);
    expect((await rowActions.boundingBox())?.width).toBe(actionsWidth);
    await expect(openUnity).toContainText("Open Unity");
    await page.setViewportSize({ width: 1180, height: 720 });
    const narrowScroll = main.locator(".material-data-table-scroll");
    const narrowProjectWidth = (await projectHeader.boundingBox())?.width ?? 0;
    const narrowActionsBox = await rowActions.boundingBox();
    const narrowScrollBox = await narrowScroll.boundingBox();
    const narrowActionsHeaderBox = await actionsHeader.boundingBox();
    expect(narrowProjectWidth).toBeLessThan(wideProjectWidth);
    expect(narrowProjectWidth).toBeGreaterThanOrEqual(179);
    expect((await rowActions.boundingBox())?.width).toBe(actionsWidth);
    expect((narrowActionsBox?.x ?? 0) + (narrowActionsBox?.width ?? 0)).toBeLessThanOrEqual((narrowScrollBox?.x ?? 0) + (narrowScrollBox?.width ?? 0) + 1);
    expect(Math.abs(((narrowActionsHeaderBox?.x ?? 0) + (narrowActionsHeaderBox?.width ?? 0)) - ((narrowScrollBox?.x ?? 0) + (narrowScrollBox?.width ?? 0)))).toBeLessThanOrEqual(1);
    await expect(actionsHeader).toHaveCSS("position", "sticky");
    await expect(moreActions).toHaveJSProperty("ariaLabel", "More actions for <private-project>");
    const moreIcon = moreActions.locator(".alcomd-icon");
    await expect(moreIcon).toBeVisible();
    await expect(moreIcon).toHaveCSS("width", "24px");
    await expect(moreIcon).toHaveCSS("height", "24px");
    await expect(moreIcon).toHaveAttribute("data-icon-name", "more_vert");
    await expect(moreIcon).toHaveAttribute("data-optical-size", "24");
    await expect(moreIcon).toHaveCSS("-webkit-mask-size", "100%");
    await moreActions.click();
    const projectMenu = rowActions.locator("md-menu");
    await expect(projectMenu).toHaveJSProperty("open", true);
    const openDirectory = projectMenu.locator("md-menu-item").filter({ hasText: "Open Project Directory" });
    const copyProject = projectMenu.locator("md-menu-item").filter({ hasText: "Copy Project" });
    await expect(openDirectory).toHaveJSProperty("disabled", false);
    await expect(copyProject).toHaveJSProperty("disabled", false);
    await expect(openDirectory.getByRole("menuitem", { name: "Open Project Directory" })).toBeVisible();
    await expect(copyProject.getByRole("menuitem", { name: "Copy Project" })).toBeVisible();
    await copyProject.getByRole("menuitem", { name: "Copy Project" }).click();
    const copyDialog = page.getByRole("dialog", { name: "Copy project" });
    await expect(copyDialog).toBeVisible();
    await page.getByRole("button", { name: "Review copy" }).click();
    await expect(page.getByText(/C:\\Fixture\\Avatar/)).toBeVisible();
    await page.getByRole("button", { name: "Start copy" }).click();
    await expect(page.locator('[role="status"]').filter({ hasText: "Copy succeeded" })).toBeVisible({ timeout: 3_000 });
    await page.getByRole("button", { name: "Close" }).click();
    await expect(page.getByRole("heading", { level: 1, name: "Projects" })).toBeVisible();
    await moreActions.click();
    const removeProject = projectMenu.getByRole("menuitem", { name: "Remove Project" });
    await expect(removeProject).toBeVisible();
    await expect(removeProject).toHaveCSS("--md-menu-item-label-text-color", "#b3261e");
    await removeProject.click();
    const removeDialog = page.getByRole("dialog", { name: "Remove this project?" });
    await expect(removeDialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(removeDialog).not.toBeVisible();
    const observedButton = observedHeader.getByRole("button", { name: "Last observed" });
    const observedControl = observedHeader.locator("md-text-button");
    const observedLabel = observedControl.locator(".material-data-table-sort-label");
    expect(await observedLabel.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    const headerBox = await observedHeader.boundingBox();
    const buttonBox = await observedControl.boundingBox();
    expect(headerBox).not.toBeNull();
    expect(buttonBox).not.toBeNull();
    expect(Math.abs((headerBox?.x ?? 0) - (buttonBox?.x ?? 0))).toBeLessThanOrEqual(1);
    expect(Math.abs((headerBox?.width ?? 0) - (buttonBox?.width ?? 0))).toBeLessThanOrEqual(1);
    expect(Math.abs((headerBox?.height ?? 0) - (buttonBox?.height ?? 0))).toBeLessThanOrEqual(1);
    const sortContentBox = await observedControl.locator(".material-data-table-sort-content").boundingBox();
    expect(Math.abs(((sortContentBox?.x ?? 0) - (headerBox?.x ?? 0)) - 12)).toBeLessThanOrEqual(1);
    const sortIconBox = await observedControl.locator(".material-data-table-sort-icon").boundingBox();
    const sortLabelBox = await observedControl.locator(".material-data-table-sort-label").boundingBox();
    expect(sortContentBox?.height).toBe(20);
    expect(sortLabelBox?.height).toBe(20);
    await expect(observedControl.locator(".material-data-table-sort-content")).toHaveCSS("position", "absolute");
    await expect(observedControl.locator(".material-data-table-sort-label")).toHaveCSS("line-height", "20px");
    expect(sortIconBox?.width).toBe(20);
    expect(sortIconBox?.height).toBe(20);
    expect(Math.abs(((sortIconBox?.y ?? 0) + (sortIconBox?.height ?? 0) / 2) - ((sortLabelBox?.y ?? 0) + (sortLabelBox?.height ?? 0) / 2))).toBeLessThanOrEqual(1);
    const inactiveTypeControl = typeHeader.locator("md-text-button");
    const inactiveTypeContent = inactiveTypeControl.locator(".material-data-table-sort-content");
    const inactiveTypeLabel = inactiveTypeControl.locator(".material-data-table-sort-label");
    const inactiveTypeContentBox = await inactiveTypeContent.boundingBox();
    const inactiveTypeHeaderBox = await typeHeader.boundingBox();
    const inactiveTypeLabelBox = await inactiveTypeLabel.boundingBox();
    expect(Math.abs(((inactiveTypeContentBox?.x ?? 0) - (inactiveTypeHeaderBox?.x ?? 0)) - 12)).toBeLessThanOrEqual(1);
    expect(Math.abs(((inactiveTypeLabelBox?.x ?? 0) - (inactiveTypeContentBox?.x ?? 0)) - 24)).toBeLessThanOrEqual(1);
    const inactiveTypeIcon = inactiveTypeControl.locator(".material-data-table-sort-icon");
    await expect(inactiveTypeIcon).toHaveCount(1);
    await expect(inactiveTypeIcon).toHaveCSS("visibility", "hidden");
    await expect(main.locator(".material-data-table tbody td").first()).toHaveCSS("white-space", "nowrap");
    await expect(main.locator(".material-data-table tbody td").first()).toHaveCSS("text-overflow", "ellipsis");
    await observedButton.click();
    await expect(observedHeader).toHaveAttribute("aria-sort", "ascending");
    await main.getByRole("button", { name: "Grid view" }).click();
    const sort = main.locator("md-filled-select.projects-sort");
    await expect(sort).toHaveJSProperty("value", "observed");
    await expect(main.getByRole("combobox", { name: "Sort by" })).toContainText("Last observed");
    await expect(main.locator(".projects-secondary-toolbar md-icon-button > .alcomd-icon")).toBeVisible();
    await expect(main.getByRole("button", { name: "Register project" })).toBeVisible();
    expect((await search.boundingBox())?.height).toBe(40);
    expect((await sort.boundingBox())?.height).toBe(40);
    await main.getByRole("button", { name: "List view" }).click();
    await expect(main.locator(".projects-secondary-toolbar")).toHaveCount(0);
    await expect(main.getByRole("columnheader", { name: "Last observed" })).toHaveAttribute("aria-sort", "ascending");
    await addedHeader.getByRole("button", { name: "Added" }).click();
    await expect(addedHeader).toHaveAttribute("aria-sort", "descending");
});

test("Project workspace Copy completes through Plan Apply and navigates to the copied Project", async ({ page }) => {
    await openHarness(page, "/projects/00000000-0000-4000-8000-000000000101");
    const main = page.getByRole("main");
    await main.getByRole("button", { name: "Copy project" }).click();
    const dialog = page.getByRole("dialog", { name: "Copy project" });
    await expect(dialog).toBeVisible();
    await page.getByRole("button", { name: "Review copy" }).click();
    await page.getByRole("button", { name: "Start copy" }).click();
    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000061$/);
});

async function openHarness(page: Page, route: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=ready`);
}

function navigationItem(page: Page, label: string) {
    return page.locator("#primary-navigation md-list-item").filter({
        has: page.locator(".navigation-item-label", { hasText: new RegExp(`^${escapeRegex(label)}$`) })
    });
}

function escapeRegex(value: string) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
