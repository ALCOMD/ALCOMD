import { expect, test, type Page } from "@playwright/test";
import { selectSettingsSection } from "./settings-sections";

const nativePath = "C:/fixture/native.txt";
const typedPath = "C:/fixture/typed.txt";
async function result(page: Page, detail: { path?: string; error?: boolean }) {
    await page.evaluate((detail) => window.dispatchEvent(new CustomEvent("fixture-selection-result", { detail })), detail);
}

test("the import picker retains the approved compact appearance", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("/browser-harness.html?route=/repositories");
    await page.getByRole("button", { name: "Repository list actions", exact: true }).click();
    await page.getByRole("menuitem", { name: "Import list", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    const picker = dialog.locator(".file-picker");
    const drop = picker.locator(".file-picker__drop");
    await expect(picker).toBeVisible();
    const geometry = await picker.evaluate((element) => {
        const picker = element.getBoundingClientRect();
        const drop = element.querySelector(".file-picker__drop")!.getBoundingClientRect();
        return { width: picker.width, dropWidth: drop.width, dropHeight: drop.height, gap: getComputedStyle(element).gap };
    });
    expect(geometry.width).toBeCloseTo(512, 0);
    expect(geometry.dropWidth / geometry.dropHeight).toBeCloseTo(3, 1);
    expect(geometry.gap).toBe("12px");
    await expect(drop).toHaveCSS("border-top-style", "dashed");
    await page.screenshot({ path: testInfo.outputPath("approved-appearance.png") });
    await page.setViewportSize({ width: 600, height: 700 });
    await expect(picker).toBeVisible();
    expect(await picker.evaluate((element) => element.scrollWidth <= element.clientWidth + 1)).toBe(true);
});

test("selection is single flight, cancellation retains the path, and failure permits retry", async ({ page }) => {
    await page.goto("/browser-harness.html?file-picker-evidence=1");
    const select = page.getByRole("button", { name: "Select", exact: true });
    const field = page.getByRole("textbox", { name: "Evidence path", exact: true });
    await select.click();
    await expect(page.getByTestId("picker-choosing")).toHaveText("true");
    await expect(field).toHaveJSProperty("readOnly", true);
    await select.evaluate((element: HTMLElement) => { element.click(); element.click(); });
    await expect(page.getByTestId("picker-requests")).toHaveText("1");
    await result(page, {});
    await expect(page.getByTestId("picker-choosing")).toHaveText("false");
    await expect(field).toHaveValue(typedPath);
    await select.click();
    await result(page, { error: true });
    await expect(page.getByRole("alert")).toHaveText("Could not select the file.");
    await select.click();
    await result(page, { path: nativePath });
    await expect(field).toHaveValue(nativePath);
    await expect(page.getByRole("alert")).toHaveCount(0);
});

for (const action of ["disabled", "mounted"] as const) {
    test(`a pending selection cannot change the path after ${action} changes`, async ({ page }) => {
        await page.goto("/browser-harness.html?file-picker-evidence=1");
        await page.getByRole("button", { name: "Select", exact: true }).click();
        const toggle = page.getByRole("button", { name: `Toggle ${action}`, exact: true });
        await toggle.click();
        await toggle.click();
        await result(page, { path: nativePath });
        await expect(page.getByTestId("picker-path")).toHaveText(typedPath);
        await expect(page.getByRole("textbox", { name: "Evidence path", exact: true })).toHaveValue(typedPath);
    });
}

test("Unity Hub uses an inline path field and select button", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/settings");
    await selectSettingsSection(page, "unity");
    const section = page.locator(".settings-section").filter({ has: page.locator("#settings-unity-hub-path") });
    await expect(page.locator(".file-picker")).toHaveCount(0);
    await section.getByRole("button", { name: "Select", exact: true }).click();
    await expect(section.getByRole("textbox", { name: "Unity Hub Path", exact: true })).toHaveValue("C:\\Tools\\Unity Hub\\Unity Hub.exe");
});

for (const entry of [
    { route: "/unity", action: "Manage installations", label: "Unity executable", path: "C:/Editors/Unity.exe" },
    { route: "/extensions", action: "Install extension", label: "Extension package", path: "C:/fixture/test.alcomdextension" },
    { route: "/repositories", action: "Add repository", label: "Local manifest path", path: "C:/fixture/repository.json" }
]) {
    test(`${entry.label} uses only inline controls with a working native chooser`, async ({ page }) => {
        await page.goto(`/browser-harness.html?route=${entry.route}`);
        if (entry.route === "/unity") await selectSettingsSection(page, "unity");
        await page.getByRole("button", { name: entry.action, exact: true }).click();
        const dialog = page.locator("md-dialog[open]");
        if (entry.route === "/repositories") {
            await dialog.getByRole("combobox", { name: "Source type", exact: true }).click();
            await dialog.getByRole("option", { name: "Local manifest", exact: true }).click();
        }
        await expect(dialog.locator(".file-picker")).toHaveCount(0);
        await dialog.getByRole("button", { name: "Select", exact: true }).click();
        await expect(dialog.getByRole("textbox", { name: entry.label, exact: true })).toHaveValue(entry.path);
    });
}

test("replacing an existing template uses only inline selection controls", async ({ page }) => {
    await page.goto("/browser-harness.html?route=/templates&template-existing=1&template-file-replace=1");
    await page.getByRole("button", { name: "More actions for Existing fixture", exact: true }).click();
    await page.getByRole("menuitem", { name: "Replace template", exact: true }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.locator(".file-picker")).toHaveCount(0);
    await dialog.getByRole("button", { name: "Select", exact: true }).click();
    await expect(dialog.getByRole("textbox", { name: "Template file", exact: true })).toHaveValue("C:\\Fixture\\input.alcomdtemplate");
});
