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
    await expect(row(page, "Avatar tools").locator(".package-source-summary")).toHaveText("Example packages");
});

test("latest shows the Core project-compatible version used by the update review", async ({ page }) => {
    await open(page, "ready", "versions=1&incompatibleLatest=1");
    const installed = row(page, "Avatar tools");
    const latest = installed.locator("td").nth(3);
    await expect(latest.locator(".package-latest-update")).toHaveText("1.10.0");
    await expect(latest.locator(".package-latest-update")).toHaveAttribute("title", "Incompatible version: 3.0.0");
    await expect(latest.locator("small")).toHaveCount(0);
    await installed.getByRole("button", { name: "Update", exact: true }).click();
    const review = page.locator("md-dialog[open]");
    await expect(review).toContainText("1.2.3 → 1.10.0");
    expect(await page.evaluate(() => window.packageRequests[0])).toMatchObject({ method: "upgrade", params: { versionRange: "=1.10.0" } });
    await review.getByRole("button", { name: "Cancel", exact: true }).click();
    expect(await page.evaluate(() => window.packageRequests.map((request) => request.method))).toEqual(["upgrade"]);
});


test("installed yank warning follows the current version like v3 without waiting for repository inventory", async ({ page }, testInfo) => {
    await open(page, "ready", "presentation=1&installedYanked=1&holdRepositories=1");
    const installed = row(page,"Avatar tools");
    await expect(installed.locator(".package-column-name strong")).toHaveAttribute("title","Package description from Core");
    await expect(installed.locator(".package-column-name")).not.toContainText("Yanked");
    await expect(installed.getByRole("combobox", { name: "Version for Avatar tools: 1.2.3 Yanked", exact: true })).toBeVisible();
    await expect(installed.locator(".package-version-picker")).toHaveJSProperty("displayText", "1.2.3 Yanked");
    await expect(installed.locator(".package-version-picker")).toHaveClass(/package-version-picker--yanked/);
    await expect(row(page,"Remote tools").locator(".package-version-picker")).toHaveJSProperty("displayText", "Not installed");
    await expect(row(page,"Remote tools").locator(".package-version-picker")).not.toHaveClass(/package-version-picker--yanked/);
    const textColor = (picker: HTMLElement) => getComputedStyle(picker.shadowRoot!.querySelector("#label")!).color;
    expect(await installed.locator(".package-version-picker").evaluate(textColor)).not.toBe(await row(page,"Remote tools").locator(".package-version-picker").evaluate(textColor));
    for (const packageRow of [installed, row(page, "Remote tools")]) {
        const bounds = await packageRow.locator(".package-version-picker").evaluate((picker) => {
            const label = picker.shadowRoot!.querySelector<HTMLElement>("#label")!;
            return { text: label.textContent, content: label.scrollWidth, available: label.clientWidth };
        });
        expect(bounds.content, JSON.stringify(bounds)).toBeLessThanOrEqual(bounds.available);
    }
    await page.screenshot({ path: testInfo.outputPath("v3-yanked-version.png") });
    await page.getByRole("button",{name:"Filter packages",exact:true}).click();
    await expect(page.getByRole("status").filter({hasText:"Loading…"})).toBeVisible();
    await page.evaluate(() => window.dispatchEvent(new Event("test-repositories-ready")));
    await expect(page.getByRole("checkbox",{name:"Example packages",exact:true})).toBeVisible();
});

test("version groups follow Core compatibility and yanked versions are absent", async ({ page }, testInfo) => {
    await open(page,"ready","versionGroups=1");
    await row(page,"Avatar tools").locator(".package-version-picker").getByRole("combobox").click();
    const picker = row(page,"Avatar tools").locator(".package-version-picker");
    await expect(picker.locator(".alcomd-select-heading")).toHaveText(["Compatibility unknown", "Incompatible versions"]);
    for (const heading of ["Compatibility unknown","Incompatible versions"]) {
        await expect(picker.getByRole("option", { name: heading, exact: true })).toHaveCount(0);
    }
    await expect(picker.locator("md-select-option").filter({hasText:"9.0.0"})).toHaveCount(0);
    await page.screenshot({path:testInfo.outputPath("version-compatibility-groups.png")});
});


test("normal package reads have no separate fill region or project-refresh requests", async ({ page }) => {
    await open(page, "ready", "partialPackageManifest=1");
    await expect(page.locator(".package-presentation-status")).toHaveCount(0);
    await expect(page.getByText("Some package details are unavailable.", { exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Refresh package details", exact: true })).toHaveCount(0);
    await expect(row(page, "Avatar tools")).toContainText("1.2.3");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(row(page, "Refreshed package")).toBeVisible();
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "projects.refresh"))).toEqual([]);
});

test("the version column shows only the current Core installed version like v3", async ({ page }, testInfo) => {
    await open(page, "ready", "freshProjectPackage=1");
    const installed=row(page, "Avatar tools");
    await expect(installed.getByRole("combobox", { name: "Version for Avatar tools: 1.2.4", exact: true })).toBeVisible();
    await expect(installed.locator(".package-version-picker")).toHaveJSProperty("displayText", "1.2.4");
    await expect(installed.locator("td").nth(2).locator("small")).toHaveCount(0);
    await expect(installed).not.toContainText("Requested");
    await expect(installed).not.toContainText("^1.2.4");
    await expect(row(page, "Remote tools").locator(".package-version-picker")).toHaveJSProperty("displayText", "Not installed");
    await page.screenshot({ path: testInfo.outputPath("v3-version-column.png") });
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "projects.refresh"))).toEqual([]);
});

test("installed versions are present in the first painted fields without a version-list read or refresh", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fprojects&state=ready");
    await expect(page.getByRole("button", { name: "Manage", exact: true })).toBeVisible();
    const recording = await page.evaluateHandle(() => {
        const state = { frames: [] as Array<Array<string>>, done: false };
        const sample = () => {
            const fields = [...document.querySelectorAll(".package-version-picker")];
            if (fields.length && fields.every((field) => field.shadowRoot?.querySelector("#field")?.getBoundingClientRect().height)) {
                state.frames.push(fields.map((field) => field.shadowRoot?.querySelector("#label")?.textContent?.trim() ?? ""));
                state.done = state.frames.length === 8;
            }
            if (!state.done) requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
        return state;
    });
    await page.getByRole("button", { name: "Manage", exact: true }).click();
    await expect.poll(() => recording.evaluate((state) => state.done)).toBe(true);
    const frames = await recording.evaluate((state) => state.frames);
    for (const frame of frames) expect(frame).toEqual(["1.2.3", "Not installed"]);
    expect(await page.evaluate(() => window.candidateRequests.some((request) => request.view.kind === "versions"))).toBe(false);
    expect(await page.evaluate(() => window.resourceRequests.filter((request) => request.method === "projects.refresh"))).toEqual([]);
    await recording.dispose();
});
