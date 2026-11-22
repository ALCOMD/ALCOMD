import { expect, test } from "@playwright/test";

test("Settings lists client programs and confirms a specific live connection before disconnecting it", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fsettings&state=ready");
    await page.getByRole("button", { name: "Clients", exact: true }).click();
    await expect(page.getByText("Installed GUI", {exact: true})).toBeVisible();
    await page.getByRole("button", {name: "View connections"}).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByText("self-reported GUI", {exact: false})).toBeVisible();
    await dialog.getByRole("button", {name: "Disconnect connection"}).click();
    await dialog.getByRole("button", {name: "Back", exact: true}).click();
    await expect(dialog.getByText("connection-fixture", {exact: true})).toBeVisible();
    await dialog.getByRole("button", {name: "Disconnect connection"}).click();
    await dialog.getByRole("button", {name: "Disconnect", exact: true}).click();
    await expect(dialog.getByText("No active connections.", {exact: true})).toBeVisible();
    await dialog.getByRole("button", {name: "Close", exact: true}).click();
    await expect(page.getByText("0 active instances", {exact: false})).toBeVisible();
});
