import { expect, test } from "@playwright/test";
import { expectRequestError } from "./request-error-assertions";

test("Projects Create uses the typed Template Plan Apply Operation flow and opens the Core-created project", async ({ page }) => {
    await openHarness(page, "/projects");

    await page.getByRole("button", { name: "Create project" }).click();
    const dialog = page.getByRole("dialog", { name: "Create project" });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Project template", exact: true })).toContainText("Avatar starter");
    await page.getByRole("button", { name: "Choose directory" }).click();
    await expect(page.getByLabel("Create target parent")).toHaveValue("C:\\Fixture\\Avatar");
    await page.getByLabel("Create project name").fill("Created Project");
    await page.getByRole("button", { name: "Review creation" }).click();
    await expect(page.getByText("Destination", { exact: true }).locator("..")).toContainText("Created Project");
    await expect(page.getByText(/ALCOMD Core will validate the frozen template/)).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Create project" }).click();

    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/, { timeout: 5_000 });
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Created Project" })).toBeVisible();
});

test("creation accepts an explicit uninstalled Editor version", async ({ page }) => {
    await openHarness(page, "/projects", "unity-zero");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("combobox", { name: "Unity version", exact: true })).toContainText("Custom");
    await expect(page.getByRole("textbox", { name: "Unity version", exact: true })).toHaveValue("");
    await page.getByRole("textbox", { name: "Unity version", exact: true }).fill("2022.3.99f1");
    await page.getByLabel("Create project name").fill("Uninstalled Version");
    await page.getByRole("button", { name: "Review creation" }).click();
    await expect(page.getByText("Unity version", { exact: true }).locator("..")).toContainText("2022.3.99f1");
    expect(await page.evaluate(() => window.templateRequests.find((request) => request.method === "templates.planCreateProject")?.params)).toMatchObject({ unityVersion: "2022.3.99f1" });
});

test("creation prefills one compatible Editor but requires a choice for multiple installations", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    const version = page.getByRole("combobox", { name: "Unity version", exact: true });
    await expect(version).toContainText("2022.3.22f1");
    await expect(page.getByRole("textbox", { name: "Unity version", exact: true })).toHaveCount(0);
    await version.click();
    await page.getByRole("option", { name: "Custom", exact: true }).click();
    const custom = page.getByRole("textbox", { name: "Unity version", exact: true });
    await custom.fill("2022.3.22f1");
    await expect(custom).toBeVisible();
    await version.click();
    await page.getByRole("option", { name: "2022.3.22f1", exact: true }).click();
    await expect(custom).toHaveCount(0);
    await openHarness(page, "/projects", "unity-multiple");
    await page.getByRole("button", { name: "Create project", exact: true }).click();
    await expect(page.getByRole("combobox", { name: "Unity version", exact: true })).toBeVisible();
    await expect(page.getByRole("textbox", { name: "Unity version", exact: true })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Review creation" })).toBeDisabled();
});

test("Projects Restore uses a managed Backup Plan Apply Operation and opens the registered result", async ({ page }) => {
    await openHarness(page, "/projects");

    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Restore project" });
    await expect(dialog).toBeVisible();
    await expect(page.getByRole("combobox", { name: "Project backup", exact: true })).toContainText("11/14/2023");
    await page.getByRole("button", { name: "Choose directory" }).click();
    await expect(page.getByLabel("Restore target parent")).toHaveValue("C:\\Fixture\\Avatar");
    await page.getByLabel("Restore project name").fill("Restored Project");
    await page.getByRole("button", { name: "Review restore" }).click();
    await expect(page.getByText(/Destination/).locator("..")).toContainText("Restored Project");
    await expect(page.getByText(/ALCOMD Core will validate the managed archive/)).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Restore project" }).click();

    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000109$/, { timeout: 5_000 });
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Restored Project" })).toBeVisible();
});

test("Create and Restore expose cancel, empty, and structured planning failure states", async ({ page }) => {
    await openHarness(page, "/projects");
    await page.getByRole("button", { name: "Create project" }).click();
    await page.getByRole("button", { name: "Cancel" }).click();
    await expect(page.getByRole("dialog", { name: "Create project" })).toHaveCount(0);

    await openHarness(page, "/projects", "empty");
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
    await expect(page.getByText("No managed backups available")).toBeVisible();
    const restoreDialogBox = await page.getByRole("dialog", { name: "Restore project" }).boundingBox();
    const closeBox = await page.getByRole("button", { name: "Close", exact: true }).boundingBox();
    expect(restoreDialogBox).not.toBeNull();
    expect(closeBox).not.toBeNull();
    expect(closeBox!.x + closeBox!.width).toBeLessThanOrEqual(restoreDialogBox!.x + restoreDialogBox!.width);

    await openHarness(page, "/projects", "create-error");
    await page.getByRole("button", { name: "Create project" }).click();
    await page.getByRole("button", { name: "Choose directory" }).click();
    await page.getByLabel("Create project name").fill("Conflict");
    await page.getByRole("button", { name: "Review creation" }).click();
    await expectRequestError(page.getByRole("alert"), "template_plan_stale");

    await openHarness(page, "/projects", "restore-error");
    await page.getByRole("button", { name: "More project creation options" }).click();
    await page.getByRole("menuitem", { name: "Restore from backup", exact: true }).click();
    await page.getByRole("button", { name: "Choose directory" }).click();
    await page.getByLabel("Restore project name").fill("Conflict");
    await page.getByRole("button", { name: "Review restore" }).click();
    await expectRequestError(page.getByRole("alert"), "backup_target_exists");
});

async function openHarness(page: import("@playwright/test").Page, route: string, state = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}
