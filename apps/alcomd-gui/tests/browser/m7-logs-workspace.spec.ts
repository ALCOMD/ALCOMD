import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("activity search and record filters preserve the loaded snapshot and open real operation detail in a dialog", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Factivity&state=ready");
    const table = page.getByRole("table", { name: "Activity", exact: true });
    await expect(table.getByRole("columnheader")).toHaveText(["Time", "Status", "Activity", "Target", "Actions"]);
    await expect(table.getByText("operation packages apply running", { exact: true })).toBeVisible();
    await expect(page.getByText("1 of 1 loaded entries · Search and filters apply to loaded entries only.")).toBeVisible();
    const search = page.getByRole("searchbox", { name: "Search activity" });
    await search.fill("missing event");
    await expect(table.getByText("No matching loaded activity. Change the filters or load more entries.")).toBeVisible();
    await search.fill("PACKAGES");
    await expect(table.getByText("operation packages apply running", { exact: true })).toBeVisible();
    await page.getByRole("checkbox", { name: "Show details" }).check();
    await expect(table.getByText("Summary code", { exact: true })).toBeVisible();
    await table.getByRole("button", { name: "View operation" }).click();
    const dialog = page.getByRole("dialog", { name: "Operation detail" });
    await expect(dialog).toBeVisible();
    // Material's native dialog is in shadow DOM; its slotted content belongs to the host.
    const dialogHost = page.locator("md-dialog").filter({ has: page.getByRole("heading", { name: "Operation detail", exact: true }) });
    await expect(dialogHost.getByText("Phase", { exact: true })).toBeVisible();
    await expect(dialogHost.getByText("00000000-0000-4000-8000-000000000105", { exact: true })).toBeVisible();
    await dialogHost.getByRole("button", { name: "Close", exact: true }).click();
    await expect(dialog).not.toBeVisible();
    await expect(page.getByRole("heading", { name: "Activity", exact: true, level: 1 })).toBeVisible();
    await expect(search).toHaveValue("PACKAGES");
});

test("diagnostics filters real severity fields and retains complete diagnostic identity", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fdiagnostics&state=ready");
    const table = page.getByRole("table", { name: "Diagnostics", exact: true });
    await expect(table.getByRole("columnheader")).toHaveText(["Time", "Severity", "Subsystem", "Diagnostic", "Actions"]);
    await page.getByRole("combobox", { name: "Severity", exact: true }).click();
    await page.getByRole("option", { name: "warning", exact: true }).click();
    await expect(table.getByText("No matching loaded diagnostics. Change the filters or load more entries.")).toBeVisible();
    await page.getByRole("combobox", { name: "Severity", exact: true }).click();
    await page.getByRole("option", { name: "error", exact: true }).click();
    await expect(table.getByText("package_archive_invalid", { exact: true })).toBeVisible();
    await page.getByRole("searchbox", { name: "Search diagnostics" }).fill("00000000-0000-4000-8000-000000000998");
    await expect(table.getByText("package_archive_invalid", { exact: true })).toBeVisible();
    await table.getByRole("button", { name: "View operation" }).click();
    await expect(page.getByRole("dialog", { name: "Operation detail" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog", { name: "Operation detail" })).not.toBeVisible();
    await expect(table.getByText("package_archive_invalid", { exact: true })).toBeVisible();
});

test("empty logs retain usable search and filters without creating records", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Factivity&state=empty");
    await expect(page.getByRole("searchbox", { name: "Search activity" })).toBeVisible();
    await expect(page.getByRole("table", { name: "Activity", exact: true }).getByText("No activity yet")).toBeVisible();
    await expect(page.getByRole("button", { name: "View operation" })).toHaveCount(0);
    await page.getByRole("navigation", { name: "Logs", exact: true }).getByRole("button", { name: "Diagnostics" }).click();
    await expect(page.getByRole("table", { name: "Diagnostics", exact: true }).getByText("No diagnostics reported")).toBeVisible();
});
