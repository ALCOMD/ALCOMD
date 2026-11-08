import { expect, test, type Page } from "@playwright/test";

const projectId = "00000000-0000-4000-8000-000000000101";
async function open(page: Page, state = "ready", options = "") {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto(`/browser-harness.html?route=/projects/${projectId}&state=${state}&${options}`);
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
}
function row(page: Page, name: string) {
    return page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: `Select ${name}`, exact: true }) });
}

test("installed and available package identities have distinct emphasis without dimming controls", async ({ page }, testInfo) => {
    await open(page);
    const installed = row(page, "Avatar tools");
    const available = row(page, "Remote tools");
    await expect(installed).toHaveClass("package-row--installed");
    await expect(available).toHaveClass("package-row--available");
    const installedName = installed.locator(".package-column-name strong");
    const availableName = available.locator(".package-column-name strong");
    await expect(installedName).toHaveCSS("font-weight", "600");
    await expect(availableName).toHaveCSS("font-weight", "400");
    expect(await installedName.evaluate((element) => getComputedStyle(element).color)).not.toBe(await availableName.evaluate((element) => getComputedStyle(element).color));
    await expect(installed).toHaveCSS("opacity", "1");
    await expect(available).toHaveCSS("opacity", "1");
    await expect(available.getByRole("button", { name: "Install", exact: true })).toBeEnabled();
    await expect(installed.getByRole("button", { name: "Update", exact: true })).toBeEnabled();
    await page.screenshot({ path: testInfo.outputPath("installed-and-available.png") });
});

test("source facts are visible while the menu retains automatic and explicit choices", async ({ page }) => {
    await open(page, "package-multiple", "versions=1");
    const source = row(page, "Avatar tools").locator(".package-source-picker");
    await expect(source).toHaveJSProperty("displayText", "Multiple sources");
    await expect(source).toHaveJSProperty("value", "");
    await expect(row(page, "Avatar tools").locator(".package-source-summary")).toHaveAttribute("title", "Example packages, Local packages");
    await source.getByRole("combobox").click();
    await expect(page.getByRole("option", { name: "Automatic source selection", exact: true })).toBeVisible();
    await page.getByRole("option", { name: "Example packages", exact: true }).click();
    await expect(source).toHaveJSProperty("displayText", "Example packages");
    await expect(source).toHaveJSProperty("value", "repository:00000000-0000-4000-8000-000000000102");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);

    await open(page);
    await expect(row(page, "Avatar tools").locator(".package-source-picker")).toHaveJSProperty("displayText", "Example packages");
});

test("latest shows the Core project-compatible version used by the update review", async ({ page }) => {
    await open(page, "ready", "versions=1&incompatibleLatest=1");
    const installed = row(page, "Avatar tools");
    const latest = installed.locator("td").nth(3);
    await expect(latest.locator("span")).toHaveText("1.10.0");
    await expect(latest.locator("small")).toHaveText("Incompatible version: 3.0.0");
    await installed.getByRole("button", { name: "Update", exact: true }).click();
    const review = page.locator("md-dialog[open]");
    await expect(review).toContainText("1.2.3 → 1.10.0");
    expect(await page.evaluate(() => window.packageRequests[0])).toMatchObject({ method: "upgrade", params: { versionRange: "=1.10.0" } });
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["upgrade"]);
});


test("installed yank warning and description come from Core without waiting for repository inventory", async ({ page }) => {
    await open(page, "ready", "presentation=1&installedYanked=1&holdRepositories=1");
    const installed = row(page,"Avatar tools");
    await expect(installed.locator(".package-column-name strong")).toHaveAttribute("title","Package description from Core");
    await expect(installed).toContainText("Installed version is yanked");
    await expect(row(page,"Remote tools")).not.toContainText("Installed version is yanked");
    await page.getByRole("button",{name:"Filter packages",exact:true}).click();
    await expect(page.getByRole("status").filter({hasText:"Loading…"})).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await expect(page.getByRole("checkbox",{name:"Example packages",exact:true})).toBeVisible();
});

test("version groups follow Core compatibility and yanked versions are absent", async ({ page }, testInfo) => {
    await open(page,"ready","versionGroups=1");
    await row(page,"Avatar tools").locator(".package-version-picker").getByRole("combobox").click();
    const picker = row(page,"Avatar tools").locator(".package-version-picker");
    for (const heading of ["Compatible versions","Compatibility unknown","Incompatible versions"]) {
        await expect.poll(() => picker.locator("md-select-option").evaluateAll((elements) => elements.map((element) => ({ label: element.textContent?.trim(), disabled: (element as HTMLElement & { disabled: boolean }).disabled })))).toContainEqual({ label: heading, disabled: true });
    }
    await expect(picker.locator("md-select-option").filter({hasText:"9.0.0"})).toHaveCount(0);
    await page.screenshot({path:testInfo.outputPath("version-compatibility-groups.png")});
});


test("incomplete presentation keeps installed facts and package actions visible", async ({ page }) => {
    await open(page,"ready","presentationIncomplete=1");
    await expect(page.getByText("Some package details are unavailable.",{exact:true})).toBeVisible();
    const installed=row(page,"Avatar tools");
    await expect(installed).toContainText("1.2.3");
    await expect(installed.getByRole("button",{name:"Update",exact:true})).toBeEnabled();
    await expect(installed).not.toContainText("Installed version is yanked");
    await page.getByRole("button",{name:"Refresh package details",exact:true}).click();
    await expect(page.getByText("Some package details are unavailable.",{exact:true})).toHaveCount(0);
    await expect(row(page,"Avatar tools")).toContainText("1.2.3");
});


test("failed presentation refresh keeps the installed list and allows retry", async ({ page }) => {
    await open(page,"ready","presentationIncomplete=1&presentationRefreshError=1");
    const refresh=page.getByRole("button",{name:"Refresh package details",exact:true});
    await refresh.click();
    await expect(page.getByRole("alert")).toContainText("Core is unavailable.");
    await expect(refresh).toBeEnabled();
    await expect(row(page,"Avatar tools")).toContainText("1.2.3");
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});
