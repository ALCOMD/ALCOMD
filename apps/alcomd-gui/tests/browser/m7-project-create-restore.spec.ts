import { expect, test } from "@playwright/test";

test("Projects Create uses the typed Template Plan Apply Operation flow and opens the Core-created project", async ({ page }) => {
    await openHarness(page, "/projects");

    await page.getByRole("button", { name: "Create project" }).click();
    const dialog = page.getByRole("dialog", { name: "Create project" });
    await expect(dialog).toBeVisible();
    await expect(page.getByLabel("Project template")).toContainText("Avatar starter");
    await page.getByRole("button", { name: "Choose directory" }).click();
    await expect(page.getByLabel("Create target parent")).toHaveValue("C:\\Fixture\\Avatar");
    await page.getByLabel("Create project name").fill("Created Project");
    await page.getByRole("button", { name: "Review creation" }).click();
    await expect(page.getByText(/Destination/).locator("..")).toContainText("Created Project");
    await expect(page.getByText(/ALCOMD Core will validate the frozen template/)).toBeVisible();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Create project" }).click();

    await expect(page).toHaveURL(/\/projects\/00000000-0000-4000-8000-000000000108$/, { timeout: 5_000 });
    await page.getByRole("button", { name: "Back", exact: true }).click();
    await expect(page.getByRole("row").filter({ hasText: "Created Project" })).toBeVisible();
});

test("Projects Restore uses a managed Backup Plan Apply Operation and opens the registered result", async ({ page }) => {
    await openHarness(page, "/projects");

    await page.getByRole("button", { name: "Restore project" }).click();
    const dialog = page.getByRole("dialog", { name: "Restore project" });
    await expect(dialog).toBeVisible();
    await expect(page.getByLabel("Project backup")).toContainText("2023-11-14");
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
    await page.getByRole("button", { name: "Restore project" }).click();
    await expect(page.getByText("No managed backups available")).toBeVisible();

    await openHarness(page, "/projects", "create-error");
    await page.getByRole("button", { name: "Create project" }).click();
    await page.getByRole("button", { name: "Choose directory" }).click();
    await page.getByLabel("Create project name").fill("Conflict");
    await page.getByRole("button", { name: "Review creation" }).click();
    await expect(page.getByRole("alert")).toContainText("template_plan_stale");

    await openHarness(page, "/projects", "restore-error");
    await page.getByRole("button", { name: "Restore project" }).click();
    await page.getByRole("button", { name: "Choose directory" }).click();
    await page.getByLabel("Restore project name").fill("Conflict");
    await page.getByRole("button", { name: "Review restore" }).click();
    await expect(page.getByRole("alert")).toContainText("backup_target_conflict");
});

async function openHarness(page: import("@playwright/test").Page, route: string, state = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}
