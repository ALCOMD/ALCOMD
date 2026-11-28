import { expect, test, type Page } from "@playwright/test";

async function actions(page: Page) {
    return page.evaluate(() => window.resourceRequests
        .filter((request) => request.method === "gui_product_update")
        .map((request) => (request.params as { action: string }).action)
        .filter((action) => action !== "status"));
}

test("checks, downloads the selected version and confirms restart from About", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fabout&state=ready&product-update-fixture=normal");
    const updates = page.getByRole("region", { name: "Product updates" });
    await updates.getByRole("button", { name: "Check for updates" }).click();
    await expect(updates.getByRole("status")).toHaveText("Version 4.0.0-alpha.1 is available (20 MiB).");
    expect(await actions(page)).toEqual(["check"]);
    await updates.getByRole("button", { name: "Download update" }).click();
    await expect(updates.getByRole("status")).toHaveText("Version 4.0.0-alpha.1 is ready to install.");
    await updates.getByRole("button", { name: "Install and restart" }).click();
    const confirmation = page.locator('md-dialog[open][data-aria-label="Install and restart"]');
    await expect(confirmation).toBeVisible();
    expect(await actions(page)).toEqual(["check", "download"]);
    await confirmation.getByRole("button", { name: "Install and restart" }).click();
    await expect(updates.getByRole("status")).toHaveText("Restarting to complete the update…");
    expect(await actions(page)).toEqual(["check", "download", "install"]);
});

test("cancels the transfer without sending an installation request", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fabout&state=ready&product-update-fixture=cancel");
    const updates = page.getByRole("region", { name: "Product updates" });
    await updates.getByRole("button", { name: "Check for updates" }).click();
    await updates.getByRole("button", { name: "Download update" }).click();
    await expect(updates.getByRole("status")).toHaveText("Downloading the signed update…");
    await updates.getByRole("button", { name: "Cancel" }).click();
    await expect(updates.getByRole("button", { name: "Check for updates" })).toBeVisible();
    expect(await actions(page)).toEqual(["check", "download", "cancel"]);
});

test("resumes an interrupted update only after restart confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fabout&state=ready&product-update-fixture=recovery");
    const updates = page.getByRole("region", { name: "Product updates" });
    await expect(updates.getByRole("status")).toHaveText("An interrupted update is ready to resume.");
    await updates.getByRole("button", { name: "Resume and restart" }).click();
    const confirmation = page.locator('md-dialog[open][data-aria-label="Resume and restart"]');
    await confirmation.getByRole("button", { name: "Cancel" }).click();
    await expect(confirmation).toHaveCount(0);
    expect(await actions(page)).toEqual([]);
    await updates.getByRole("button", { name: "Resume and restart" }).click();
    await confirmation.getByRole("button", { name: "Resume and restart" }).click();
    await expect.poll(() => actions(page)).toEqual(["resume"]);
});
