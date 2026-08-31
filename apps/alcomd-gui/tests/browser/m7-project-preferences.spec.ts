import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";
const OPERATION_ID = "00000000-0000-4000-8000-000000000105";

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
    await expect(page.getByLabel("Additional arguments")).toHaveValue("-logFile");
    await page.getByRole("button", { name: "Clear launch arguments" }).click();
    await expect(page.getByLabel("Additional arguments")).toHaveValue("");
    await expect(page.getByText("Exact installations").locator("..")).toContainText("1");
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
