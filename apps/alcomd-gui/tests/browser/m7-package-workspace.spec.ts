import { expect, test, type Page } from "@playwright/test";

const PROJECT_ID = "00000000-0000-4000-8000-000000000101";

test("Package refresh is visible and handles zero or one repository with a final reload", async ({ page }) => {
    await openHarness(page, "package-no-repositories");
    const refresh = page.getByRole("button", { name: "Refresh", exact: true });
    await expect(refresh).toBeVisible();
    await refresh.click();
    await expect(page.getByText("0 refreshed, 0 failed.")).toBeVisible();

    await openHarness(page, "ready");
    await expect(page.getByText("Refreshed package")).toHaveCount(0);
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("1 refreshed, 0 failed.")).toBeVisible();
    await expect(page.getByText("Refreshed package")).toBeVisible();
});

test("Package refresh processes every repository and reports partial failure without fake success", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("2 refreshed, 0 failed.")).toBeVisible();
    await expect(page.getByText("Refreshed package")).toBeVisible();

    await openHarness(page, "package-partial-failure");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText("1 refreshed, 1 failed.");
    await expect(page.getByText("Refreshed package")).toBeVisible();
    await expect(page.getByText("2 refreshed, 0 failed.")).toHaveCount(0);
});

test("Package refresh retries a revision conflict once and then reloads", async ({ page }) => {
    await openHarness(page, "package-revision-conflict");
    await page.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(page.getByText("1 refreshed, 0 failed.")).toBeVisible();
    await expect(page.getByText("Refreshed package")).toBeVisible();
});

test("Package source filter uses daemon source kinds and only changes presentation", async ({ page }) => {
    await openHarness(page, "package-multiple");
    await expect(page.getByText("Remote tools")).toBeVisible();
    await expect(page.getByText("Local tools")).toBeVisible();

    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Remote", exact: true }).click();
    await expect(page.getByText("Remote tools")).toBeVisible();
    await expect(page.getByText("Local tools")).toHaveCount(0);
    await expect(page.getByText("2 packages")).toBeVisible();

    await openHarness(page, "package-multiple");
    await page.getByRole("combobox", { name: "Source" }).click();
    await page.getByRole("option", { name: "Local", exact: true }).click();
    await expect(page.getByText("Local tools")).toBeVisible();
    await expect(page.getByText("Remote tools")).toHaveCount(0);
    await expect(page.getByText("1 packages")).toBeVisible();
    await expect(page.getByText(/refreshed, .* failed/)).toHaveCount(0);
});

async function openHarness(page: Page, state: string) {
    await page.goto(`/browser-harness.html?route=${encodeURIComponent(`/projects/${PROJECT_ID}`)}&state=${state}`);
}
