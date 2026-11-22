import { expect, test } from "@playwright/test";

test("Register an AI client, issue its credential once, and revoke it after confirmation", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fsettings&state=ready");
    await page.getByRole("button", { name: "Clients", exact: true }).click();
    const ai = page.getByRole("region", { name: "AI clients", exact: true });
    await ai.getByRole("textbox", { name: "Client name" }).fill("Local assistant");
    await ai.getByRole("button", { name: "Register client" }).click();
    const dialog = page.locator("md-dialog[open]");
    await expect(dialog.getByText("Local assistant", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    const row = ai.locator(".settings-row").filter({ hasText: "Local assistant" });
    await row.getByRole("button", { name: "Issue credential" }).click();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(dialog.getByText("browser-fixture-one-time-credential")).toBeVisible();
    await dialog.getByRole("button", { name: "Close", exact: true }).click();
    await expect(ai.getByText("browser-fixture-one-time-credential")).toHaveCount(0);
    await row.getByRole("button", { name: "Revoke client" }).click();
    await dialog.getByRole("button", { name: "Cancel" }).click();
    await row.getByRole("button", { name: "Revoke client" }).click();
    await dialog.getByRole("button", { name: "Confirm", exact: true }).click();
    await expect(row.getByText("Revoked", { exact: true })).toBeVisible();
    await expect(ai.getByText("Core connections (0)", { exact: true })).toBeVisible();
    await expect(ai.getByText("Transport status · available", { exact: true })).toBeVisible();
});

test("Review the original management request before applying its change", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fsettings&state=ready");
    await page.getByRole("button", { name: "Clients", exact: true }).click();
    const ai = page.getByRole("region", { name: "AI clients", exact: true });
    const dialog = page.locator("md-dialog[open]");
    await ai.getByRole("textbox", { name: "Client name" }).fill("Reviewed assistant");
    await ai.getByRole("button", { name: "Register client" }).click();
    await dialog.getByRole("button", { name: "Submit for approval" }).click();
    await expect(ai.getByRole("button", { name: "Issue credential" })).toHaveCount(0);
    await ai.locator(".settings-row").filter({ hasText: "local:owner · Pending" }).getByRole("button", { name: "Review request" }).click();
    await expect(dialog.getByText("Reviewed assistant", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Approve request" }).click();
    await expect(ai.getByText("local:owner · Approved")).toBeVisible();
    await expect(ai.locator(".settings-row").filter({ hasText: "Reviewed assistant" }).getByRole("button", { name: "Issue credential" })).toBeVisible();
});
