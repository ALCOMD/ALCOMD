import { expect, test, type Page } from "@playwright/test";

const projectId = "00000000-0000-4000-8000-000000000101";
async function open(page: Page, state = "ready", flags = "") {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto(`/browser-harness.html?route=/projects/${projectId}&state=${state}&${flags}`);
    await expect(page.getByRole("table", { name: "Packages", exact: true })).toBeVisible();
}
function row(page: Page, name: string) {
    return page.getByRole("row").filter({ has: page.getByRole("checkbox", { name: `Select ${name}`, exact: true }) });
}

test("the expanded menu has plain versions and one selected current version without a placeholder or compatible heading", async ({ page }, info) => {
    await open(page);
    const installed = row(page, "Avatar tools");
    const picker = installed.locator(".package-version-picker");
    await picker.getByRole("combobox").click();
    await expect(picker.locator("md-select-option")).toHaveText(["1.10.0", "1.2.3", "1.1.0"]);
    await expect(picker.locator(".alcomd-select-heading")).toHaveCount(0);
    await expect(picker.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.2.3", exact: true }) })).toHaveJSProperty("selected", true);
    await expect(picker.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.2.3", exact: true }) })).toHaveJSProperty("disabled", false);
    await page.screenshot({ path: info.outputPath("plain-version-menu.png") });
    await picker.getByRole("option", { name: "1.2.3", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?", exact: true })).toHaveCount(0);
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await row(page, "Remote tools").locator(".package-version-picker").getByRole("combobox").click();
    await expect(row(page, "Remote tools").getByRole("option", { name: "Not installed", exact: true })).toHaveCount(0);
});

test("group labels are not selectable rows and technical explanations do not enter version labels", async ({ page }, info) => {
    await open(page, "ready", "versionGroups=1");
    const picker = row(page, "Avatar tools").locator(".package-version-picker");
    await picker.getByRole("combobox").click();
    await expect(picker.locator(".alcomd-select-heading")).toHaveText(["Compatibility unknown", "Incompatible versions"]);
    await expect(picker.getByRole("separator")).toHaveCount(2);
    await expect(picker.locator("md-select-option")).toHaveText(["1.10.0", "1.2.3", "1.0.0", "1.1.0"]);
    await expect(picker.getByRole("option", { name: "Incompatible versions", exact: true })).toHaveCount(0);
    await page.screenshot({ path: info.outputPath("incompatible-version-section.png") });
});

test("source collisions render one version and require explicit provenance before creating a plan", async ({ page }) => {
    await open(page, "package-multiple", "versions=1");
    const installed = row(page, "Avatar tools");
    const picker = installed.locator(".package-version-picker");
    await picker.getByRole("combobox").click();
    await expect(picker.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.1.0", exact: true }) })).toHaveCount(1);
    await expect(picker.locator("md-select-option").filter({ has: page.getByRole("option", { name: "1.1.0", exact: true }) })).toHaveJSProperty("disabled", true);
    await page.keyboard.press("Escape");
    await installed.locator(".package-source-picker").getByRole("combobox").click();
    await page.getByRole("option", { name: "Local packages", exact: true }).click();
    await picker.getByRole("combobox").click();
    await picker.getByRole("option", { name: "1.1.0", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Apply package changes?", exact: true })).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests[0])).toMatchObject({ method: "downgrade", params: { version: "1.1.0", source: { kind: "repository", repositoryId: "00000000-0000-4000-8000-000000000112" } } });
});

test("the latest column owns upgrading while the row tail owns install or remove", async ({ page }, info) => {
    await open(page);
    const installed = row(page, "Avatar tools");
    const available = row(page, "Remote tools");
    await expect(installed.locator(".package-column-latest").getByRole("button", { name: "Update", exact: true })).toBeVisible();
    await expect(installed.locator(".package-column-latest .package-latest-update")).toHaveText("1.10.0");
    await expect(installed.locator(".package-column-actions").getByRole("button", { name: "Remove", exact: true })).toBeEnabled();
    await expect(available.locator(".package-column-actions").getByRole("button", { name: "Install", exact: true })).toBeEnabled();
    await expect(installed.locator(".package-column-latest small")).toHaveCount(0);
    await expect(available.locator(".package-column-latest small")).toHaveCount(0);
    await expect(installed.locator(".package-source-summary")).toHaveText("Example packages");
    await expect(installed.locator(".package-source-picker")).toHaveCount(0);
    await installed.getByRole("button", { name: "More actions for Avatar tools", exact: true }).click();
    await expect(installed.locator("md-menu-item")).toHaveText(["Reinstall", "Changelog", "Documentation"]);
    await page.keyboard.press("Escape");
    await page.screenshot({ path: info.outputPath("package-workspace-parity.png") });
});

test("Shift selection spans visible rows and Shift deselection clears the same range", async ({ page }) => {
    await open(page, "package-multiple");
    const choices = page.getByRole("table", { name: "Packages", exact: true }).locator("tbody").getByRole("checkbox");
    await expect(choices).toHaveCount(3);
    await choices.nth(0).click();
    await choices.nth(2).click({ modifiers: ["Shift"] });
    for (let i = 0; i < 3; i++) await expect(choices.nth(i)).toBeChecked();
    await choices.nth(0).click({ modifiers: ["Shift"] });
    for (let i = 0; i < 3; i++) await expect(choices.nth(i)).not.toBeChecked();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
});

test("manual incompatible selection requires confirmation and cancellation creates no plan", async ({ page }) => {
    await open(page, "ready", "versionGroups=1");
    const picker = row(page, "Avatar tools").locator(".package-version-picker");
    await picker.getByRole("combobox").click();
    await picker.getByRole("option", { name: "1.1.0", exact: true }).click();
    const confirmation = page.locator("md-dialog").filter({ hasText: "Use incompatible version?" });
    await expect(confirmation).toBeVisible();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await confirmation.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(confirmation).toBeHidden();
    expect(await page.evaluate(() => window.packageRequests)).toEqual([]);
    await picker.getByRole("combobox").click();
    await picker.getByRole("option", { name: "1.1.0", exact: true }).click();
    await confirmation.getByRole("button", { name: "Continue with this version", exact: true }).click();
    const review = page.locator("md-dialog").filter({ hasText: "Apply package changes?" });
    await expect(review).toBeVisible();
    await expect(review.locator(".package-compatibility-warning")).toContainText("1.1.0");
    expect(await page.evaluate(() => window.packageRequests)).toMatchObject([{ method: "downgrade", params: { version: "1.1.0", allowUnityIncompatible: true } }]);
});
