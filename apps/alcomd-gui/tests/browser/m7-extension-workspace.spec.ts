import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 1440, height: 900 } });

test("Extensions expose installed cards and their lifecycle and Open actions", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready");
    await expect(page.getByRole("heading", { name: "Installed", exact: true })).toBeVisible();
    const card = page.getByRole("article", { name: "com.cqmhv.discord", exact: true });
    await expect(card.getByRole("heading", { name: "com.cqmhv.discord", exact: true })).toBeVisible();
    await expect(page.getByRole("table", { name: "Extensions", exact: true })).toHaveCount(0);
    await card.getByRole("switch", { name: "Enabled", exact: true }).click();
    await expect(page.getByRole("status").filter({ hasText: "com.cqmhv.discord: disabled." })).toBeVisible();
    // The lifecycle fixture is stateless: assert the returned mutation feedback,
    // not a fictitious persisted change after its subsequent list refresh.
    await card.getByRole("button", { name: "Manage extension", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Extension detail", exact: true })).toBeVisible();
});

test("Extensions open only their declared Portable UI through the card", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready");
    await page.getByRole("article", { name: "com.cqmhv.discord", exact: true }).getByRole("button", { name: "Open", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Extension UI", exact: true })).toBeVisible();
});

test("Extension install input and real Plan review share one modal and can be discarded", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions&state=ready");
    await page.getByRole("button", { name: "Install extension", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Install extension", exact: true })).toBeVisible();
    // The native shadow dialog owns the accessible name, while form controls
    // are slotted light-DOM children of md-dialog, not DOM descendants of it.
    const dialog = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Install extension", exact: true }) });
    await dialog.getByLabel("Extension package", { exact: true }).fill("C:\\Fixture\\extension.alcomdext");
    await dialog.getByLabel("Expected registry revision", { exact: true }).fill("0");
    await dialog.getByRole("button", { name: "Create install plan", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(dialog.getByRole("heading", { name: "Review extension install", exact: true })).toBeVisible();
    await expect(dialog.getByText("Publisher", { exact: true })).toBeVisible();
    await dialog.getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(dialog.getByLabel("Extension package", { exact: true })).toHaveValue("C:\\Fixture\\extension.alcomdext");
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("Extension permission and uninstall workflows open named modals with cancel", async ({ page }) => {
    await page.goto("/browser-harness.html?route=%2Fextensions%2Fcom.cqmhv.discord&state=ready");
    await page.getByRole("button", { name: "Manage permissions", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Extension permissions", exact: true })).toBeVisible();
    await page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Extension permissions", exact: true }) }).getByRole("button", { name: "Close", exact: true }).click();
    await page.getByRole("button", { name: "Uninstall extension", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Uninstall extension", exact: true })).toBeVisible();
    const preparation = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Uninstall extension", exact: true }) });
    await preparation.getByRole("button", { name: "Create uninstall plan", exact: true }).click();
    await expect(page.getByRole("dialog", { name: "Review extension uninstall", exact: true })).toBeVisible();
    const review = page.locator("md-dialog[open]").filter({ has: page.getByRole("heading", { name: "Review extension uninstall", exact: true }) });
    await expect(page.getByRole("dialog")).toHaveCount(1);
    await expect(review.getByText("retain data", { exact: true })).toBeVisible();
    await review.getByRole("button", { name: "Discard plan", exact: true }).click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
});
