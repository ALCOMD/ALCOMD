import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

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

test("Unity explicit selection clears to Automatic while preserving arguments", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity`);
    await expect(page.getByText("Selection:").locator("..")).toContainText("Explicit");
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByRole("button", { name: "Forget selected editor" }).click();
    await expect(page.getByText("Selection:").locator("..")).toContainText("Automatic");
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
});

test("Unity automatic launch handles zero, one, and multiple compatible editors", async ({ page }) => {
    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-automatic");
    await expect(page.getByText("Selection:").locator("..")).toContainText("Automatic");
    await launch(page);
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();

    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-zero");
    await launch(page);
    await expect(page.getByRole("alert")).toContainText("unity_installation_not_found");

    await openHarness(page, `/projects/${PROJECT_ID}/unity`, "unity-multiple");
    await launch(page);
    const chooser = page.getByRole("combobox", { name: "Choose a compatible editor" });
    await expect(chooser).toBeVisible();
    await chooser.click();
    await page.getByRole("option", { name: "Unity 2022.3.22f1 (x86_64)" }).nth(1).click();
    await page.getByRole("button", { name: "Select and launch" }).click();
    await expect(page.getByText("Unity launch spawned.")).toBeVisible();
});

async function launch(page: Page) {
    await page.getByRole("button", { name: "Launch Unity" }).click();
    await page.locator("md-dialog[open]").getByRole("button", { name: "Confirm" }).click();
}

async function openHarness(page: Page, route: string, state = "ready") {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(route)}&state=${state}`);
}
